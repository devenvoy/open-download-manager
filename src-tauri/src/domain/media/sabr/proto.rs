// Created: 2026-11-07
// Reason: feat(media): modularize download executors, enable native multi-track streaming, and fix progress UX
// Last updated: 2026-11-07
// Reason: feat(media): modularize download executors, enable native multi-track streaming, and fix progress UX
// Description: UMP (Universal Media Protocol) chunk framing and variable-length integer encoding.

pub struct ProtoReader<'a> {
    pub data: &'a [u8],
}

impl<'a> ProtoReader<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        Self { data }
    }

    pub fn read_varint(&mut self) -> Result<u64, String> {
        let mut result = 0u64;
        let mut shift = 0;
        while !self.data.is_empty() {
            let byte = self.data[0];
            self.data = &self.data[1..];
            result |= ((byte & 0x7F) as u64) << shift;
            if (byte & 0x80) == 0 {
                return Ok(result);
            }
            shift += 7;
            if shift >= 64 {
                return Err("Varint overflow in protobuf.".to_owned());
            }
        }
        Err("Unexpected end of protobuf varint.".to_owned())
    }

    pub fn read_tag(&mut self) -> Result<(u32, u8), String> {
        let varint = self.read_varint()?;
        let field_number = (varint >> 3) as u32;
        let wire_type = (varint & 0x07) as u8;
        Ok((field_number, wire_type))
    }

    pub fn read_length_delimited(&mut self) -> Result<&'a [u8], String> {
        let len = self.read_varint()? as usize;
        if self.data.len() < len {
            return Err("Unexpected end of length-delimited protobuf field.".to_owned());
        }
        let (bytes, rest) = self.data.split_at(len);
        self.data = rest;
        Ok(bytes)
    }

    pub fn skip_field(&mut self, wire_type: u8) -> Result<(), String> {
        match wire_type {
            0 => {
                let _ = self.read_varint()?;
                Ok(())
            }
            1 => {
                if self.data.len() < 8 {
                    return Err("Unexpected end of 64-bit protobuf field.".to_owned());
                }
                self.data = &self.data[8..];
                Ok(())
            }
            2 => {
                let _ = self.read_length_delimited()?;
                Ok(())
            }
            5 => {
                if self.data.len() < 4 {
                    return Err("Unexpected end of 32-bit protobuf field.".to_owned());
                }
                self.data = &self.data[4..];
                Ok(())
            }
            _ => Err(format!("Unsupported protobuf wire type: {wire_type}")),
        }
    }
}

pub fn encode_varint(mut val: u64, buf: &mut Vec<u8>) {
    while val >= 0x80 {
        buf.push(((val & 0x7F) as u8) | 0x80);
        val >>= 7;
    }
    buf.push((val & 0x7F) as u8);
}

pub fn encode_varint_field(buf: &mut Vec<u8>, field_no: u32, val: u64) {
    let tag = (field_no as u64) << 3;
    encode_varint(tag, buf);
    encode_varint(val, buf);
}

pub fn encode_bytes_field(buf: &mut Vec<u8>, field_no: u32, bytes: &[u8]) {
    let tag = ((field_no as u64) << 3) | 2;
    encode_varint(tag, buf);
    encode_varint(bytes.len() as u64, buf);
    buf.extend_from_slice(bytes);
}

pub fn encode_ump_chunk(part_type: u64, payload: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    encode_varint(part_type, &mut out);
    encode_varint(payload.len() as u64, &mut out);
    out.extend_from_slice(payload);
    out
}

pub fn parse_media_header_itag(payload: &[u8]) -> Option<u32> {
    let mut reader = ProtoReader::new(payload);
    while !reader.data.is_empty() {
        if let Ok((field_no, wire_type)) = reader.read_tag() {
            if field_no == 2 && wire_type == 0 {
                if let Ok(itag) = reader.read_varint() {
                    return Some(itag as u32);
                }
            } else {
                let _ = reader.skip_field(wire_type);
            }
        } else {
            break;
        }
    }
    None
}
