//! The HTTP pre-stop call: a loopback-only `HTTP/1.1` request that follows no redirect.
//!
//! [`is_loopback`] and the call share one parse. The URL is parsed once into scheme, host and
//! port, and the connection goes to the parsed host and port with the parsed path. The raw text
//! is never matched and never used to connect, so a URL that reads as loopback to the check
//! cannot connect anywhere else. There is no TLS client, so only `http` is accepted.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use url::{Host, Position, Url};

/// How many bytes of the response the call reads to find the status line.
const STATUS_LIMIT: usize = 512;

/// A URL that passed the loopback check, with the parts a request needs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Target {
    /// The addresses to try, in order.
    pub(crate) addresses: Vec<SocketAddr>,
    /// The value of the `Host` header.
    pub(crate) host_header: String,
    /// The path and query of the request line.
    pub(crate) path_and_query: String,
}

/// Parses `url` and accepts it only if a request to it stays on this machine: scheme `http`, no
/// userinfo, port other than 0, and host `localhost`, an IPv4 address in `127.0.0.0/8` or `[::1]`.
pub(crate) fn loopback_target(url: &str) -> Option<Target> {
    let parsed = Url::parse(url).ok()?;
    if parsed.scheme() != "http" {
        return None;
    }
    if !parsed.username().is_empty()
        || parsed.password().is_some()
        || parsed.authority().contains('@')
    {
        return None;
    }
    let port = parsed.port_or_known_default().filter(|port| *port != 0)?;
    let (addresses, name) = match parsed.host()? {
        Host::Domain("localhost") => (
            vec![
                SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), port),
                SocketAddr::new(IpAddr::V6(Ipv6Addr::LOCALHOST), port),
            ],
            "localhost".to_owned(),
        ),
        Host::Ipv4(address) if address.is_loopback() => (
            vec![SocketAddr::new(IpAddr::V4(address), port)],
            address.to_string(),
        ),
        Host::Ipv6(address) if address == Ipv6Addr::LOCALHOST => (
            vec![SocketAddr::new(IpAddr::V6(address), port)],
            format!("[{address}]"),
        ),
        Host::Domain(_) | Host::Ipv4(_) | Host::Ipv6(_) => return None,
    };
    // INDEX: `Url` slices by `Position`, which always falls on character boundaries
    let path_and_query = parsed[Position::BeforePath..Position::AfterQuery].to_owned(); // BORROW: .to_owned() makes the owned path and query
    Some(Target {
        addresses,
        host_header: format!("{name}:{port}"),
        path_and_query,
    })
}

/// Whether a request to `url` stays on this machine: the scheme is `http`, there is no userinfo,
/// the port is not 0, and the host is `localhost`, an address of `127.0.0.0/8` or `[::1]`. A URL
/// that does not parse is not loopback.
#[must_use]
pub fn is_loopback(url: &str) -> bool {
    loopback_target(url).is_some()
}

/// Makes the call and says what came of it in a short phrase, within `limit`. The phrase never
/// holds the URL or the body: `http 200`, `refused: not loopback`, `http failed: connect`,
/// `http timed out`.
pub(crate) async fn call(method: &str, url: &str, body: Option<&str>, limit: Duration) -> String {
    let Some(target) = loopback_target(url) else {
        return "refused: not loopback".to_owned();
    };
    tokio::time::timeout(limit, exchange(method, &target, body))
        .await
        .unwrap_or_else(|_| "http timed out".to_owned())
}

/// Connects to the first address that answers, sends the request and reads the status line.
async fn exchange(method: &str, target: &Target, body: Option<&str>) -> String {
    let mut stream = None;
    for address in &target.addresses {
        if let Ok(connected) = TcpStream::connect(address).await {
            stream = Some(connected);
            break;
        }
    }
    let Some(mut stream) = stream else {
        return "http failed: connect".to_owned();
    };
    let mut request = format!(
        "{method} {} HTTP/1.1\r\nHost: {}\r\nConnection: close\r\n",
        target.path_and_query, target.host_header
    );
    if let Some(body) = body {
        request.push_str("Content-Length: ");
        request.push_str(&body.len().to_string());
        request.push_str("\r\n");
    }
    request.push_str("\r\n");
    if let Some(body) = body {
        request.push_str(body);
    }
    if stream.write_all(request.as_bytes()).await.is_err() {
        return "http failed: write".to_owned();
    }
    let mut seen: Vec<u8> = Vec::new();
    let mut chunk = [0_u8; 128];
    while !seen.contains(&b'\n') && seen.len() < STATUS_LIMIT {
        match stream.read(&mut chunk).await {
            Ok(0) | Err(_) => break,
            Ok(count) => seen.extend_from_slice(chunk.get(..count).unwrap_or_default()),
        }
    }
    status_phrase(&seen)
}

/// `http NNN` for a response that opens with `HTTP/1.x NNN`, and a failure phrase otherwise.
fn status_phrase(seen: &[u8]) -> String {
    let line_end = seen
        .iter()
        .position(|byte| *byte == b'\r' || *byte == b'\n')
        .unwrap_or(seen.len());
    let line = String::from_utf8_lossy(seen.get(..line_end).unwrap_or_default());
    let mut parts = line.split(' ');
    let version = parts.next().unwrap_or_default();
    let code = parts.next().unwrap_or_default();
    let valid = version.starts_with("HTTP/1.")
        && code.len() == 3
        && code.bytes().all(|byte| byte.is_ascii_digit());
    if valid {
        format!("http {code}")
    } else {
        "http failed: no status line".to_owned()
    }
}
