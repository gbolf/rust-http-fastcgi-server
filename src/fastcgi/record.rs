use std::{
    fmt,
    io::{self, Read, Write},
};

use super::constants::{FCGI_VERSION_1, MAX_CONTENT_LENGTH};

#[derive(Debug, PartialEq, Eq)]
pub struct FastCgiRecord {
    pub record_type: u8,
    pub request_id: u16,
    pub content: Vec<u8>,
}

#[derive(Debug)]
pub enum FastCgiError {
    Timeout,
    Io(io::Error),
    TruncatedHeader,
    TruncatedContent,
    InvalidVersion(u8),
    UnexpectedRequestId(u16),
    MalformedResponse(&'static str),
    ProtocolStatus(u8),
    ApplicationStatus(u32),
}

impl fmt::Display for FastCgiError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Timeout => write!(formatter, "FastCGI communication timed out"),
            Self::Io(error) => write!(formatter, "FastCGI I/O failed: {error}"),
            Self::TruncatedHeader => write!(formatter, "truncated FastCGI header"),
            Self::TruncatedContent => write!(formatter, "truncated FastCGI content or padding"),
            Self::InvalidVersion(version) => write!(formatter, "invalid FastCGI version {version}"),
            Self::UnexpectedRequestId(id) => {
                write!(formatter, "unexpected FastCGI request ID {id}")
            }
            Self::MalformedResponse(message) => {
                write!(formatter, "malformed FastCGI response: {message}")
            }
            Self::ProtocolStatus(status) => write!(formatter, "FastCGI protocol status {status}"),
            Self::ApplicationStatus(status) => {
                write!(formatter, "FastCGI application status {status}")
            }
        }
    }
}

impl std::error::Error for FastCgiError {}

impl From<io::Error> for FastCgiError {
    fn from(error: io::Error) -> Self {
        if matches!(
            error.kind(),
            io::ErrorKind::TimedOut | io::ErrorKind::WouldBlock
        ) {
            Self::Timeout
        } else {
            Self::Io(error)
        }
    }
}

pub fn write_record<W: Write>(
    writer: &mut W,
    record_type: u8,
    request_id: u16,
    content: &[u8],
) -> io::Result<()> {
    if content.is_empty() {
        return write_one_record(writer, record_type, request_id, content);
    }

    // content_length is only 16 bits, so a larger logical stream is represented
    // by consecutive records of the same type.
    for chunk in content.chunks(MAX_CONTENT_LENGTH) {
        write_one_record(writer, record_type, request_id, chunk)?;
    }
    Ok(())
}

fn write_one_record<W: Write>(
    writer: &mut W,
    record_type: u8,
    request_id: u16,
    content: &[u8],
) -> io::Result<()> {
    let request_id_bytes = request_id.to_be_bytes();
    let content_length = u16::try_from(content.len())
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "FastCGI record is too large"))?;
    let content_length_bytes = content_length.to_be_bytes();

    let header = [
        FCGI_VERSION_1,
        record_type,
        request_id_bytes[0],
        request_id_bytes[1],
        content_length_bytes[0],
        content_length_bytes[1],
        0, // outgoing padding_length
        0, // reserved
    ];
    writer.write_all(&header)?;
    writer.write_all(content)
}

pub fn read_record<R: Read>(reader: &mut R) -> Result<FastCgiRecord, FastCgiError> {
    let mut header = [0_u8; 8];
    read_exact_classified(reader, &mut header, FastCgiError::TruncatedHeader)?;
    if header[0] != FCGI_VERSION_1 {
        return Err(FastCgiError::InvalidVersion(header[0]));
    }

    let request_id = u16::from_be_bytes([header[2], header[3]]);
    let content_length = u16::from_be_bytes([header[4], header[5]]) as usize;
    let padding_length = header[6] as usize;
    let mut content = vec![0_u8; content_length];
    read_exact_classified(reader, &mut content, FastCgiError::TruncatedContent)?;
    let mut padding = vec![0_u8; padding_length];
    read_exact_classified(reader, &mut padding, FastCgiError::TruncatedContent)?;

    Ok(FastCgiRecord {
        record_type: header[1],
        request_id,
        content,
    })
}

fn read_exact_classified<R: Read>(
    reader: &mut R,
    buffer: &mut [u8],
    eof_error: FastCgiError,
) -> Result<(), FastCgiError> {
    match reader.read_exact(buffer) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::UnexpectedEof => Err(eof_error),
        Err(error) => Err(error.into()),
    }
}
