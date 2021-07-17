use crate::Request;
use http::status::StatusCode;

#[derive(Debug, Clone)]
pub struct Response {
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
    pub status: StatusCode,
}
impl Default for Response {
    fn default() -> Self {
        Self {
            body: Vec::new(),
            headers: Vec::new(),
            status: http::StatusCode::default(),
        }
    }
}

/// The struct used to define mock responses
#[derive(Debug, Clone)]
pub struct Mock {
    /// The path to match again
    pub path: String,
    /// The HTTP method to match again
    pub method: String,
    /// The response to return
    pub response: Response,
}
impl Mock {
    /// Builds a [`Mock`] with the given `method` and `path` and a [`Default`] [`Response`]
    pub fn new(method: &str, path: &str) -> Self {
        Self {
            method: method.to_string(),
            path: path.to_string(),
            response: Response::default(),
        }
    }

    /// Reads the response body from disk
    pub fn with_body_from_file(
        &mut self,
        filename: &str,
    ) -> Result<&mut Self, Box<dyn std::error::Error>> {
        self.response.body = std::fs::read(filename)?;
        Ok(self)
    }

    /// Sets the response body to anything with [`Into<json::JsonValue>`]
    pub fn with_body_from_json<T>(
        &mut self,
        value: T,
    ) -> Result<&mut Self, Box<dyn std::error::Error>>
    where
        T: Into<json::JsonValue>,
    {
        self.response.body = json::stringify_pretty(value, 2).as_bytes().to_vec();
        Ok(self)
    }

    /// Freezes the given [`Mock`]
    pub fn create(&self) -> Self {
        self.clone()
    }

    pub(super) fn matches(&self, request: &Request) -> bool {
        &self.path == request.path.as_ref().unwrap()
            && &self.method == request.method.as_ref().unwrap()
    }
}
