use super::Method;

#[derive(Clone, Debug)]
pub struct HttpRequest {
    pub method: Method,
    pub request_target: String,
    pub path: String,
    pub query_string: String,
    pub version: String,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl HttpRequest {
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(header_name, _)| header_name.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.as_str())
    }
}
