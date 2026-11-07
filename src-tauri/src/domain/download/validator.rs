use serde::{de, Deserialize, Deserializer, Serialize, Serializer};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TransferValidator {
    ETag(String),
    LastModified(String),
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "type", content = "value", rename_all = "camelCase")]
enum TransferValidatorRepr {
    #[serde(rename = "etag")]
    ETag(String),
    #[serde(rename = "lastModified")]
    LastModified(String),
}

impl TransferValidator {
    pub fn etag(value: impl Into<String>) -> Self {
        Self::ETag(value.into())
    }

    pub fn last_modified(value: impl Into<String>) -> Self {
        Self::LastModified(value.into())
    }

    pub fn value(&self) -> &str {
        match self {
            Self::ETag(value) => value,
            Self::LastModified(value) => value,
        }
    }

    pub fn is_strong(&self) -> bool {
        match self {
            Self::ETag(value) => {
                !value.starts_with("W/") && value.starts_with('"') && value.ends_with('"')
            }
            Self::LastModified(value) => !value.trim().is_empty(),
        }
    }

    pub fn entity_header_name(&self) -> reqwest::header::HeaderName {
        match self {
            Self::ETag(_) => reqwest::header::ETAG,
            Self::LastModified(_) => reqwest::header::LAST_MODIFIED,
        }
    }

    pub fn if_range_value(&self) -> &str {
        self.value()
    }
}

impl Serialize for TransferValidator {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let repr = match self {
            Self::ETag(val) => TransferValidatorRepr::ETag(val.clone()),
            Self::LastModified(val) => TransferValidatorRepr::LastModified(val.clone()),
        };
        repr.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for TransferValidator {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Helper {
            Repr(TransferValidatorRepr),
            RawString(String),
        }

        match Helper::deserialize(deserializer)? {
            Helper::Repr(TransferValidatorRepr::ETag(val)) => Ok(TransferValidator::ETag(val)),
            Helper::Repr(TransferValidatorRepr::LastModified(val)) => {
                Ok(TransferValidator::LastModified(val))
            }
            Helper::RawString(s) => {
                if !s.starts_with("W/") && s.starts_with('"') && s.ends_with('"') {
                    Ok(TransferValidator::ETag(s))
                } else {
                    Err(de::Error::custom("unsupported untyped resume validator"))
                }
            }
        }
    }
}

pub fn deserialize_optional_transfer_validator<'de, D>(
    deserializer: D,
) -> Result<Option<TransferValidator>, D::Error>
where
    D: Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum OptionalHelper {
        Repr(TransferValidatorRepr),
        RawString(String),
        None,
    }

    match Option::<OptionalHelper>::deserialize(deserializer)? {
        Some(OptionalHelper::Repr(TransferValidatorRepr::ETag(val))) => {
            Ok(Some(TransferValidator::ETag(val)))
        }
        Some(OptionalHelper::Repr(TransferValidatorRepr::LastModified(val))) => {
            Ok(Some(TransferValidator::LastModified(val)))
        }
        Some(OptionalHelper::RawString(s)) => {
            if !s.starts_with("W/") && s.starts_with('"') && s.ends_with('"') {
                Ok(Some(TransferValidator::ETag(s)))
            } else {
                // Untyped strings that are not safe strong ETags are safely treated as None
                // so legacy records survive without errors and restart safely.
                Ok(None)
            }
        }
        Some(OptionalHelper::None) | None => Ok(None),
    }
}
