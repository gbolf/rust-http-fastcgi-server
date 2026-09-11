use crate::http::{HttpResponse, StatusCode};

use super::FastCgiError;

pub fn parse_cgi_response(output: &[u8]) -> Result<HttpResponse, FastCgiError> {
    let (header_bytes, body) = split_headers_and_body(output).ok_or(
        FastCgiError::MalformedResponse("CGI headers have no terminator"),
    )?;
    let header_text = std::str::from_utf8(header_bytes)
        .map_err(|_| FastCgiError::MalformedResponse("CGI headers are not valid UTF-8"))?;

    let normalized = header_text.replace("\r\n", "\n");
    let mut status = None;
    let mut headers = Vec::new();
    let mut has_location = false;

    // PHP produces CGI headers, not an HTTP status line. Only a small safe
    // response-header subset is copied into the HTTP response.
    for line in normalized.split('\n') {
        if line.is_empty() {
            continue;
        }
        let (name, value) = line
            .split_once(':')
            .ok_or(FastCgiError::MalformedResponse("malformed CGI header"))?;
        let name = name.trim();
        let value = value.trim_matches([' ', '\t']);
        if name.is_empty() || value.bytes().any(|byte| matches!(byte, b'\r' | b'\n' | 0)) {
            return Err(FastCgiError::MalformedResponse("invalid CGI header"));
        }

        if name.eq_ignore_ascii_case("status") {
            if status.is_some() {
                return Err(FastCgiError::MalformedResponse("duplicate Status header"));
            }
            status = Some(parse_status(value)?);
        } else if name.eq_ignore_ascii_case("content-type") {
            headers.push(("Content-Type".to_string(), value.to_string()));
        } else if name.eq_ignore_ascii_case("set-cookie") {
            headers.push(("Set-Cookie".to_string(), value.to_string()));
        } else if name.eq_ignore_ascii_case("location") {
            has_location = true;
            headers.push(("Location".to_string(), value.to_string()));
        }
        // Content-Length and hop-by-hop/server headers are intentionally ignored.
    }

    let status = status.unwrap_or(if has_location {
        StatusCode::Found
    } else {
        StatusCode::Ok
    });
    if !headers
        .iter()
        .any(|(name, _)| name.eq_ignore_ascii_case("content-type"))
    {
        headers.push((
            "Content-Type".to_string(),
            "text/html; charset=utf-8".to_string(),
        ));
    }

    Ok(HttpResponse {
        status,
        headers,
        body: body.to_vec(),
    })
}

fn split_headers_and_body(output: &[u8]) -> Option<(&[u8], &[u8])> {
    if let Some(position) = find_bytes(output, b"\r\n\r\n") {
        return Some((&output[..position], &output[position + 4..]));
    }
    find_bytes(output, b"\n\n").map(|position| (&output[..position], &output[position + 2..]))
}

fn find_bytes(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

fn parse_status(value: &str) -> Result<StatusCode, FastCgiError> {
    let (code_text, reason_text) = value.split_once(' ').unwrap_or((value, ""));
    let code = code_text
        .parse::<u16>()
        .ok()
        .filter(|code| (100..=599).contains(code))
        .ok_or(FastCgiError::MalformedResponse("invalid CGI status code"))?;
    let reason = if reason_text.trim().is_empty() {
        default_reason(code).to_string()
    } else {
        let reason = reason_text.trim();
        if reason.bytes().any(|byte| byte.is_ascii_control()) {
            return Err(FastCgiError::MalformedResponse("invalid CGI reason phrase"));
        }
        reason.to_string()
    };

    Ok(StatusCode::Custom(code, reason))
}

fn default_reason(code: u16) -> &'static str {
    match code {
        200 => "OK",
        201 => "Created",
        204 => "No Content",
        301 => "Moved Permanently",
        302 => "Found",
        303 => "See Other",
        307 => "Temporary Redirect",
        308 => "Permanent Redirect",
        400 => "Bad Request",
        403 => "Forbidden",
        404 => "Not Found",
        500 => "Internal Server Error",
        502 => "Bad Gateway",
        503 => "Service Unavailable",
        504 => "Gateway Timeout",
        _ => "CGI Response",
    }
}
