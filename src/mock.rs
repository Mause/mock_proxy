use crate::Request;
use http::status::StatusCode;
use std::convert::TryInto;

#[derive(Debug, Clone)]
pub struct Response {
    pub(super) headers: Vec<(String, String)>,
    pub(super) body: Vec<u8>,
    pub(super) status: StatusCode,
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
    pub(super) path: String,
    /// The HTTP method to match again
    pub(super) method: String,
    /// The response to return
    pub(super) response: Response,
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
    /// # Errors
    /// If we fail to read the file from disk
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

    /// Adds a header to the response
    ///
    /// Does not remove existing headers with the same name
    pub fn with_header<K, V>(&mut self, name: K, value: V) -> &mut Self
    where
        K: ToString,
        V: ToString,
    {
        self.response
            .headers
            .push((name.to_string(), value.to_string()));
        self
    }

    /// Sets the response status
    /// Default is 200 OK
    pub fn with_status<T>(&mut self, status: T) -> &mut Self
    where
        T: TryInto<http::StatusCode>,
    {
        self.response.status = match status.try_into() {
            Ok(status) => status,
            Err(_) => panic!("Bad status"),
        };
        self
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
