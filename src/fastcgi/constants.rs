pub const FCGI_VERSION_1: u8 = 1;
pub const FCGI_BEGIN_REQUEST: u8 = 1;
pub const FCGI_END_REQUEST: u8 = 3;
pub const FCGI_PARAMS: u8 = 4;
pub const FCGI_STDIN: u8 = 5;
pub const FCGI_STDOUT: u8 = 6;
pub const FCGI_STDERR: u8 = 7;

pub const FCGI_RESPONDER: u16 = 1;
pub const FCGI_REQUEST_COMPLETE: u8 = 0;
pub const REQUEST_ID: u16 = 1;
pub const MAX_CONTENT_LENGTH: usize = u16::MAX as usize;
