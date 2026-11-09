//! Job-local loopback proxy. Connections use only the resolved public IPs checked here.
use std::{
    io::{self, BufRead, BufReader, Read, Write},
    net::{IpAddr, Shutdown, SocketAddr, TcpListener, TcpStream, ToSocketAddrs},
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc,
    },
    thread,
    time::{Duration, Instant},
};
use url::Url;

const MAX_HEADER: usize = 16 * 1024;
const MAX_BYTES: usize = 32 * 1024 * 1024;
const MAX_CONNECTIONS: usize = 12;
const MAX_REQUESTS: usize = 96;

#[derive(Default)]
struct Limits {
    stopped: AtomicBool,
    blocked: AtomicBool,
    connections: AtomicUsize,
    requests: AtomicUsize,
    bytes: AtomicUsize,
}
pub struct Proxy {
    endpoint: String,
    limits: Arc<Limits>,
    listener: Option<thread::JoinHandle<()>>,
}
impl Proxy {
    pub fn start() -> io::Result<Self> {
        let listener = TcpListener::bind(("127.0.0.1", 0))?;
        listener.set_nonblocking(true)?;
        let endpoint = format!("http://{}", listener.local_addr()?);
        let limits = Arc::new(Limits::default());
        let worker = Arc::clone(&limits);
        let listener = thread::Builder::new().name("analysis-proxy".into()).spawn(move || {
            while !worker.stopped.load(Ordering::Relaxed) {
                match listener.accept() {
                    Ok((mut stream, _)) => {
                        let requests = worker.requests.fetch_add(1, Ordering::Relaxed);
                        if requests >= MAX_REQUESTS || worker.connections.load(Ordering::Relaxed) >= MAX_CONNECTIONS {
                            let _ = stream.set_write_timeout(Some(Duration::from_millis(200)));
                            let _ = stream.write_all(b"HTTP/1.1 503 Service Unavailable\r\nConnection: close\r\nContent-Length: 0\r\n\r\n");
                            continue;
                        }
                        let job = Arc::clone(&worker);
                        job.connections.fetch_add(1, Ordering::Relaxed);
                        thread::spawn(move || {
                            let _ = serve(stream, &job);
                            job.connections.fetch_sub(1, Ordering::Relaxed);
                        });
                    }
                    Err(error) if error.kind() == io::ErrorKind::WouldBlock => thread::sleep(Duration::from_millis(10)),
                    Err(_) => break,
                }
            }
        })?;
        Ok(Self {
            endpoint,
            limits,
            listener: Some(listener),
        })
    }
    pub fn endpoint(&self) -> &str {
        &self.endpoint
    }
    pub fn blocked(&self) -> bool {
        self.limits.blocked.load(Ordering::Relaxed)
    }
}
impl Drop for Proxy {
    fn drop(&mut self) {
        self.limits.stopped.store(true, Ordering::Relaxed);
        if let Some(listener) = self.listener.take() {
            let _ = listener.join();
        }
    }
}

pub fn target(input: &str) -> Option<Url> {
    if !dm_video_protocol::public_url(input) {
        return None;
    }
    let url = Url::parse(input).ok()?;
    if url.port().is_some() {
        return None;
    }
    Some(url)
}
fn connect_target(authority: &str) -> Option<Url> {
    if !authority.ends_with(":443") || authority.contains(['/', '\\', '@', '?', '#']) {
        return None;
    }
    target(&format!("https://{authority}/"))
}
fn public_address(address: IpAddr) -> bool {
    match address {
        IpAddr::V4(ip) => {
            let [a, b, c, _] = ip.octets();
            a != 0
                && a < 224
                && !ip.is_loopback()
                && !ip.is_private()
                && !ip.is_link_local()
                && !ip.is_documentation()
                && !(a == 100 && (64..=127).contains(&b)
                    || a == 192 && b == 0 && c == 0
                    || a == 198 && matches!(b, 18 | 19))
        }
        IpAddr::V6(ip) => {
            let [a, b, ..] = ip.segments();
            a & 0xe000 == 0x2000
                && a != 0x2002
                && !(a == 0x2001 && (b <= 0x01ff || b == 0x0db8))
                && !(a == 0x3fff && b < 0x1000)
        }
    }
}
fn resolve(url: &Url) -> io::Result<Vec<SocketAddr>> {
    let host = url.host_str().unwrap_or_default().trim_matches(['[', ']']);
    let port = url.port_or_known_default().unwrap_or_default();
    (host, port).to_socket_addrs().map(|items| items.collect())
}
fn approved_addresses(addresses: &[SocketAddr]) -> bool {
    !addresses.is_empty() && addresses.iter().all(|address| public_address(address.ip()))
}

fn deny(client: &mut TcpStream, limits: &Limits) -> io::Result<()> {
    limits.blocked.store(true, Ordering::Relaxed);
    client.write_all(b"HTTP/1.1 403 Forbidden\r\nConnection: close\r\nContent-Length: 0\r\n\r\n")
}
fn header(reader: &mut BufReader<TcpStream>) -> io::Result<Vec<u8>> {
    let mut data = Vec::new();
    while data.len() < MAX_HEADER {
        let start = data.len();
        let n = reader
            .by_ref()
            .take((MAX_HEADER - start) as u64)
            .read_until(b'\n', &mut data)?;
        if n == 0 || !data.ends_with(b"\r\n") {
            break;
        }
        if &data[start..] == b"\r\n" {
            return Ok(data);
        }
    }
    Err(io::ErrorKind::InvalidData.into())
}

fn serve(stream: TcpStream, limits: &Limits) -> io::Result<()> {
    stream.set_nonblocking(false)?;
    stream.set_read_timeout(Some(Duration::from_secs(2)))?;
    stream.set_write_timeout(Some(Duration::from_secs(2)))?;
    let mut client = BufReader::new(stream);
    let data = header(&mut client)?;
    let mut headers = [httparse::EMPTY_HEADER; 64];
    let mut request = httparse::Request::new(&mut headers);
    if !request
        .parse(&data)
        .is_ok_and(|status| status.is_complete())
    {
        return deny(client.get_mut(), limits);
    }
    let (Some(method), Some(path)) = (request.method, request.path) else {
        return deny(client.get_mut(), limits);
    };
    let url = match method {
        "CONNECT" => connect_target(path),
        "GET" | "HEAD" => target(path).filter(|url| url.scheme() == "http"),
        _ => None,
    };
    let Some(url) = url else {
        return deny(client.get_mut(), limits);
    };
    if request.headers.iter().any(|header| {
        header.name.eq_ignore_ascii_case("transfer-encoding")
            || (header.name.eq_ignore_ascii_case("content-length") && header.value != b"0")
    }) {
        return deny(client.get_mut(), limits);
    }
    let addresses = match resolve(&url) {
        Ok(addresses) => addresses,
        Err(_) => return unavailable(client.get_mut()),
    };
    if !approved_addresses(&addresses) {
        return deny(client.get_mut(), limits);
    }
    let Some(mut remote) = addresses
        .iter()
        .take(4)
        .find_map(|address| TcpStream::connect_timeout(address, Duration::from_secs(2)).ok())
    else {
        return unavailable(client.get_mut());
    };
    remote.set_read_timeout(Some(Duration::from_millis(500)))?;
    remote.set_write_timeout(Some(Duration::from_secs(2)))?;
    client
        .get_ref()
        .set_read_timeout(Some(Duration::from_millis(500)))?;
    if method == "CONNECT" {
        client
            .get_mut()
            .write_all(b"HTTP/1.1 200 Connection Established\r\n\r\n")?;
        let mut client_writer = client.get_ref().try_clone()?;
        let mut remote_reader = remote.try_clone()?;
        thread::scope(|scope| {
            let back = scope.spawn(|| {
                relay(&mut remote_reader, &mut client_writer, limits);
                let _ = client_writer.shutdown(Shutdown::Write);
            });
            relay(&mut client, &mut remote, limits);
            let _ = remote.shutdown(Shutdown::Write);
            let _ = back.join();
        });
    } else {
        let path = match url.query() {
            Some(query) => format!("{}?{query}", url.path()),
            None => url.path().to_owned(),
        };
        write!(
            remote,
            "{method} {path} HTTP/1.1\r\nHost: {}\r\nConnection: close\r\n",
            url.host_str().unwrap_or_default()
        )?;
        for header in request.headers.iter() {
            if ["user-agent", "accept", "accept-language", "range"]
                .iter()
                .any(|name| header.name.eq_ignore_ascii_case(name))
            {
                write!(remote, "{}: ", header.name)?;
                remote.write_all(header.value)?;
                remote.write_all(b"\r\n")?;
            }
        }
        remote.write_all(b"\r\n")?;
        relay(&mut remote, client.get_mut(), limits);
    }
    Ok(())
}
fn unavailable(client: &mut TcpStream) -> io::Result<()> {
    client.write_all(b"HTTP/1.1 502 Bad Gateway\r\nConnection: close\r\nContent-Length: 0\r\n\r\n")
}
fn relay(reader: &mut impl Read, writer: &mut impl Write, limits: &Limits) {
    let deadline = Instant::now() + Duration::from_secs(35);
    let mut buffer = [0; 8192];
    while !limits.stopped.load(Ordering::Relaxed) && Instant::now() < deadline {
        match reader.read(&mut buffer) {
            Ok(0) => break,
            Ok(n) => {
                if limits.bytes.fetch_add(n, Ordering::Relaxed) + n > MAX_BYTES
                    || writer.write_all(&buffer[..n]).is_err()
                {
                    break;
                }
            }
            Err(error)
                if matches!(
                    error.kind(),
                    io::ErrorKind::TimedOut | io::ErrorKind::WouldBlock
                ) => {}
            Err(_) => break,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn validates_destinations_and_mixed_dns_answers() {
        for url in [
            "file:///x",
            "http://localhost/",
            "http://0x7f000001/",
            "https://example.org/?token=x",
            "https://a:b@example.org/",
            "https://example.org:22/",
            "https://example.org/#x",
        ] {
            assert!(target(url).is_none(), "{url}");
        }
        assert!(target("https://example.org/watch?v=1").is_some());
        assert!(connect_target("example.org:443").is_some());
        assert!(connect_target("example.org:443@internal:443").is_none());
        let public = "1.1.1.1:443".parse().unwrap();
        for addr in [
            "127.0.0.1:443",
            "169.254.169.254:443",
            "100.64.0.1:443",
            "[::ffff:127.0.0.1]:443",
            "[fc00::1]:443",
        ] {
            assert!(!approved_addresses(&[public, addr.parse().unwrap()]));
        }
        assert!(approved_addresses(&[public]));
    }
    #[test]
    fn real_proxy_denies_private_connect_and_private_http() {
        let proxy = Proxy::start().unwrap();
        let endpoint = proxy.endpoint().strip_prefix("http://").unwrap();
        for request in [
            "CONNECT 127.0.0.1:443 HTTP/1.1\r\n\r\n",
            "GET http://169.254.169.254/x HTTP/1.1\r\n\r\n",
        ] {
            let mut stream = TcpStream::connect(endpoint).unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            stream.write_all(request.as_bytes()).unwrap();
            let mut response = String::new();
            stream.read_to_string(&mut response).unwrap();
            assert!(response.starts_with("HTTP/1.1 403"));
        }
        assert!(proxy.blocked());
    }
}
