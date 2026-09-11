#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StatusCode {
    Ok,
    Created,
    Found,
    BadRequest,
    Forbidden,
    NotFound,
    MethodNotAllowed,
    RequestTimeout,
    ContentTooLarge,
    UriTooLong,
    InternalServerError,
    NotImplemented,
    BadGateway,
    GatewayTimeout,
    HttpVersionNotSupported,
    Custom(u16, String),
}

impl StatusCode {
    pub fn code(&self) -> u16 {
        match self {
            Self::Ok => 200,
            Self::Created => 201,
            Self::Found => 302,
            Self::BadRequest => 400,
            Self::Forbidden => 403,
            Self::NotFound => 404,
            Self::MethodNotAllowed => 405,
            Self::RequestTimeout => 408,
            Self::ContentTooLarge => 413,
            Self::UriTooLong => 414,
            Self::InternalServerError => 500,
            Self::NotImplemented => 501,
            Self::BadGateway => 502,
            Self::GatewayTimeout => 504,
            Self::HttpVersionNotSupported => 505,
            Self::Custom(code, _) => *code,
        }
    }

    pub fn reason(&self) -> &str {
        match self {
            Self::Ok => "OK",
            Self::Created => "Created",
            Self::Found => "Found",
            Self::BadRequest => "Bad Request",
            Self::Forbidden => "Forbidden",
            Self::NotFound => "Not Found",
            Self::MethodNotAllowed => "Method Not Allowed",
            Self::RequestTimeout => "Request Timeout",
            Self::ContentTooLarge => "Content Too Large",
            Self::UriTooLong => "URI Too Long",
            Self::InternalServerError => "Internal Server Error",
            Self::NotImplemented => "Not Implemented",
            Self::BadGateway => "Bad Gateway",
            Self::GatewayTimeout => "Gateway Timeout",
            Self::HttpVersionNotSupported => "HTTP Version Not Supported",
            Self::Custom(_, reason) => reason,
        }
    }
}
