use super::{Control, Progress, TRANSFER_TIMEOUT};
use crate::analysis::proxy;
use reqwest::{
    blocking::{Client, Response},
    header::{ACCEPT_ENCODING, CONTENT_RANGE, LOCATION, RANGE},
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

pub(super) const MAX_PARTS: usize = 1000;
const MAX_PLAYLIST: usize = 1024 * 1024;

pub(super) struct Part {
    pub url: Url,
    pub range: Option<RangeInclusive<u64>>,
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

    fn get(
        &self,
        mut url: Url,
        range: Option<&RangeInclusive<u64>>,
    ) -> Result<Response, &'static str> {
        for _ in 0..=5 {
            self.check()?;
            proxy::target(url.as_str()).ok_or("URL_NOT_ALLOWED")?;
            let mut request = self
                .client
                .get(url.clone())
                .header(ACCEPT_ENCODING, "identity");
            if let Some(range) = range {
                request = request.header(RANGE, format!("bytes={}-{}", range.start(), range.end()));
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
                200 if range.is_none() => return Ok(response),
                206 if range.is_some_and(|range| valid_content_range(&response, range)) => {
                    return Ok(response)
                }
                401 => return Err("AUTH_REQUIRED"),
                403 => return Err("ACCESS_DENIED"),
                404 => return Err("NOT_FOUND"),
                410 => return Err("EXPIRED_OR_GONE"),
                408 | 429 | 500..=599 => return Err("NETWORK_ERROR"),
                _ => return Err("STREAM_RESPONSE_INVALID"),
            }
        }
        Err("STREAM_RESPONSE_INVALID")
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
            let mut response = self.get(part.url.clone(), part.range.as_ref())?;
            let expected = part
                .range
                .as_ref()
                .map(|range| range.end() - range.start() + 1)
                .or_else(|| response.content_length());
            let mut count = 0;
            loop {
                self.check()?;
                let read = response.read(&mut buffer).map_err(|_| "NETWORK_ERROR");
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
                    });
                    last_update = Instant::now();
                }
            }
            if count == 0 || expected.is_some_and(|expected| count != expected) {
                return Err("STREAM_RESPONSE_INVALID");
            }
        }
        file.flush().map_err(|_| "DOWNLOAD_SAVE_FAILED")?;
        update(Progress {
            downloaded,
            total: Some(downloaded),
            bytes_per_second: None,
            segments: Some((parts.len(), parts.len())),
        });
        Ok(())
    }
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
