use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr},
    time::Duration,
};

use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::TcpStream,
    time::timeout,
};

use super::error::FtpError;

pub const DEFAULT_FTP_PORT: u16 = 21;
pub const DEFAULT_FTP_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FtpReply {
    pub code: u16,
    pub message: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FtpFeatures {
    pub epsv: bool,
    pub eprt: bool,
    pub size: bool,
    pub mdtm: bool,
    pub rest_stream: bool,
    pub mlst: bool,
    pub utf8: bool,
}

pub struct FtpConnection {
    reader: BufReader<tokio::net::tcp::OwnedReadHalf>,
    writer: tokio::net::tcp::OwnedWriteHalf,
    control_host: String,
    peer_addr: SocketAddr,
}

impl FtpConnection {
    pub async fn connect(host: &str, port: u16, connect_timeout: Duration) -> Result<Self, FtpError> {
        let stream = match timeout(connect_timeout, TcpStream::connect((host, port))).await {
            Ok(Ok(stream)) => stream,
            Ok(Err(err)) => return Err(FtpError::ConnectFailed(err.to_string())),
            Err(_) => return Err(FtpError::ConnectFailed(format!("Connection to {host}:{port} timed out"))),
        };

        let peer_addr = stream.peer_addr().map_err(FtpError::Io)?;
        let (read_half, write_half) = stream.into_split();
        let mut conn = Self {
            reader: BufReader::new(read_half),
            writer: write_half,
            control_host: host.to_owned(),
            peer_addr,
        };

        let reply = conn.read_reply(connect_timeout).await?;
        if reply.code != 220 {
            return Err(FtpError::ConnectFailed(format!(
                "Unexpected greeting reply: {} {}",
                reply.code, reply.message
            )));
        }

        Ok(conn)
    }

    pub fn control_host(&self) -> &str {
        &self.control_host
    }

    pub fn peer_ip(&self) -> IpAddr {
        self.peer_addr.ip()
    }

    pub async fn read_reply(&mut self, read_timeout: Duration) -> Result<FtpReply, FtpError> {
        let mut lines = Vec::new();
        let mut expected_code: Option<u16> = None;

        loop {
            let mut line = String::new();
            match timeout(read_timeout, self.reader.read_line(&mut line)).await {
                Ok(Ok(0)) => return Err(FtpError::ConnectFailed("Control connection closed by server".to_owned())),
                Ok(Ok(_)) => {}
                Ok(Err(err)) => return Err(FtpError::Io(err)),
                Err(_) => return Err(FtpError::Protocol("Timed out waiting for server reply".to_owned())),
            }

            let trimmed = line.trim_end_matches(['\r', '\n']);
            if trimmed.is_empty() {
                continue;
            }

            lines.push(trimmed.to_owned());

            // Check if line starts with 3 digits
            if trimmed.len() >= 3 && trimmed.as_bytes()[..3].iter().all(u8::is_ascii_digit) {
                let code = trimmed[..3].parse::<u16>().map_err(|_| {
                    FtpError::Protocol(format!("Invalid reply status code: {trimmed}"))
                })?;

                if expected_code.is_none() {
                    if trimmed.len() > 3 && trimmed.as_bytes()[3] == b'-' {
                        expected_code = Some(code);
                    } else {
                        // Single-line reply
                        let message = if trimmed.len() > 4 {
                            trimmed[4..].trim().to_owned()
                        } else {
                            String::new()
                        };
                        return Ok(FtpReply { code, message });
                    }
                } else if expected_code == Some(code) && (trimmed.len() == 3 || trimmed.as_bytes()[3] == b' ') {
                    // Final line of multi-line reply
                    let message = lines.join("\n");
                    return Ok(FtpReply { code, message });
                }
            }
        }
    }

    pub async fn send_command(&mut self, cmd: &str) -> Result<FtpReply, FtpError> {
        let command_bytes = format!("{cmd}\r\n");
        self.writer
            .write_all(command_bytes.as_bytes())
            .await
            .map_err(FtpError::Io)?;
        self.writer.flush().await.map_err(FtpError::Io)?;
        self.read_reply(DEFAULT_FTP_TIMEOUT).await
    }

    pub async fn login(&mut self, user: Option<&str>, pass: Option<&str>) -> Result<(), FtpError> {
        let username = user.unwrap_or("anonymous");
        let reply = self.send_command(&format!("USER {username}")).await?;

        if reply.code == 230 {
            return Ok(());
        }

        if reply.code == 331 {
            let password = pass.unwrap_or("anonymous@");
            let pass_reply = self.send_command(&format!("PASS {password}")).await?;
            if pass_reply.code == 230 {
                return Ok(());
            }
            return Err(FtpError::AuthFailed(pass_reply.message));
        }

        Err(FtpError::AuthFailed(reply.message))
    }

    pub async fn send_feat(&mut self) -> FtpFeatures {
        let mut features = FtpFeatures::default();
        if let Ok(reply) = self.send_command("FEAT").await {
            if reply.code == 211 {
                for line in reply.message.lines() {
                    let upper = line.trim().to_ascii_uppercase();
                    if upper.starts_with("EPSV") {
                        features.epsv = true;
                    } else if upper.starts_with("EPRT") {
                        features.eprt = true;
                    } else if upper.starts_with("SIZE") {
                        features.size = true;
                    } else if upper.starts_with("MDTM") {
                        features.mdtm = true;
                    } else if upper.starts_with("REST STREAM") || upper == "REST" {
                        features.rest_stream = true;
                    } else if upper.starts_with("MLST") || upper.starts_with("MLSD") {
                        features.mlst = true;
                    } else if upper.contains("UTF8") {
                        features.utf8 = true;
                    }
                }
            }
        }
        features
    }

    pub async fn send_opts_utf8(&mut self) -> Result<bool, FtpError> {
        let reply = self.send_command("OPTS UTF8 ON").await?;
        Ok(reply.code == 200)
    }

    pub async fn send_type_binary(&mut self) -> Result<(), FtpError> {
        let reply = self.send_command("TYPE I").await?;
        if reply.code == 200 {
            Ok(())
        } else {
            Err(FtpError::Protocol(format!("TYPE I rejected: {} {}", reply.code, reply.message)))
        }
    }

    pub async fn send_size(&mut self, path: &str) -> Result<Option<u64>, FtpError> {
        let reply = self.send_command(&format!("SIZE {path}")).await?;
        if reply.code == 213 {
            let size = reply
                .message
                .split_whitespace()
                .next()
                .and_then(|s| s.parse::<u64>().ok())
                .ok_or_else(|| FtpError::Protocol(format!("Invalid SIZE reply: {}", reply.message)))?;
            Ok(Some(size))
        } else if reply.code == 550 {
            Err(FtpError::FileNotFound(reply.message))
        } else if reply.code == 500 || reply.code == 502 {
            // Command unrecognized or not implemented
            Ok(None)
        } else {
            Ok(None)
        }
    }

    pub async fn send_mdtm(&mut self, path: &str) -> Result<Option<String>, FtpError> {
        let reply = self.send_command(&format!("MDTM {path}")).await?;
        if reply.code == 213 {
            let timestamp = reply.message.trim().to_owned();
            Ok(Some(timestamp))
        } else {
            Ok(None)
        }
    }

    pub async fn send_mlst(&mut self, path: &str) -> Result<(Option<u64>, Option<String>), FtpError> {
        let reply = self.send_command(&format!("MLST {path}")).await?;
        if reply.code == 250 {
            let mut size = None;
            let mut modify = None;
            for line in reply.message.lines() {
                let trimmed = line.trim();
                for fact in trimmed.split(';') {
                    let fact = fact.trim();
                    if let Some(s) = fact.strip_prefix("size=") {
                        size = s.parse::<u64>().ok();
                    } else if let Some(m) = fact.strip_prefix("modify=") {
                        modify = Some(m.to_owned());
                    }
                }
            }
            Ok((size, modify))
        } else {
            Ok((None, None))
        }
    }

    pub async fn send_epsv(&mut self) -> Result<SocketAddr, FtpError> {
        let reply = self.send_command("EPSV").await?;
        if reply.code != 229 {
            return Err(FtpError::DataConnectionFailed(format!(
                "EPSV rejected: {} {}",
                reply.code, reply.message
            )));
        }

        // Parse (|||<port>|) or delimiter-based port
        let open_paren = reply.message.find('(');
        let close_paren = reply.message.find(')');
        let inner = match (open_paren, close_paren) {
            (Some(s), Some(e)) if s < e => &reply.message[s + 1..e],
            _ => {
                return Err(FtpError::Protocol(format!(
                    "Malformed EPSV reply: {}",
                    reply.message
                )))
            }
        };

        let delimiter = inner.chars().next().ok_or_else(|| {
            FtpError::Protocol("Empty EPSV response".to_owned())
        })?;

        let parts: Vec<&str> = inner.split(delimiter).collect();
        if parts.len() < 4 {
            return Err(FtpError::Protocol(format!(
                "Malformed EPSV port specification: {inner}"
            )));
        }

        let port_str = parts[3];
        let port: u16 = port_str.parse().map_err(|_| {
            FtpError::Protocol(format!("Invalid EPSV port number: {port_str}"))
        })?;

        Ok(SocketAddr::new(self.peer_ip(), port))
    }

    pub async fn send_pasv(&mut self) -> Result<SocketAddr, FtpError> {
        let reply = self.send_command("PASV").await?;
        if reply.code != 227 {
            return Err(FtpError::DataConnectionFailed(format!(
                "PASV rejected: {} {}",
                reply.code, reply.message
            )));
        }

        // Parse (h1,h2,h3,h4,p1,p2)
        let open_paren = reply.message.find('(');
        let close_paren = reply.message.find(')');
        let numbers_str = match (open_paren, close_paren) {
            (Some(start), Some(end)) if start < end => &reply.message[start + 1..end],
            _ => {
                return Err(FtpError::Protocol(format!(
                    "Malformed PASV reply: {}",
                    reply.message
                )))
            }
        };

        let nums: Vec<u8> = numbers_str
            .split(',')
            .filter_map(|s| s.trim().parse::<u8>().ok())
            .collect();

        if nums.len() != 6 {
            return Err(FtpError::Protocol(format!(
                "Malformed PASV response digits: {}",
                reply.message
            )));
        }

        let port = ((nums[4] as u16) << 8) | (nums[5] as u16);
        let pasv_ip = Ipv4Addr::new(nums[0], nums[1], nums[2], nums[3]);

        // NAT Traversal check: If server advertises 0.0.0.0 or a private IP while control host is different,
        // use the control host IP or resolve it.
        let target_ip = if pasv_ip.is_unspecified() || (pasv_ip.is_private() && self.control_host != "127.0.0.1" && self.control_host != "localhost") {
            // Resolve control host
            match tokio::net::lookup_host(format!("{}:{}", self.control_host, port)).await {
                Ok(mut addrs) => match addrs.next() {
                    Some(addr) => addr.ip(),
                    None => IpAddr::V4(pasv_ip),
                },
                Err(_) => IpAddr::V4(pasv_ip),
            }
        } else {
            IpAddr::V4(pasv_ip)
        };

        Ok(SocketAddr::new(target_ip, port))
    }

    pub async fn send_port(&mut self, addr: SocketAddr) -> Result<(), FtpError> {
        let ip = match addr.ip() {
            IpAddr::V4(ip) => ip,
            IpAddr::V6(_) => {
                return Err(FtpError::Protocol(
                    "IPv6 PORT requires EPRT, standard PORT requires IPv4".to_owned(),
                ))
            }
        };
        let port = addr.port();
        let octets = ip.octets();
        let p1 = (port >> 8) as u8;
        let p2 = (port & 0xFF) as u8;

        let port_cmd = format!(
            "PORT {},{},{},{},{},{}",
            octets[0], octets[1], octets[2], octets[3], p1, p2
        );
        let reply = self.send_command(&port_cmd).await?;
        if reply.code == 200 {
            Ok(())
        } else {
            Err(FtpError::DataConnectionFailed(format!(
                "PORT command rejected: {} {}",
                reply.code, reply.message
            )))
        }
    }

    pub async fn send_rest(&mut self, offset: u64) -> Result<bool, FtpError> {
        let reply = self.send_command(&format!("REST {offset}")).await?;
        if reply.code == 350 {
            Ok(true)
        } else {
            Ok(false)
        }
    }

    pub async fn send_cwd(&mut self, path: &str) -> Result<bool, FtpError> {
        let reply = self.send_command(&format!("CWD {path}")).await?;
        Ok(reply.code == 250)
    }

    pub async fn send_cdup(&mut self) -> Result<bool, FtpError> {
        let reply = self.send_command("CDUP").await?;
        Ok(reply.code == 250)
    }

    pub async fn send_retr(&mut self, path: &str) -> Result<(), FtpError> {
        let reply = self.send_command(&format!("RETR {path}")).await?;
        if reply.code == 150 || reply.code == 125 {
            Ok(())
        } else if reply.code == 550 {
            Err(FtpError::FileNotFound(reply.message))
        } else {
            Err(FtpError::Protocol(format!(
                "RETR failed: {} {}",
                reply.code, reply.message
            )))
        }
    }

    pub async fn send_abor(&mut self) -> Result<(), FtpError> {
        let _ = self.send_command("ABOR").await;
        Ok(())
    }

    pub async fn send_noop(&mut self) -> Result<(), FtpError> {
        let reply = self.send_command("NOOP").await?;
        if reply.code == 200 {
            Ok(())
        } else {
            Err(FtpError::Protocol("NOOP rejected".to_owned()))
        }
    }

    pub async fn check_transfer_end_reply(&mut self) -> Result<(), FtpError> {
        let reply = self.read_reply(DEFAULT_FTP_TIMEOUT).await?;
        if reply.code == 226 || reply.code == 250 {
            Ok(())
        } else if reply.code == 426 {
            Err(FtpError::TransferAborted(reply.message))
        } else {
            Err(FtpError::Protocol(format!(
                "Transfer completed with unexpected reply: {} {}",
                reply.code, reply.message
            )))
        }
    }

    pub async fn send_quit(&mut self) -> Result<(), FtpError> {
        let _ = self.send_command("QUIT").await;
        Ok(())
    }
}
