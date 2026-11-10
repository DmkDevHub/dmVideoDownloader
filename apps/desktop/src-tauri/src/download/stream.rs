use super::{Continuation, Control, Progress, TRANSFER_TIMEOUT};
use crate::analysis::proxy;
use reqwest::{
    blocking::{Client, Response},
    header::{
        HeaderValue, ACCEPT_ENCODING, CONTENT_ENCODING, CONTENT_RANGE, ETAG, IF_RANGE, LOCATION,
        RANGE,
    },
    redirect::Policy,
};
use std::{
    fs::File,
    io::{Read, Write},
    ops::RangeInclusive,
    path::Path,
    time::{Duration, Instant},
};
use url::Url;

pub(super) use proxy::MAX_MEDIA_SEGMENTS as MAX_PARTS;
const MAX_PLAYLIST: usize = 1024 * 1024;

pub(super) struct Part {
    pub url: Url,
    pub range: Option<RangeInclusive<u64>>,
}

pub(super) struct PartialFile {
    path: tempfile::TempPath,
    identity: FileIdentity,
    downloaded: u64,
}

struct FileIdentity {
    source: Url,
    destination: Url,
    etag: HeaderValue,
    length: u64,
}

enum RangeRequest<'a> {
    Segment(&'a RangeInclusive<u64>),
    Resume {
        identity: &'a FileIdentity,
        offset: u64,
    },
}

pub(super) fn resolve(base: &Url, reference: &str) -> Result<Url, &'static str> {
    let url = base.join(reference).map_err(|_| "URL_NOT_ALLOWED")?;
    proxy::target(url.as_str()).ok_or("URL_NOT_ALLOWED")
}

pub(super) struct StreamClient<'a> {
    client: Client,
    proxy: proxy::Proxy,
    control: &'a Control,
    started: Instant,
}

impl<'a> StreamClient<'a> {
    pub fn new(control: &'a Control) -> Result<Self, &'static str> {
        let proxy = proxy::Proxy::download(TRANSFER_TIMEOUT).map_err(|_| "NETWORK_ERROR")?;
        Self::with_proxy(control, proxy)
    }

    fn with_proxy(control: &'a Control, proxy: proxy::Proxy) -> Result<Self, &'static str> {
        let client = Client::builder()
            .redirect(Policy::none())
            .proxy(reqwest::Proxy::all(proxy.endpoint()).map_err(|_| "NETWORK_ERROR")?)
            .connect_timeout(Duration::from_secs(5))
            .timeout(Duration::from_secs(20))
            .build()
            .map_err(|_| "NETWORK_ERROR")?;
        Ok(Self {
            client,
            proxy,
            control,
            started: Instant::now(),
        })
    }

    fn check(&self) -> Result<(), &'static str> {
        self.control.check()?;
        if self.proxy.blocked() {
            return Err("URL_NOT_ALLOWED");
        }
        if self.proxy.limited() {
            return Err("DOWNLOAD_LIMIT");
        }
        if self.started.elapsed() >= TRANSFER_TIMEOUT {
            return Err("DOWNLOAD_TIMEOUT");
        }
        Ok(())
    }

    fn get(&self, mut url: Url, range: Option<RangeRequest<'_>>) -> Result<Response, &'static str> {
        for _ in 0..=5 {
            self.check()?;
            proxy::target(url.as_str()).ok_or("URL_NOT_ALLOWED")?;
            let mut request = self
                .client
                .get(url.clone())
                .header(ACCEPT_ENCODING, "identity");
            match &range {
                Some(RangeRequest::Segment(range)) => {
                    request =
                        request.header(RANGE, format!("bytes={}-{}", range.start(), range.end()));
                }
                Some(RangeRequest::Resume { identity, offset }) if url == identity.destination => {
                    request = request
                        .header(RANGE, format!("bytes={}-{}", offset, identity.length - 1))
                        .header(IF_RANGE, &identity.etag);
                }
                _ => {}
            }
            let response = request.send().map_err(|error| {
                if error.is_timeout() {
                    "DOWNLOAD_TIMEOUT"
                } else {
                    "NETWORK_ERROR"
                }
            });
            self.check()?;
            let response = response?;
            match response.status().as_u16() {
                301 | 302 | 303 | 307 | 308 => {
                    let next = response
                        .headers()
                        .get(LOCATION)
                        .and_then(|value| value.to_str().ok())
                        .ok_or("STREAM_RESPONSE_INVALID")?;
                    url = resolve(&url, next)?;
                }
                200 if !matches!(range, Some(RangeRequest::Segment(_))) => return Ok(response),
                206 if matches!(&range, Some(RangeRequest::Segment(segment)) if valid_content_range(&response, segment)) => {
                    return Ok(response)
                }
                206 if matches!(&range, Some(RangeRequest::Resume { identity, offset }) if valid_resume(&response, identity, *offset)) => {
                    return Ok(response)
                }
                200 | 206 | 412 | 416 if range.is_some() => return Err("STREAM_RANGE_INVALID"),
                401 => return Err("AUTH_REQUIRED"),
                403 => return Err("ACCESS_DENIED"),
                404 => return Err("NOT_FOUND"),
                410 => return Err("EXPIRED_OR_GONE"),
                429 => return Err("RATE_LIMITED"),
                408 | 500..=599 => return Err("NETWORK_ERROR"),
                _ => return Err("STREAM_RESPONSE_INVALID"),
            }
        }
        Err("STREAM_REDIRECT_LIMIT")
    }

    pub fn playlist(&self, url: &Url) -> Result<(Url, String), &'static str> {
        let mut response = self.get(url.clone(), None)?;
        let base = response.url().clone();
        let mut bytes = Vec::new();
        response
            .by_ref()
            .take(MAX_PLAYLIST as u64 + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| "NETWORK_ERROR")?;
        self.check()?;
        if bytes.len() > MAX_PLAYLIST {
            return Err("DOWNLOAD_LIMIT");
        }
        let text = String::from_utf8(bytes).map_err(|_| "STREAM_RESPONSE_INVALID")?;
        Ok((base, text))
    }

    pub fn transfer(
        &self,
        parts: &[Part],
        path: &Path,
        mut update: impl FnMut(Progress),
    ) -> Result<(), &'static str> {
        crate::folders::check_path(path)?;
        if parts.is_empty() {
            return Err("STREAM_RESPONSE_INVALID");
        }
        if parts.len() > MAX_PARTS {
            return Err("DOWNLOAD_LIMIT");
        }
        let mut file = File::options()
            .write(true)
            .create_new(true)
            .open(path)
            .map_err(|_| "DOWNLOAD_SAVE_FAILED")?;
        let started = Instant::now();
        let mut last_update = started;
        let total = parts.iter().try_fold(0u64, |total, part| {
            let range = part.range.as_ref()?;
            total.checked_add(range.end().checked_sub(*range.start())?.checked_add(1)?)
        });
        let mut downloaded = 0u64;
        let mut buffer = [0; 64 * 1024];
        for (index, part) in parts.iter().enumerate() {
            let mut response = self
                .get(
                    part.url.clone(),
                    part.range.as_ref().map(RangeRequest::Segment),
                )
                .map_err(|error| {
                    if error == "NOT_FOUND" {
                        "STREAM_SEGMENT_MISSING"
                    } else {
                        error
                    }
                })?;
            let expected = part
                .range
                .as_ref()
                .map(|range| range.end() - range.start() + 1)
                .or_else(|| response.content_length());
            let mut count = 0;
            loop {
                self.check()?;
                let read = response.read(&mut buffer).map_err(|_| {
                    if expected.is_some_and(|expected| count < expected) {
                        "STREAM_BODY_INCOMPLETE"
                    } else {
                        "NETWORK_ERROR"
                    }
                });
                self.check()?;
                let n = read?;
                if n == 0 {
                    break;
                }
                count += n as u64;
                if expected.is_some_and(|expected| count > expected) {
                    return Err("STREAM_RESPONSE_INVALID");
                }
                file.write_all(&buffer[..n])
                    .map_err(|_| "DOWNLOAD_SAVE_FAILED")?;
                downloaded += n as u64;
                if last_update.elapsed() >= Duration::from_millis(250) {
                    update(Progress {
                        downloaded,
                        total,
                        bytes_per_second: Some(downloaded as f64 / started.elapsed().as_secs_f64()),
                        segments: Some((index, parts.len())),
                        continuation: None,
                    });
                    last_update = Instant::now();
                }
            }
            if expected.is_some_and(|expected| count != expected) {
                return Err("STREAM_BODY_INCOMPLETE");
            }
            if count == 0 {
                return Err("STREAM_SEGMENT_EMPTY");
            }
        }
        file.flush().map_err(|_| "DOWNLOAD_SAVE_FAILED")?;
        update(Progress {
            downloaded,
            total: Some(downloaded),
            bytes_per_second: None,
            segments: Some((parts.len(), parts.len())),
            continuation: None,
        });
        Ok(())
    }

    pub fn download_file(
        &self,
        url: &Url,
        path: &Path,
        directory: &Path,
        saved: &mut Option<PartialFile>,
        mut update: impl FnMut(Progress),
    ) -> Result<(), &'static str> {
        crate::folders::check_path(path)?;
        crate::folders::check_path(directory)?;
        let had_partial = saved.is_some();
        let mut offset = 0;
        if let Some(partial) = saved.as_ref() {
            crate::folders::check_path(&partial.path)?;
            offset = partial.downloaded;
            if partial.identity.source != *url
                || !std::fs::metadata(&partial.path).is_ok_and(|metadata| metadata.len() == offset)
            {
                saved.take();
                offset = 0;
            }
        }
        let requested_range = saved.as_ref().map(|partial| RangeRequest::Resume {
            identity: &partial.identity,
            offset,
        });
        let response = match self.get(url.clone(), requested_range) {
            Err("STREAM_RANGE_INVALID") if saved.is_some() => {
                saved.take();
                self.get(url.clone(), None)?
            }
            result => result?,
        };
        if response
            .headers()
            .get(CONTENT_ENCODING)
            .is_some_and(|value| !value.as_bytes().eq_ignore_ascii_case(b"identity"))
        {
            saved.take();
            return Err("STREAM_RESPONSE_INVALID");
        }
        let (mut file, temporary, identity) = if response.status().as_u16() == 206 {
            let partial = saved.take().ok_or("STREAM_RANGE_INVALID")?;
            let file = File::options()
                .append(true)
                .open(&partial.path)
                .map_err(|_| "DOWNLOAD_SAVE_FAILED")?;
            if file.metadata().map_err(|_| "DOWNLOAD_SAVE_FAILED")?.len() != offset {
                return Err("DOWNLOAD_SAVE_FAILED");
            }
            (file, partial.path, Some(partial.identity))
        } else {
            saved.take();
            offset = 0;
            let temporary = tempfile::Builder::new()
                .prefix(".dm-part-")
                .tempfile_in(directory)
                .map_err(|_| "DOWNLOAD_SAVE_FAILED")?;
            let (file, path) = temporary.into_parts();
            let identity = strong_etag(&response)
                .cloned()
                .zip(response.content_length())
                .map(|(etag, length)| FileIdentity {
                    source: url.clone(),
                    destination: response.url().clone(),
                    etag,
                    length,
                });
            (file, path, identity)
        };
        let total = identity
            .as_ref()
            .map(|value| value.length)
            .or_else(|| response.content_length());
        let mut downloaded = offset;
        let continuation = if offset > 0 {
            Some(Continuation::Resumed)
        } else if had_partial {
            Some(Continuation::Restarted)
        } else {
            None
        };
        let result = self.copy_file(
            response,
            &mut file,
            total,
            &mut downloaded,
            continuation,
            &mut update,
        );
        drop(file);
        match result {
            Ok(()) => temporary
                .persist_noclobber(path)
                .map_err(|_| "DOWNLOAD_SAVE_FAILED"),
            Err(reason) => {
                if matches!(
                    reason,
                    "STREAM_BODY_INCOMPLETE"
                        | "NETWORK_ERROR"
                        | "DOWNLOAD_TIMEOUT"
                        | "DOWNLOAD_INTERRUPTED"
                ) {
                    if let Some(identity) =
                        identity.filter(|identity| downloaded > 0 && downloaded < identity.length)
                    {
                        *saved = Some(PartialFile {
                            path: temporary,
                            identity,
                            downloaded,
                        });
                    }
                }
                Err(reason)
            }
        }
    }

    fn copy_file(
        &self,
        mut response: Response,
        file: &mut File,
        total: Option<u64>,
        downloaded: &mut u64,
        continuation: Option<Continuation>,
        update: &mut impl FnMut(Progress),
    ) -> Result<(), &'static str> {
        let offset = *downloaded;
        if continuation.is_some() {
            update(Progress {
                downloaded: offset,
                total,
                bytes_per_second: None,
                segments: None,
                continuation,
            });
        }
        let started = Instant::now();
        let mut last_update = None;
        let mut buffer = [0; 64 * 1024];
        loop {
            self.check()?;
            let read = response.read(&mut buffer);
            self.check()?;
            let n = read.map_err(|_| {
                if total.is_some_and(|size| *downloaded < size) {
                    "STREAM_BODY_INCOMPLETE"
                } else {
                    "NETWORK_ERROR"
                }
            })?;
            if n == 0 {
                break;
            }
            let next = *downloaded + n as u64;
            if total.is_some_and(|size| next > size) {
                return Err("STREAM_RESPONSE_INVALID");
            }
            file.write_all(&buffer[..n])
                .map_err(|_| "DOWNLOAD_SAVE_FAILED")?;
            *downloaded = next;
            if last_update.is_none_or(|last: Instant| last.elapsed() >= Duration::from_millis(250))
            {
                update(Progress {
                    downloaded: next,
                    total,
                    bytes_per_second: Some(
                        (next - offset) as f64 / started.elapsed().as_secs_f64().max(0.001),
                    ),
                    segments: None,
                    continuation,
                });
                last_update = Some(Instant::now());
            }
        }
        if total.is_some_and(|size| *downloaded != size) {
            return Err("STREAM_BODY_INCOMPLETE");
        }
        if *downloaded == 0 {
            return Err("DOWNLOAD_FAILED");
        }
        file.flush().map_err(|_| "DOWNLOAD_SAVE_FAILED")?;
        update(Progress {
            downloaded: *downloaded,
            total: Some(*downloaded),
            bytes_per_second: None,
            segments: None,
            continuation,
        });
        self.check()
    }
}

fn strong_etag(response: &Response) -> Option<&HeaderValue> {
    let tags = response.headers().get_all(ETAG);
    let mut values = tags.iter();
    let etag = values.next()?;
    if values.next().is_some() {
        return None;
    }
    let value = etag.to_str().ok()?.strip_prefix('"')?.strip_suffix('"')?;
    (value.len() <= 256
        && value
            .bytes()
            .all(|byte| (0x21..=0x7e).contains(&byte) && byte != b'"'))
    .then_some(etag)
}

fn valid_resume(response: &Response, identity: &FileIdentity, offset: u64) -> bool {
    response.url() == &identity.destination
        && strong_etag(response) == Some(&identity.etag)
        && response
            .headers()
            .get(CONTENT_RANGE)
            .and_then(|value| value.to_str().ok())
            == Some(
                format!(
                    "bytes {}-{}/{}",
                    offset,
                    identity.length - 1,
                    identity.length
                )
                .as_str(),
            )
}

fn valid_content_range(response: &Response, range: &RangeInclusive<u64>) -> bool {
    let Some((actual, total)) = response
        .headers()
        .get(CONTENT_RANGE)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("bytes "))
        .and_then(|value| value.split_once('/'))
    else {
        return false;
    };
    actual == format!("{}-{}", range.start(), range.end())
        && (total == "*" || total.parse::<u64>().is_ok_and(|total| total > *range.end()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        io::{BufRead, BufReader},
        net::TcpListener,
        thread,
    };

    fn fixture(
        responses: Vec<&'static str>,
        run: impl FnOnce(&StreamClient<'_>, Url),
    ) -> Vec<String> {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let proxy = proxy::Proxy::with_test_fixture(listener.local_addr().unwrap()).unwrap();
        listener.set_nonblocking(true).unwrap();
        let server = thread::spawn(move || {
            let mut requests = Vec::new();
            for response in responses {
                let deadline = Instant::now() + Duration::from_secs(5);
                let mut connection = loop {
                    match listener.accept() {
                        Ok((stream, _)) => break stream,
                        Err(error)
                            if error.kind() == std::io::ErrorKind::WouldBlock
                                && Instant::now() < deadline =>
                        {
                            thread::sleep(Duration::from_millis(10))
                        }
                        Err(error) => panic!("fixture did not receive request: {error}"),
                    }
                };
                connection.set_nonblocking(false).unwrap();
                connection
                    .set_read_timeout(Some(Duration::from_secs(2)))
                    .unwrap();
                let mut request = String::new();
                let mut reader = BufReader::new(&mut connection);
                loop {
                    let mut line = String::new();
                    assert!(reader.read_line(&mut line).unwrap() > 0);
                    if line == "\r\n" {
                        break;
                    }
                    request.push_str(&line);
                }
                connection.write_all(response.as_bytes()).unwrap();
                requests.push(request.to_ascii_lowercase());
            }
            requests
        });
        let control = Control::default();
        let client = StreamClient::with_proxy(&control, proxy).unwrap();
        run(
            &client,
            Url::parse("http://fixture.example.org/asset").unwrap(),
        );
        server.join().unwrap()
    }

    #[test]
    fn distinguishes_missing_empty_truncated_rate_limited_and_bad_range_responses() {
        for (response, range, expected) in [
            ("HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n", None, "STREAM_SEGMENT_MISSING"),
            ("HTTP/1.1 200 OK\r\nContent-Length: 0\r\nConnection: close\r\n\r\n", None, "STREAM_SEGMENT_EMPTY"),
            ("HTTP/1.1 200 OK\r\nContent-Length: 4\r\nConnection: close\r\n\r\nab", None, "STREAM_BODY_INCOMPLETE"),
            ("HTTP/1.1 429 Too Many Requests\r\nContent-Length: 0\r\nConnection: close\r\n\r\n", None, "RATE_LIMITED"),
            ("HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nab", Some(2..=3), "STREAM_RANGE_INVALID"),
            ("HTTP/1.1 206 Partial Content\r\nContent-Range: bytes 0-1/4\r\nContent-Length: 2\r\nConnection: close\r\n\r\nab", Some(2..=3), "STREAM_RANGE_INVALID"),
        ] {
            fixture(vec![response], |client, url| {
                let folder = tempfile::tempdir().unwrap();
                let path = folder.path().join("part");
                assert_eq!(client.transfer(&[Part { url, range }], &path, |_| {}), Err(expected));
            });
        }
    }

    #[test]
    fn range_transfer_follows_only_validated_redirects_and_keeps_no_cookies() {
        let requests = fixture(vec![
            "HTTP/1.1 302 Found\r\nLocation: /media\r\nSet-Cookie: fixture=unused\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
            "HTTP/1.1 206 Partial Content\r\nContent-Range: bytes 2-3/4\r\nContent-Length: 2\r\nConnection: close\r\n\r\ncd",
        ], |client, url| {
            let folder = tempfile::tempdir().unwrap();
            let path = folder.path().join("part");
            client.transfer(&[Part { url, range: Some(2..=3) }], &path, |_| {}).unwrap();
            assert_eq!(std::fs::read(path).unwrap(), b"cd");
        });
        assert!(requests[1].starts_with("get /media "));
        assert!(requests
            .iter()
            .all(|request| request.contains("\r\nrange: bytes=2-3")
                && !request.contains("\r\ncookie:")
                && !request.contains("\r\nauthorization:")));
        for location in [
            "HTTP/1.1 302 Found\r\nLocation: http://127.0.0.1/private\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
            "HTTP/1.1 302 Found\r\nLocation: /private?token=fixture\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
        ] {
            fixture(vec![location], |client, url| {
                assert_eq!(client.playlist(&url).err(), Some("URL_NOT_ALLOWED"));
            });
        }
    }

    #[test]
    fn resumes_a_truncated_file_at_the_saved_offset() {
        let requests = fixture(vec![
            "HTTP/1.1 200 OK\r\nETag: \"first\"\r\nContent-Length: 6\r\nConnection: close\r\n\r\nabc",
            "HTTP/1.1 206 Partial Content\r\nETag: \"first\"\r\nContent-Range: bytes 3-5/6\r\nContent-Length: 3\r\nConnection: close\r\n\r\ndef",
        ], |client, url| {
            let folder = tempfile::tempdir().unwrap();
            let path = folder.path().join("video.mp4");
            let mut saved = None;
            assert_eq!(client.download_file(&url, &path, folder.path(), &mut saved, |_| {}), Err("STREAM_BODY_INCOMPLETE"));
            assert!(!path.exists());
            assert_eq!(saved.as_ref().map(|part| part.downloaded), Some(3));
            let mut progress = Vec::new();
            client.download_file(&url, &path, folder.path(), &mut saved, |value| progress.push(value)).unwrap();
            assert_eq!(progress[0].continuation, Some(Continuation::Resumed));
            assert_eq!(progress[0].downloaded, 3);
            assert_eq!(std::fs::read(path).unwrap(), b"abcdef");
            assert!(saved.is_none());
            assert_eq!(std::fs::read_dir(folder.path()).unwrap().count(), 1);
        });
        assert!(requests[1].contains("\r\nrange: bytes=3-5"));
        assert!(requests[1].contains("\r\nif-range: \"first\""));
    }

    #[test]
    fn restarts_cleanly_for_a_changed_file_or_unverifiable_range() {
        let interrupted = "HTTP/1.1 200 OK\r\nETag: \"first\"\r\nContent-Length: 6\r\nConnection: close\r\n\r\nabc";
        let changed = "HTTP/1.1 200 OK\r\nETag: \"changed\"\r\nContent-Length: 6\r\nConnection: close\r\n\r\n123456";
        let incorrect = "HTTP/1.1 206 Partial Content\r\nETag: \"first\"\r\nContent-Range: bytes 3-5/9\r\nContent-Length: 3\r\nConnection: close\r\n\r\nbad";
        let weak = "HTTP/1.1 200 OK\r\nETag: W/\"first\"\r\nContent-Length: 6\r\nConnection: close\r\n\r\nabc";
        for (responses, expected_range) in [
            (vec![interrupted, changed], true),
            (vec![interrupted, incorrect, changed], true),
            (vec![weak, changed], false),
        ] {
            let expected_count = responses.len();
            let requests = fixture(responses, |client, url| {
                let folder = tempfile::tempdir().unwrap();
                let path = folder.path().join("video.mp4");
                let mut saved = None;
                assert_eq!(
                    client.download_file(&url, &path, folder.path(), &mut saved, |_| {}),
                    Err("STREAM_BODY_INCOMPLETE")
                );
                assert_eq!(saved.is_some(), expected_range);
                let mut progress = Vec::new();
                client
                    .download_file(&url, &path, folder.path(), &mut saved, |value| {
                        progress.push(value)
                    })
                    .unwrap();
                assert_eq!(
                    progress[0].continuation,
                    expected_range.then_some(Continuation::Restarted)
                );
                assert_eq!(std::fs::read(path).unwrap(), b"123456");
            });
            assert_eq!(requests[1].contains("\r\nif-range:"), expected_range);
            if expected_count == 3 {
                assert!(!requests[2].contains("\r\nrange:"));
            }
        }
    }

    #[test]
    fn does_not_send_the_validator_to_a_new_redirect_destination() {
        let requests = fixture(vec![
            "HTTP/1.1 302 Found\r\nLocation: /original\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
            "HTTP/1.1 200 OK\r\nETag: \"first\"\r\nContent-Length: 6\r\nConnection: close\r\n\r\nabc",
            "HTTP/1.1 302 Found\r\nLocation: /replacement\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
            "HTTP/1.1 200 OK\r\nETag: \"first\"\r\nContent-Length: 6\r\nConnection: close\r\n\r\n123456",
        ], |client, url| {
            let folder = tempfile::tempdir().unwrap();
            let path = folder.path().join("video.mp4");
            let mut saved = None;
            assert_eq!(client.download_file(&url, &path, folder.path(), &mut saved, |_| {}), Err("STREAM_BODY_INCOMPLETE"));
            client.download_file(&url, &path, folder.path(), &mut saved, |_| {}).unwrap();
            assert_eq!(std::fs::read(path).unwrap(), b"123456");
        });
        assert!(requests[3].starts_with("get /replacement "));
        assert!(requests
            .iter()
            .all(|request| !request.contains("\r\nif-range:")));
    }
}
