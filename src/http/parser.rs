use std::{
    fmt,
    io::{self, Read},
};

use crate::config::{MAX_HEADER_SIZE, MAX_REQUEST_LINE_SIZE};

use super::{HttpRequest, Method, StatusCode};

#[derive(Clone, Copy, Debug)]
pub struct RequestLimits {
    pub max_request_line_size: usize,
    pub max_header_size: usize,
    pub max_body_size: usize,
}

impl RequestLimits {
    pub fn new(max_body_size: usize) -> Self {
        Self {
            max_request_line_size: MAX_REQUEST_LINE_SIZE,
            max_header_size: MAX_HEADER_SIZE,
            max_body_size,
        }
    }
}

#[derive(Debug)]
pub enum ParseError {
    BadRequest(&'static str),
    RequestTimeout,
    ContentTooLarge,
    UriTooLong,
    NotImplemented,
    VersionNotSupported,
    Io(io::Error),
}

impl ParseError {
    pub fn status(&self) -> StatusCode {
        match self {
            Self::BadRequest(_) | Self::Io(_) => StatusCode::BadRequest,
            Self::RequestTimeout => StatusCode::RequestTimeout,
            Self::ContentTooLarge => StatusCode::ContentTooLarge,
            Self::UriTooLong => StatusCode::UriTooLong,
            Self::NotImplemented => StatusCode::NotImplemented,
            Self::VersionNotSupported => StatusCode::HttpVersionNotSupported,
        }
    }
}

impl fmt::Display for ParseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BadRequest(message) => write!(formatter, "bad request: {message}"),
            Self::RequestTimeout => write!(formatter, "request read timed out"),
            Self::ContentTooLarge => write!(formatter, "request body is too large"),
            Self::UriTooLong => write!(formatter, "request target is too long"),
            Self::NotImplemented => write!(formatter, "transfer encoding is not supported"),
            Self::VersionNotSupported => write!(formatter, "HTTP version is not supported"),
            Self::Io(error) => write!(formatter, "request read failed: {error}"),
        }
    }
}

impl std::error::Error for ParseError {}

pub fn read_request<R: Read>(
    reader: &mut R,
    limits: RequestLimits,
) -> Result<HttpRequest, ParseError> {
    let mut received = Vec::new();
    let header_end;

    // TCP is a byte stream: a request may be split at any byte boundary. Keep
    // reading until the complete CRLF-CRLF header delimiter has arrived.
    loop {
        if let Some(position) = find_bytes(&received, b"\r\n\r\n") {
            header_end = position + 4;
            if header_end > limits.max_header_size {
                return Err(ParseError::BadRequest(
                    "headers exceed the configured limit",
                ));
            }
            break;
        }

        if let Some(line_end) = find_bytes(&received, b"\r\n") {
            if line_end > limits.max_request_line_size {
                return Err(ParseError::UriTooLong);
            }
        } else if received.len() > limits.max_request_line_size {
            return Err(ParseError::UriTooLong);
        }

        if received.len() > limits.max_header_size {
            return Err(ParseError::BadRequest(
                "headers exceed the configured limit",
            ));
        }

        let mut chunk = [0_u8; 4096];
        let count = reader.read(&mut chunk).map_err(map_read_error)?;
        if count == 0 {
            return Err(ParseError::BadRequest(
                "connection closed before headers ended",
            ));
        }
        received.extend_from_slice(&chunk[..count]);
    }

    let (method, request_target, path, query_string, version, headers, content_length) =
        parse_head(&received[..header_end - 4], limits)?;

    if content_length > limits.max_body_size {
        return Err(ParseError::ContentTooLarge);
    }

    // Bytes after the header delimiter belong to the body. If they did not all
    // arrive with the headers, continue until exactly Content-Length bytes exist.
    let available_body = &received[header_end..];
    let initial_count = available_body.len().min(content_length);
    let mut body = Vec::with_capacity(content_length);
    body.extend_from_slice(&available_body[..initial_count]);
    while body.len() < content_length {
        let remaining = content_length - body.len();
        let mut chunk = [0_u8; 8192];
        let read_size = remaining.min(chunk.len());
        let count = reader
            .read(&mut chunk[..read_size])
            .map_err(map_read_error)?;
        if count == 0 {
            return Err(ParseError::BadRequest(
                "connection closed before body ended",
            ));
        }
        body.extend_from_slice(&chunk[..count]);
    }

    Ok(HttpRequest {
        method,
        request_target,
        path,
        query_string,
        version,
        headers,
        body,
    })
}

type ParsedHead = (
    Method,
    String,
    String,
    String,
    String,
    Vec<(String, String)>,
    usize,
);

fn parse_head(bytes: &[u8], limits: RequestLimits) -> Result<ParsedHead, ParseError> {
    let text = std::str::from_utf8(bytes)
        .map_err(|_| ParseError::BadRequest("request headers are not valid UTF-8"))?;
    let mut lines = text.split("\r\n");
    let request_line = lines
        .next()
        .ok_or(ParseError::BadRequest("missing request line"))?;

    if request_line.len() > limits.max_request_line_size {
        return Err(ParseError::UriTooLong);
    }

    let mut parts = request_line.split(' ');
    let method_text = parts
        .next()
        .ok_or(ParseError::BadRequest("invalid request line"))?;
    let target = parts
        .next()
        .ok_or(ParseError::BadRequest("invalid request line"))?;
    let version = parts
        .next()
        .ok_or(ParseError::BadRequest("invalid request line"))?;
    if parts.next().is_some() || method_text.is_empty() || target.is_empty() {
        return Err(ParseError::BadRequest("invalid request line"));
    }

    let method =
        Method::parse(method_text).map_err(|_| ParseError::BadRequest("invalid HTTP method"))?;
    if version != "HTTP/1.1" {
        if version.starts_with("HTTP/") {
            return Err(ParseError::VersionNotSupported);
        }
        return Err(ParseError::BadRequest("invalid HTTP version"));
    }
    if target.len() > limits.max_request_line_size {
        return Err(ParseError::UriTooLong);
    }
    if !target.starts_with('/')
        || target.contains('#')
        || target.bytes().any(|byte| byte.is_ascii_control())
    {
        return Err(ParseError::BadRequest(
            "only origin-form request targets are supported",
        ));
    }

    let (path, query_string) = match target.split_once('?') {
        Some((path, query)) => (path.to_string(), query.to_string()),
        None => (target.to_string(), String::new()),
    };

    let mut headers = Vec::new();
    for line in lines {
        if line.is_empty() {
            return Err(ParseError::BadRequest("unexpected empty header line"));
        }
        let (name, value) = line
            .split_once(':')
            .ok_or(ParseError::BadRequest("malformed header"))?;
        if name.is_empty() || !name.bytes().all(is_header_name_byte) {
            return Err(ParseError::BadRequest("invalid header name"));
        }
        let value = value.trim_matches([' ', '\t']);
        if value
            .bytes()
            .any(|byte| byte.is_ascii_control() && byte != b'\t')
        {
            return Err(ParseError::BadRequest("invalid header value"));
        }
        headers.push((name.to_string(), value.to_string()));
    }

    validate_host(&headers)?;
    if headers
        .iter()
        .any(|(name, _)| name.eq_ignore_ascii_case("transfer-encoding"))
    {
        // This server consistently answers Transfer-Encoding with 501 because
        // no transfer coding, including chunked encoding, is implemented.
        return Err(ParseError::NotImplemented);
    }
    let content_length = parse_content_length(&headers)?;

    Ok((
        method,
        target.to_string(),
        path,
        query_string,
        version.to_string(),
        headers,
        content_length,
    ))
}

fn validate_host(headers: &[(String, String)]) -> Result<(), ParseError> {
    let hosts: Vec<&str> = headers
        .iter()
        .filter(|(name, _)| name.eq_ignore_ascii_case("host"))
        .map(|(_, value)| value.as_str())
        .collect();
    if hosts.len() != 1 {
        return Err(ParseError::BadRequest(
            "HTTP/1.1 requires exactly one Host header",
        ));
    }
    if hosts[0].is_empty() || hosts[0].bytes().any(|byte| byte.is_ascii_whitespace()) {
        return Err(ParseError::BadRequest("invalid Host header"));
    }
    Ok(())
}

fn parse_content_length(headers: &[(String, String)]) -> Result<usize, ParseError> {
    let values: Vec<&str> = headers
        .iter()
        .filter(|(name, _)| name.eq_ignore_ascii_case("content-length"))
        .map(|(_, value)| value.as_str())
        .collect();
    if values.is_empty() {
        return Ok(0);
    }

    let first = parse_decimal_length(values[0])?;
    for value in &values[1..] {
        if parse_decimal_length(value)? != first {
            return Err(ParseError::BadRequest("conflicting Content-Length headers"));
        }
    }
    Ok(first)
}

fn parse_decimal_length(value: &str) -> Result<usize, ParseError> {
    if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(ParseError::BadRequest("invalid Content-Length"));
    }
    value
        .parse::<usize>()
        .map_err(|_| ParseError::BadRequest("invalid Content-Length"))
}

fn is_header_name_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric()
        || matches!(
            byte,
            b'!' | b'#'
                | b'$'
                | b'%'
                | b'&'
                | b'\''
                | b'*'
                | b'+'
                | b'-'
                | b'.'
                | b'^'
                | b'_'
                | b'`'
                | b'|'
                | b'~'
        )
}

fn find_bytes(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

fn map_read_error(error: io::Error) -> ParseError {
    if matches!(
        error.kind(),
        io::ErrorKind::TimedOut | io::ErrorKind::WouldBlock
    ) {
        ParseError::RequestTimeout
    } else {
        ParseError::Io(error)
    }
}
