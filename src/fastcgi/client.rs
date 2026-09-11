use std::{
    io::{self, Read, Write},
    net::{SocketAddr, TcpStream, ToSocketAddrs},
    path::Path,
    time::Duration,
};

#[cfg(unix)]
use std::os::unix::net::UnixStream;

use crate::{
    config::PhpFpmEndpoint,
    http::{HttpRequest, HttpResponse, Method},
};

use super::{
    constants::{
        FCGI_BEGIN_REQUEST, FCGI_END_REQUEST, FCGI_PARAMS, FCGI_REQUEST_COMPLETE, FCGI_RESPONDER,
        FCGI_STDERR, FCGI_STDIN, FCGI_STDOUT, REQUEST_ID,
    },
    encode_param, parse_cgi_response, read_record, write_record, FastCgiError,
};

pub struct FastCgiRequest<'a> {
    pub http: &'a HttpRequest,
    pub script_filename: &'a Path,
    pub document_root: &'a Path,
    pub remote_address: &'a str,
    pub server_port: &'a str,
}

pub fn execute(
    endpoint: &PhpFpmEndpoint,
    request: FastCgiRequest<'_>,
    read_timeout: Duration,
    write_timeout: Duration,
) -> Result<HttpResponse, FastCgiError> {
    let mut connection = connect(endpoint, read_timeout, write_timeout)?;
    exchange(&mut connection, request)
}

enum FpmConnection {
    Tcp(TcpStream),
    #[cfg(unix)]
    Unix(UnixStream),
}

impl Read for FpmConnection {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        match self {
            Self::Tcp(stream) => stream.read(buffer),
            #[cfg(unix)]
            Self::Unix(stream) => stream.read(buffer),
        }
    }
}

impl Write for FpmConnection {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        match self {
            Self::Tcp(stream) => stream.write(buffer),
            #[cfg(unix)]
            Self::Unix(stream) => stream.write(buffer),
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        match self {
            Self::Tcp(stream) => stream.flush(),
            #[cfg(unix)]
            Self::Unix(stream) => stream.flush(),
        }
    }
}

fn connect(
    endpoint: &PhpFpmEndpoint,
    read_timeout: Duration,
    write_timeout: Duration,
) -> Result<FpmConnection, FastCgiError> {
    match endpoint {
        PhpFpmEndpoint::Tcp(address) => {
            let socket_address = resolve_first(address)?;
            let stream = TcpStream::connect_timeout(&socket_address, write_timeout)?;
            stream.set_read_timeout(Some(read_timeout))?;
            stream.set_write_timeout(Some(write_timeout))?;
            Ok(FpmConnection::Tcp(stream))
        }
        #[cfg(unix)]
        PhpFpmEndpoint::Unix(path) => {
            let stream = UnixStream::connect(path)?;
            stream.set_read_timeout(Some(read_timeout))?;
            stream.set_write_timeout(Some(write_timeout))?;
            Ok(FpmConnection::Unix(stream))
        }
    }
}

fn resolve_first(address: &str) -> Result<SocketAddr, FastCgiError> {
    address
        .to_socket_addrs()?
        .next()
        .ok_or(FastCgiError::MalformedResponse(
            "PHP-FPM address resolved to nothing",
        ))
}

fn exchange<S: Read + Write>(
    stream: &mut S,
    request: FastCgiRequest<'_>,
) -> Result<HttpResponse, FastCgiError> {
    let role = FCGI_RESPONDER.to_be_bytes();
    let begin_body = [role[0], role[1], 0, 0, 0, 0, 0, 0];
    write_record(stream, FCGI_BEGIN_REQUEST, REQUEST_ID, &begin_body)?;

    let params = build_params(&request)?;
    write_record(stream, FCGI_PARAMS, REQUEST_ID, &params)?;

    write_record(stream, FCGI_PARAMS, REQUEST_ID, &[])?;
    if matches!(request.http.method, Method::Post) && !request.http.body.is_empty() {
        write_record(stream, FCGI_STDIN, REQUEST_ID, &request.http.body)?;
    }
    write_record(stream, FCGI_STDIN, REQUEST_ID, &[])?;
    stream.flush()?;

    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    loop {
        let record = read_record(stream)?;
        if record.request_id != REQUEST_ID {
            return Err(FastCgiError::UnexpectedRequestId(record.request_id));
        }
        match record.record_type {
            FCGI_STDOUT => stdout.extend_from_slice(&record.content),
            FCGI_STDERR => stderr.extend_from_slice(&record.content),
            FCGI_END_REQUEST => {
                parse_end_request(&record.content)?;
                break;
            }
            _ => {}
        }
    }

    if !stderr.is_empty() {
        eprintln!("PHP-FPM stderr: {}", String::from_utf8_lossy(&stderr));
    }
    parse_cgi_response(&stdout)
}

fn build_params(request: &FastCgiRequest<'_>) -> Result<Vec<u8>, FastCgiError> {
    let script_filename = request
        .script_filename
        .to_str()
        .ok_or(FastCgiError::MalformedResponse("script path is not UTF-8"))?;
    let document_root = request
        .document_root
        .to_str()
        .ok_or(FastCgiError::MalformedResponse(
            "document root is not UTF-8",
        ))?;
    let host = request.http.header("Host").unwrap_or("");
    let server_name = host_name(host);
    let content_type = request.http.header("Content-Type").unwrap_or("");
    let content_length = if matches!(request.http.method, Method::Post) {
        request.http.body.len()
    } else {
        0
    }
    .to_string();
    let mut output = Vec::new();

    let fixed = [
        ("GATEWAY_INTERFACE", "CGI/1.1"),
        ("SERVER_SOFTWARE", "thesis-rust-server/0.1"),
        ("SERVER_PROTOCOL", request.http.version.as_str()),
        ("REQUEST_METHOD", request.http.method.as_str()),
        ("REQUEST_URI", request.http.request_target.as_str()),
        ("SCRIPT_NAME", request.http.path.as_str()),
        ("SCRIPT_FILENAME", script_filename),
        ("QUERY_STRING", request.http.query_string.as_str()),
        ("DOCUMENT_ROOT", document_root),
        ("SERVER_NAME", server_name),
        ("SERVER_PORT", request.server_port),
        ("REMOTE_ADDR", request.remote_address),
        ("CONTENT_TYPE", content_type),
        ("CONTENT_LENGTH", content_length.as_str()),
    ];
    for (name, value) in fixed {
        encode_param(name, value, &mut output);
    }

    // Only the selected incoming fields requested by this project are converted
    // to CGI's HTTP_* convention. Content fields have dedicated CGI variables.
    for header in ["Host", "User-Agent", "Accept", "Cookie", "Referer"] {
        if let Some(value) = request.http.header(header) {
            let cgi_name = format!("HTTP_{}", header.to_ascii_uppercase().replace('-', "_"));
            encode_param(&cgi_name, value, &mut output);
        }
    }
    Ok(output)
}

fn host_name(host: &str) -> &str {
    if let Some(rest) = host.strip_prefix('[') {
        return rest.split_once(']').map_or(host, |(name, _)| name);
    }
    host.rsplit_once(':').map_or(host, |(name, port)| {
        if port.bytes().all(|byte| byte.is_ascii_digit()) {
            name
        } else {
            host
        }
    })
}

fn parse_end_request(content: &[u8]) -> Result<(), FastCgiError> {
    if content.len() != 8 {
        return Err(FastCgiError::MalformedResponse("invalid END_REQUEST body"));
    }
    let application_status = u32::from_be_bytes([content[0], content[1], content[2], content[3]]);
    let protocol_status = content[4];
    if protocol_status != FCGI_REQUEST_COMPLETE {
        return Err(FastCgiError::ProtocolStatus(protocol_status));
    }
    if application_status != 0 {
        return Err(FastCgiError::ApplicationStatus(application_status));
    }
    Ok(())
}
