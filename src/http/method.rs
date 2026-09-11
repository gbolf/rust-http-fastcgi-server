#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Method {
    Get,
    Head,
    Post,
    KnownUnsupported(String),
    Unknown(String),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InvalidMethod;

impl Method {
    pub fn parse(value: &str) -> Result<Self, InvalidMethod> {
        if value.is_empty() || !value.bytes().all(is_token_byte) {
            return Err(InvalidMethod);
        }

        Ok(match value {
            "GET" => Self::Get,
            "HEAD" => Self::Head,
            "POST" => Self::Post,
            "PUT" | "DELETE" | "CONNECT" | "OPTIONS" | "TRACE" | "PATCH" => {
                Self::KnownUnsupported(value.to_string())
            }
            _ => Self::Unknown(value.to_string()),
        })
    }

    pub fn as_str(&self) -> &str {
        match self {
            Self::Get => "GET",
            Self::Head => "HEAD",
            Self::Post => "POST",
            Self::KnownUnsupported(value) | Self::Unknown(value) => value,
        }
    }

    pub fn is_head(&self) -> bool {
        matches!(self, Self::Head)
    }
}

fn is_token_byte(byte: u8) -> bool {
    matches!(byte, b'!' | b'#'..=b'\'' | b'*' | b'+' | b'-' | b'.' | b'0'..=b'9' | b'A'..=b'Z' | b'^'..=b'z' | b'|' | b'~')
}
