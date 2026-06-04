use openssl::error::ErrorStack;
use openssl::ex_data::Index;
use openssl::ssl::{SslConnector, SslContext, SslMethod, SslStream, SslVerifyMode, SslVersion};
use openssl_sys::{EVP_MD, SSL, SSL_CIPHER, SSL_CTX, SSL_SESSION};
use std::ffi::{c_int, c_uchar, c_void};
use std::fmt;
use std::io::{Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::sync::OnceLock;
use std::time::Duration;
use thiserror::Error;

const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
const READ_TIMEOUT: Duration = Duration::from_secs(30);
const WRITE_TIMEOUT: Duration = Duration::from_secs(30);

const TLS1_3_VERSION: c_int = 0x0304;
const TLS_AES_128_GCM_SHA256_ID: [c_uchar; 2] = [0x13, 0x01];

type PskUseSessionCallback = Option<
    unsafe extern "C" fn(
        ssl: *mut SSL,
        md: *const EVP_MD,
        id: *mut *const c_uchar,
        idlen: *mut usize,
        sess: *mut *mut SSL_SESSION,
    ) -> c_int,
>;

extern "C" {
    fn SSL_CTX_set_psk_use_session_callback(ctx: *mut SSL_CTX, cb: PskUseSessionCallback);
    fn SSL_SESSION_new() -> *mut SSL_SESSION;
    fn SSL_SESSION_free(session: *mut SSL_SESSION);
    fn SSL_SESSION_set1_master_key(
        session: *mut SSL_SESSION,
        key: *const c_uchar,
        key_len: usize,
    ) -> c_int;
    fn SSL_SESSION_set_cipher(session: *mut SSL_SESSION, cipher: *const SSL_CIPHER) -> c_int;
    fn SSL_SESSION_set_protocol_version(session: *mut SSL_SESSION, version: c_int) -> c_int;
    fn SSL_CIPHER_find(ssl: *mut SSL, ptr: *const c_uchar) -> *const SSL_CIPHER;
    fn SSL_get_SSL_CTX(ssl: *mut SSL) -> *mut SSL_CTX;
    fn SSL_CTX_get_ex_data(ctx: *const SSL_CTX, idx: c_int) -> *mut c_void;
}

static PSK_STATE_INDEX: OnceLock<Index<SslContext, PskMaterial>> = OnceLock::new();

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Endpoint {
    scheme: String,
    host: String,
    port: u16,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PskMaterial {
    identity: Vec<u8>,
    key: Vec<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HttpResponse {
    pub status: u16,
    pub reason: String,
    pub body: String,
}

#[derive(Debug, Error)]
pub enum EndpointParseError {
    #[error("Wirebody endpoints must use https:// because authentication is TLS-PSK only")]
    InvalidScheme,
}

#[derive(Debug, Error)]
pub enum HttpError {
    #[error("cannot connect to Wirebody at {0}")]
    Connect(String),
    #[error("TLS-PSK setup failed: {0}")]
    TlsSetup(String),
    #[error("TLS-PSK handshake failed: {0}")]
    TlsHandshake(String),
    #[error("HTTP write failed: {0}")]
    Write(String),
    #[error("HTTP read failed: {0}")]
    Read(String),
    #[error("invalid HTTP response from Wirebody")]
    InvalidResponse,
}

impl Endpoint {
    pub fn from_parts(
        scheme: &str,
        host: impl Into<String>,
        port: u16,
    ) -> Result<Self, EndpointParseError> {
        if scheme != "https" {
            return Err(EndpointParseError::InvalidScheme);
        }
        Ok(Self {
            scheme: scheme.to_string(),
            host: host.into(),
            port,
        })
    }

    pub fn scheme(&self) -> &str {
        &self.scheme
    }

    pub fn host(&self) -> &str {
        &self.host
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    fn authority(&self) -> String {
        let default_port = self.port == 443;
        if default_port {
            self.host.clone()
        } else {
            format!("{}:{}", self.host, self.port)
        }
    }

    fn socket_addr(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }

    fn target(&self, path_and_query: &str) -> String {
        path_and_query.to_string()
    }
}

impl fmt::Display for Endpoint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}://{}", self.scheme, self.authority())
    }
}

impl PskMaterial {
    pub fn new(identity: impl Into<Vec<u8>>, key: impl Into<Vec<u8>>) -> Self {
        Self {
            identity: identity.into(),
            key: key.into(),
        }
    }
}

pub trait HttpTransport: Send + Sync {
    fn get(&self, path_and_query: &str) -> Result<HttpResponse, HttpError>;
}

pub struct RawHttpTransport {
    endpoint: Endpoint,
    psk: PskMaterial,
}

pub struct PskHttpConnection {
    endpoint: Endpoint,
    stream: SslStream<TcpStream>,
    read_buffer: Vec<u8>,
}

impl RawHttpTransport {
    pub fn new(endpoint: Endpoint, psk: PskMaterial) -> Self {
        Self { endpoint, psk }
    }
}

impl PskHttpConnection {
    pub fn connect(endpoint: Endpoint, psk: &PskMaterial) -> Result<Self, HttpError> {
        let connector = build_connector(psk.clone())?;
        let tcp_stream = tcp_stream(&endpoint)?;
        let stream = connector
            .connect(endpoint.host(), tcp_stream)
            .map_err(|error| HttpError::TlsHandshake(error.to_string()))?;
        Ok(Self {
            endpoint,
            stream,
            read_buffer: Vec::new(),
        })
    }

    pub fn endpoint(&self) -> &Endpoint {
        &self.endpoint
    }

    pub fn get(&mut self, path_and_query: &str) -> Result<HttpResponse, HttpError> {
        let request = request_bytes(&self.endpoint, path_and_query);
        self.stream
            .write_all(&request)
            .map_err(|error| HttpError::Write(error.to_string()))?;
        read_response(&mut self.stream, &mut self.read_buffer)
    }
}

impl HttpTransport for RawHttpTransport {
    fn get(&self, path_and_query: &str) -> Result<HttpResponse, HttpError> {
        let mut connection = PskHttpConnection::connect(self.endpoint.clone(), &self.psk)?;
        connection.get(path_and_query)
    }
}

fn request_bytes(endpoint: &Endpoint, path_and_query: &str) -> Vec<u8> {
    let target = endpoint.target(path_and_query);
    format!(
        "GET {target} HTTP/1.1\r\nHost: {}\r\nAccept: application/json\r\nUser-Agent: wirebody-mcp/{}\r\nConnection: keep-alive\r\n\r\n",
        endpoint.authority(),
        env!("CARGO_PKG_VERSION")
    )
    .into_bytes()
}

fn tcp_stream(endpoint: &Endpoint) -> Result<TcpStream, HttpError> {
    let mut addrs = endpoint
        .socket_addr()
        .to_socket_addrs()
        .map_err(|_| HttpError::Connect(endpoint.to_string()))?;
    let addr = addrs
        .next()
        .ok_or_else(|| HttpError::Connect(endpoint.to_string()))?;
    let stream = TcpStream::connect_timeout(&addr, CONNECT_TIMEOUT)
        .map_err(|_| HttpError::Connect(endpoint.to_string()))?;
    stream
        .set_read_timeout(Some(READ_TIMEOUT))
        .map_err(|error| HttpError::Read(error.to_string()))?;
    stream
        .set_write_timeout(Some(WRITE_TIMEOUT))
        .map_err(|error| HttpError::Write(error.to_string()))?;
    stream
        .set_nodelay(true)
        .map_err(|error| HttpError::Connect(error.to_string()))?;
    let _ = stream.set_nonblocking(false);
    Ok(stream)
}

fn build_connector(psk: PskMaterial) -> Result<SslConnector, HttpError> {
    let mut builder = SslConnector::builder(SslMethod::tls_client())
        .map_err(|error| HttpError::TlsSetup(error.to_string()))?;
    builder.set_verify(SslVerifyMode::NONE);
    builder
        .set_min_proto_version(Some(SslVersion::TLS1_2))
        .map_err(|error| HttpError::TlsSetup(error.to_string()))?;
    builder
        .set_max_proto_version(Some(SslVersion::TLS1_3))
        .map_err(|error| HttpError::TlsSetup(error.to_string()))?;
    builder
        .set_cipher_list("PSK-AES128-GCM-SHA256")
        .map_err(|error| HttpError::TlsSetup(error.to_string()))?;
    builder
        .set_ciphersuites("TLS_AES_128_GCM_SHA256")
        .map_err(|error| HttpError::TlsSetup(error.to_string()))?;
    install_tls_psk_callbacks(&mut builder, psk)
        .map_err(|error| HttpError::TlsSetup(error.to_string()))?;
    Ok(builder.build())
}

fn read_response(
    stream: &mut SslStream<TcpStream>,
    read_buffer: &mut Vec<u8>,
) -> Result<HttpResponse, HttpError> {
    loop {
        if let Some(header_end) = header_end(read_buffer) {
            let content_length = response_content_length(&read_buffer[..header_end])?;
            let response_end = header_end + 4 + content_length;
            while read_buffer.len() < response_end {
                read_more(stream, read_buffer)?;
            }

            let response_bytes = read_buffer.drain(..response_end).collect::<Vec<_>>();
            return parse_response(&response_bytes);
        }

        read_more(stream, read_buffer)?;
    }
}

fn read_more(
    stream: &mut SslStream<TcpStream>,
    read_buffer: &mut Vec<u8>,
) -> Result<(), HttpError> {
    let mut chunk = [0_u8; 16 * 1024];
    let count = stream
        .read(&mut chunk)
        .map_err(|error| HttpError::Read(error.to_string()))?;
    if count == 0 {
        return Err(HttpError::Read(
            "connection closed before a complete HTTP response".to_string(),
        ));
    }
    read_buffer.extend_from_slice(&chunk[..count]);
    Ok(())
}

fn psk_state_index() -> Result<Index<SslContext, PskMaterial>, ErrorStack> {
    if let Some(index) = PSK_STATE_INDEX.get() {
        return Ok(*index);
    }

    let index = SslContext::new_ex_index::<PskMaterial>()?;
    let _ = PSK_STATE_INDEX.set(index);
    Ok(*PSK_STATE_INDEX
        .get()
        .expect("PSK ex-data index was just initialized"))
}

fn install_tls_psk_callbacks(
    builder: &mut openssl::ssl::SslConnectorBuilder,
    psk: PskMaterial,
) -> Result<(), ErrorStack> {
    let tls12_psk = psk.clone();
    builder.set_psk_client_callback(move |_ssl, _identity_hint, identity, key| {
        if tls12_psk.identity.len() + 1 > identity.len() || tls12_psk.key.len() > key.len() {
            return Ok(0);
        }
        identity[..tls12_psk.identity.len()].copy_from_slice(&tls12_psk.identity);
        identity[tls12_psk.identity.len()] = 0;
        key[..tls12_psk.key.len()].copy_from_slice(&tls12_psk.key);
        Ok(tls12_psk.key.len())
    });

    let index = psk_state_index()?;
    builder.set_ex_data(index, psk);
    unsafe {
        SSL_CTX_set_psk_use_session_callback(builder.as_ptr(), Some(raw_psk_use_session));
    }
    Ok(())
}

unsafe extern "C" fn raw_psk_use_session(
    ssl: *mut SSL,
    _md: *const EVP_MD,
    id: *mut *const c_uchar,
    idlen: *mut usize,
    sess: *mut *mut SSL_SESSION,
) -> c_int {
    if ssl.is_null() || id.is_null() || idlen.is_null() || sess.is_null() {
        return 0;
    }

    let ctx = SSL_get_SSL_CTX(ssl);
    if ctx.is_null() {
        return 0;
    }
    let Some(index) = PSK_STATE_INDEX.get() else {
        return 0;
    };
    let state = SSL_CTX_get_ex_data(ctx, index.as_raw()) as *const PskMaterial;
    if state.is_null() {
        return 0;
    }
    let state = &*state;

    let session = SSL_SESSION_new();
    if session.is_null() {
        return 0;
    }

    if SSL_SESSION_set1_master_key(session, state.key.as_ptr(), state.key.len()) != 1 {
        SSL_SESSION_free(session);
        return 0;
    }

    let cipher = SSL_CIPHER_find(ssl, TLS_AES_128_GCM_SHA256_ID.as_ptr());
    if cipher.is_null() {
        SSL_SESSION_free(session);
        return 0;
    }
    if SSL_SESSION_set_cipher(session, cipher) != 1 {
        SSL_SESSION_free(session);
        return 0;
    }
    if SSL_SESSION_set_protocol_version(session, TLS1_3_VERSION) != 1 {
        SSL_SESSION_free(session);
        return 0;
    }

    *id = state.identity.as_ptr();
    *idlen = state.identity.len();
    *sess = session;
    1
}

pub fn parse_response(bytes: &[u8]) -> Result<HttpResponse, HttpError> {
    let header_end = header_end(bytes).ok_or(HttpError::InvalidResponse)?;
    let (head, body_with_separator) = bytes.split_at(header_end);
    let (status, reason, content_length) = parse_response_head(head)?;
    let body = &body_with_separator[4..];
    if body.len() < content_length {
        return Err(HttpError::InvalidResponse);
    }
    let body = String::from_utf8(body[..content_length].to_vec())
        .map_err(|_| HttpError::InvalidResponse)?;
    Ok(HttpResponse {
        status,
        reason,
        body,
    })
}

fn header_end(bytes: &[u8]) -> Option<usize> {
    bytes.windows(4).position(|window| window == b"\r\n\r\n")
}

fn response_content_length(head: &[u8]) -> Result<usize, HttpError> {
    let (_, _, content_length) = parse_response_head(head)?;
    Ok(content_length)
}

fn parse_response_head(head: &[u8]) -> Result<(u16, String, usize), HttpError> {
    let head = std::str::from_utf8(head).map_err(|_| HttpError::InvalidResponse)?;
    let mut lines = head.split("\r\n");
    let status_line = lines.next().ok_or(HttpError::InvalidResponse)?;
    let mut parts = status_line.splitn(3, ' ');
    let version = parts.next().ok_or(HttpError::InvalidResponse)?;
    if !version.starts_with("HTTP/") {
        return Err(HttpError::InvalidResponse);
    }
    let status = parts
        .next()
        .ok_or(HttpError::InvalidResponse)?
        .parse::<u16>()
        .map_err(|_| HttpError::InvalidResponse)?;
    let reason = parts.next().unwrap_or_default().to_string();
    let mut content_length = None;
    for line in lines {
        let Some((name, value)) = line.split_once(':') else {
            continue;
        };
        if name.trim().eq_ignore_ascii_case("content-length") {
            content_length = Some(
                value
                    .trim()
                    .parse::<usize>()
                    .map_err(|_| HttpError::InvalidResponse)?,
            );
        }
    }
    Ok((
        status,
        reason,
        content_length.ok_or(HttpError::InvalidResponse)?,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_endpoint_defaults() {
        let endpoint = Endpoint::from_parts("https", "phone.local", 5606).unwrap();
        assert_eq!(endpoint.scheme(), "https");
        assert_eq!(endpoint.host(), "phone.local");
        assert_eq!(endpoint.port(), 5606);
        assert_eq!(endpoint.target("/workouts"), "/workouts");
    }

    #[test]
    fn parses_http_response_body() {
        let response = parse_response(
            b"HTTP/1.1 404 Not Found\r\nContent-Length: 21\r\n\r\n{\"error\":\"not_found\"}",
        )
        .unwrap();
        assert_eq!(response.status, 404);
        assert_eq!(response.reason, "Not Found");
        assert_eq!(response.body, r#"{"error":"not_found"}"#);
    }

    #[test]
    fn parses_only_declared_content_length_for_keepalive() {
        let response = parse_response(
            b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: keep-alive\r\n\r\n{}HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\n[]",
        )
        .unwrap();
        assert_eq!(response.status, 200);
        assert_eq!(response.body, "{}");
    }
}
