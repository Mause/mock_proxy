#![deny(missing_docs)]
#![deny(clippy::nursery)]

//! This library was built to help test systems that use libraries which don't provide any
//! testing utilities themselves. It works by overriding the proxy and root ca attributes
//! and intercepting proxy requests, then returning mock responses defined by the user
//!
//! The following shows how to setup reqwest to send requests to a [`Proxy`] instance: [simple_test](https://github.com/Mause/mock_proxy/blob/main/src/test.rs)

use crate::identity::OpensslInterface;
use crate::identity_interface::{Cert, IdentityInterface};
use crate::mock::{split_url, Response};
use log::{error, info};
use native_tls::TlsStream;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::mpsc;
use std::thread;

mod identity;
mod identity_interface;
mod mock;
#[cfg(test)]
mod test;
pub use crate::mock::Mock;

const SERVER_ADDRESS_INTERNAL: &str = "127.0.0.1:1234";

/// Primary interface for the library
pub struct Proxy {
    mocks: Vec<Mock>,
    listening_addr: Option<SocketAddr>,
    started: bool,
    cert: Cert,
}

impl Default for Proxy {
    fn default() -> Self {
        let cert = OpensslInterface::new()
            .mk_ca_cert()
            .expect("Failed to generate CA certificate");
        Self {
            mocks: Vec::new(),
            listening_addr: None,
            started: false,
            cert,
        }
    }
}

impl Proxy {
    /// Builds a [`Default`] instance
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a given mock with the proxy
    ///
    /// # Panics
    /// Will panic if proxy has already been started
    pub fn register(&mut self, mock: Mock) {
        if self.started {
            panic!("Cannot add mocks to a started proxy");
        }
        self.mocks.push(mock);
    }

    /// Start the proxy server
    ///
    /// # Panics
    /// Will panic if proxy has already been started
    pub fn start(&mut self) {
        start_proxy(self);
    }

    /// Start the server
    /// # Panics
    /// Not supported yet
    pub fn stop(&mut self) {
        todo!();
    }

    /// Address and port of the local server.
    /// Can be used with [`std::net::TcpStream`].
    ///
    /// # Panics
    /// If server is not running
    pub fn address(&self) -> SocketAddr {
        self.listening_addr.expect("server should be listening")
    }

    /// A local `http://…` URL of the server.
    ///
    /// # Panics
    /// If server is not running
    pub fn url(&self) -> String {
        format!("http://{}", self.address())
    }

    /// Returns the root CA certificate of the server
    ///
    /// # Panics
    /// If PEM conversion fails
    pub fn get_certificate(&self) -> Vec<u8> {
        self.cert.cert()
    }
}

#[derive(Debug, Clone)]
struct Request {
    error: Option<String>,
    host: Option<String>,
    path: Option<String>,
    method: Option<String>,
    version: (u8, u8),
}

impl std::fmt::Display for Request {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        f.debug_struct("Request")
            .field("method", &self.method)
            .field("host", &self.host)
            .field("path", &self.path)
            .finish()
    }
}
impl Request {
    const fn is_ok(&self) -> bool {
        self.error().is_none()
    }
    const fn error(&self) -> Option<&String> {
        self.error.as_ref()
    }

    fn from(stream: &mut dyn Read) -> Self {
        let mut request = Self {
            error: None,
            host: None,
            path: None,
            method: None,
            version: (0, 0),
        };

        let mut all_buf = Vec::new();

        loop {
            let mut buf = [0; 1024];

            let rlen = match stream.read(&mut buf) {
                Err(e) => Err(e.to_string()),
                Ok(0) => Err("Nothing to read.".into()),
                Ok(i) => Ok(i),
            }
            .map_err(|e| request.error = Some(e))
            .unwrap_or(0);
            if request.error().is_some() {
                break;
            }

            all_buf.extend_from_slice(&buf[..rlen]);

            if rlen < 1024 {
                break;
            }
        }

        if request.error().is_some() {
            return request;
        }

        let mut headers = [httparse::EMPTY_HEADER; 16];
        let mut req = httparse::Request::new(&mut headers);

        let _ = req
            .parse(&all_buf)
            .map_err(|err| {
                request.error = Some(err.to_string());
            })
            .map(|result| match result {
                httparse::Status::Complete(_head_length) => {
                    request.method = req.method.map(|s| s.to_string());

                    if req.method.as_ref().unwrap().eq(&"CONNECT") {
                        request.host = req.path.unwrap().split(':').next().map(|f| f.to_string());
                    } else {
                        let (host, path) = split_url(
                            &req.path
                                .map(|f| f.to_string())
                                .expect("Missing path in request"),
                        );
                        request.host = host;
                        request.path = Some(path);
                    }

                    if let Some(a @ 0..=1) = req.version {
                        request.version = (1, a);
                    }
                }
                httparse::Status::Partial => panic!("Incomplete request"),
            });

        request
    }
}

fn create_identity(cn: &str, pair: &Cert) -> native_tls::Identity {
    let password = "password";

    let encrypted = OpensslInterface::new()
        .mk_ca_signed_cert(cn, pair, password)
        .unwrap();

    native_tls::Identity::from_pkcs12(&encrypted, password).expect("Unable to build identity")
}

fn start_proxy(proxy: &mut Proxy) {
    if proxy.started {
        panic!("Tried to start an already started proxy");
    }
    proxy.started = true;
    let mocks = proxy.mocks.clone();
    let cert = proxy.cert.clone();

    // if state.listening_addr.is_some() {
    //     return;
    // }

    let (tx, rx) = mpsc::channel();

    thread::spawn(move || {
        let res = TcpListener::bind(SERVER_ADDRESS_INTERNAL).or_else(|err| {
            error!("TcpListener::bind: {}", err);
            TcpListener::bind("127.0.0.1:0")
        });
        let (listener, addr) = match res {
            Ok(listener) => {
                let addr = listener.local_addr().unwrap();
                tx.send(Some(addr)).unwrap();
                (listener, addr)
            }
            Err(err) => {
                error!("alt bind: {}", err);
                tx.send(None).unwrap();
                return;
            }
        };

        info!("Server is listening at {}", addr);
        for stream in listener.incoming() {
            info!("Got stream: {:?}", stream);
            if let Ok(mut stream) = stream {
                let request = Request::from(&mut stream);
                info!("Request received: {}", request);
                if request.is_ok() {
                    handle_request(cert.to_owned(), &mocks, request, stream).unwrap();
                } else {
                    let message = request
                        .error()
                        .map_or("Could not parse the request.", |err| err.as_str());
                    error!("Could not parse request because: {}", message);
                    respond_with_error(&mut stream as &mut dyn Write, &request, message).unwrap();
                }
            } else {
                error!("Could not read from stream");
            }
        }
    });

    proxy.listening_addr = rx.recv().ok().and_then(|addr| addr);
}

fn open_tunnel<'a>(
    identity: Cert,
    request: &Request,
    stream: &'a mut TcpStream,
) -> Result<TlsStream<&'a mut TcpStream>, Box<dyn std::error::Error>> {
    let version = request.version;
    let status = 200;

    let response = Vec::from(format!(
        "HTTP/{}.{} {}\r\n\r\n",
        version.0, version.1, status
    ));

    stream.write_all(&response)?;
    stream.flush()?;
    info!("Tunnel open response written");

    let identity = create_identity(request.host.as_ref().expect("No host??"), &identity);

    info!("Wrapping with tls");
    let tstream = native_tls::TlsAcceptor::builder(identity)
        .build()
        .expect("Unable to build acceptor")
        .accept(stream)
        .expect("Unable to accept connection");
    info!("Wrapped: {:?}", tstream);

    Ok(tstream)
}

fn handle_request(
    identity: Cert,
    mocks: &[Mock],
    request: Request,
    mut stream: TcpStream,
) -> Result<(), Box<dyn std::error::Error>> {
    if request.method.as_ref().unwrap().eq("CONNECT") {
        let mut tea = open_tunnel(identity, &request, &mut stream)?;

        let mut req = Request::from(&mut tea);
        req.host = request.host;
        if !req.is_ok() {
            return Err(req.error().unwrap().as_str().into());
        };

        // TODO: should probably loop reading of requests here for #23
        _handle_request(&mut tea, req, mocks)
    } else {
        _handle_request(&mut stream, request, mocks)
    }
}

fn _handle_request<S: Read + Write>(
    tstream: &mut S,
    req: Request,
    mocks: &[Mock],
) -> Result<(), Box<dyn std::error::Error>> {
    let mut matched = false;
    for m in mocks {
        if m.matches(&req) {
            write_response(tstream, &req, &m.response)?;
            matched = true;
            break;
        }
    }

    if !matched {
        respond_with_error(tstream, &req, "No matching response")?;
    }

    Ok(())
}

fn write_response(
    tstream: &mut dyn Write,
    request: &Request,
    response: &Response,
) -> Result<(), Box<dyn std::error::Error>> {
    tstream.write_fmt(format_args!(
        "HTTP/1.{} {}\r\n",
        request.version.1, response.status
    ))?;
    for (header, value) in &response.headers {
        tstream.write_fmt(format_args!("{}: {}\r\n", header, value))?;
    }
    tstream.write_all(b"\r\n")?;
    tstream.write_all(&response.body)?;
    tstream.write_all(b"\r\n")?;

    Ok(())
}

fn respond_with_error(
    _stream: &mut dyn Write,
    request: &Request,
    message: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    write_response(
        _stream,
        request,
        &Response {
            headers: vec![],
            status: http::StatusCode::INTERNAL_SERVER_ERROR,
            body: message.as_bytes().to_vec(),
        },
    )
}
