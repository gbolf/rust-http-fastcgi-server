use std::io::{self, Write};

use super::StatusCode;

pub const SERVER_NAME: &str = "thesis-rust-server/0.1";

#[derive(Clone, Debug)]
pub struct HttpResponse {
    pub status: StatusCode,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl HttpResponse {
    pub fn new(status: StatusCode, content_type: &str, body: Vec<u8>) -> Self {
        Self {
            status,
            headers: vec![("Content-Type".to_string(), content_type.to_string())],
            body,
        }
    }

    pub fn error(status: StatusCode) -> Self {
        let body = format!(
            "<!doctype html><html><head><meta charset=\"utf-8\"><title>{} {}</title></head>\
             <body><h1>{} {}</h1></body></html>\n",
            status.code(),
            status.reason(),
            status.code(),
            status.reason()
        )
        .into_bytes();
        Self::new(status, "text/html; charset=utf-8", body)
    }

    pub fn write_to<W: Write>(&self, writer: &mut W, omit_body: bool) -> io::Result<()> {
        write!(
            writer,
            "HTTP/1.1 {} {}\r\n",
            self.status.code(),
            self.status.reason()
        )?;

        for (name, value) in &self.headers {
            if !is_server_managed_header(name) {
                write!(writer, "{name}: {value}\r\n")?;
            }
        }

        write!(
            writer,
            "Server: {SERVER_NAME}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            self.body.len()
        )?;

        if !omit_body {
            writer.write_all(&self.body)?;
        }
        writer.flush()
    }
}

fn is_server_managed_header(name: &str) -> bool {
    [
        "connection",
        "transfer-encoding",
        "content-length",
        "server",
    ]
    .iter()
    .any(|blocked| name.eq_ignore_ascii_case(blocked))
}
