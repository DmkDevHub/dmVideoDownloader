use super::{
    stream::{resolve, Part, StreamClient, MAX_PARTS},
    Demux,
};
use crate::analysis::extract::{Format, Tracks};
use std::{collections::HashMap, ops::RangeInclusive};
use url::Url;

const INVALID: &str = "HLS_PLAYLIST_UNSUPPORTED";

fn attributes(mut value: &str) -> Result<HashMap<&str, &str>, &'static str> {
    let mut result = HashMap::new();
    while !value.is_empty() {
        let (key, rest) = value.split_once('=').ok_or(INVALID)?;
        let (item, next) = if let Some(rest) = rest.strip_prefix('"') {
            rest.split_once('"').ok_or(INVALID)?
        } else {
            match rest.find(',') {
                Some(index) => rest.split_at(index),
                None => (rest, ""),
            }
        };
        if result.insert(key, item).is_some() {
            return Err(INVALID);
        }
        value = if next.is_empty() {
            next
        } else {
            next.strip_prefix(',').ok_or(INVALID)?
        };
    }
    Ok(result)
}

fn playlist(text: &str) -> Result<impl Iterator<Item = &str> + Clone, &'static str> {
    let mut lines = text.lines().map(str::trim).filter(|line| !line.is_empty());
    if lines.next() != Some("#EXTM3U") {
        return Err(INVALID);
    }
    for line in lines.clone() {
        if line.starts_with("#EXT-X-SESSION-KEY")
            || line
                .strip_prefix("#EXT-X-KEY:")
                .is_some_and(|value| value != "METHOD=NONE")
        {
            return Err("HLS_PROTECTED");
        }
    }
    Ok(lines)
}

pub(super) fn audio<'a>(
    client: &StreamClient<'_>,
    video: &Format,
    formats: &'a [Format],
) -> Result<Option<&'a Format>, &'static str> {
    let media = video.media.as_ref().ok_or(INVALID)?;
    let Some(master) = media.manifest.as_ref().filter(|url| **url != media.url) else {
        return Ok(None);
    };
    let (base, text) = client.playlist(master)?;
    let mut variants = playlist(&text)?;
    let mut matching_variant = None;
    while let Some(line) = variants.next() {
        if let Some(value) = line.strip_prefix("#EXT-X-STREAM-INF:") {
            let uri = variants
                .next()
                .filter(|line| !line.starts_with('#'))
                .ok_or(INVALID)?;
            if resolve(&base, uri).ok().as_ref() == Some(&media.url) {
                matching_variant = Some(attributes(value)?);
                break;
            }
        }
    }
    let variant = matching_variant.ok_or(INVALID)?;
    let Some(group) = variant.get("AUDIO").copied() else {
        return Ok(None);
    };
    let mut renditions = Vec::new();
    let mut external = false;
    for line in playlist(&text)? {
        let Some(value) = line.strip_prefix("#EXT-X-MEDIA:") else {
            continue;
        };
        let attrs = attributes(value)?;
        if attrs.get("TYPE") != Some(&"AUDIO") || attrs.get("GROUP-ID") != Some(&group) {
            continue;
        }
        let Some(uri) = attrs.get("URI") else {
            continue;
        };
        external = true;
        let Ok(url) = resolve(&base, uri) else {
            continue;
        };
        if let Some(format) = formats.iter().find(|format| {
            format.tracks == Tracks::AudioOnly
                && format
                    .media
                    .as_ref()
                    .is_some_and(|media| media.url == url && media.is_hls())
        }) {
            renditions.push((
                attrs.get("DEFAULT") == Some(&"YES"),
                attrs.get("AUTOSELECT") == Some(&"YES"),
                format,
            ));
        }
    }
    let selected = renditions
        .into_iter()
        .max_by_key(|(default, autoselect, _)| (*default, *autoselect))
        .map(|(_, _, format)| format);
    if external && selected.is_none() {
        return Err(INVALID);
    }
    Ok(selected)
}

fn byte_range(
    value: &str,
    url: &Url,
    previous: Option<&Part>,
) -> Result<RangeInclusive<u64>, &'static str> {
    let (len, offset) = match value.split_once('@') {
        Some((len, offset)) => (len, Some(offset.parse::<u64>().map_err(|_| INVALID)?)),
        None => (
            value,
            previous
                .filter(|part| part.url == *url)
                .and_then(|part| part.range.as_ref())
                .and_then(|range| range.end().checked_add(1)),
        ),
    };
    let start = offset.ok_or(INVALID)?;
    let len = len.parse::<u64>().map_err(|_| INVALID)?;
    let end = start
        .checked_add(len.checked_sub(1).ok_or(INVALID)?)
        .ok_or(INVALID)?;
    if end >= u64::MAX - 1 {
        return Err(INVALID);
    }
    Ok(start..=end)
}

pub(super) fn plan(base: &Url, text: &str) -> Result<(Vec<Part>, Demux), &'static str> {
    let lines = playlist(text)?;
    if !lines.clone().any(|line| line == "#EXT-X-ENDLIST") {
        return Err("HLS_LIVE_UNSUPPORTED");
    }
    let mut parts = Vec::<Part>::new();
    let mut initialization = false;
    let mut segment = false;
    let mut pending_range = None;
    let mut count = 0;
    for line in lines {
        if line == "#EXT-X-ENDLIST" {
            break;
        }
        if let Some(value) = line.strip_prefix("#EXT-X-MAP:") {
            if initialization || count != 0 {
                return Err(INVALID);
            }
            let attrs = attributes(value)?;
            let url = resolve(base, attrs.get("URI").ok_or(INVALID)?)?;
            let range = attrs
                .get("BYTERANGE")
                .map(|value| byte_range(value, &url, None))
                .transpose()?;
            parts.push(Part { url, range });
            initialization = true;
        } else if line.starts_with("#EXTINF:") {
            if segment {
                return Err(INVALID);
            }
            segment = true;
        } else if let Some(value) = line.strip_prefix("#EXT-X-BYTERANGE:") {
            if pending_range.replace(value).is_some() {
                return Err(INVALID);
            }
        } else if line.starts_with("#EXT-X-STREAM-INF:")
            || line.starts_with("#EXT-X-I-FRAMES-ONLY")
            || line.starts_with("#EXT-X-GAP")
            || line.starts_with("#EXT-X-DISCONTINUITY:")
            || line == "#EXT-X-DISCONTINUITY"
        {
            return Err(INVALID);
        } else if !line.starts_with('#') {
            if !segment {
                return Err(INVALID);
            }
            let url = resolve(base, line)?;
            let previous = if count > 0 { parts.last() } else { None };
            let range = pending_range
                .take()
                .map(|value| byte_range(value, &url, previous))
                .transpose()?;
            parts.push(Part { url, range });
            count += 1;
            segment = false;
            if parts.len() > MAX_PARTS {
                return Err("DOWNLOAD_LIMIT");
            }
        }
    }
    if count == 0 || segment || pending_range.is_some() {
        return Err(INVALID);
    }
    Ok((
        parts,
        if initialization {
            Demux::Mov
        } else {
            Demux::HlsPacked
        },
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_keys_and_unsafe_fragments_before_transfer() {
        let base = Url::parse("https://example.org/media.m3u8").unwrap();
        for (tags, uri, reason) in [
            (
                "#EXT-X-KEY:METHOD=AES-128,URI=\"key\"",
                "a.ts",
                "HLS_PROTECTED",
            ),
            (
                "#EXT-X-SESSION-KEY:METHOD=SAMPLE-AES,URI=\"key\"",
                "a.ts",
                "HLS_PROTECTED",
            ),
            ("", "http://127.0.0.1/private", "URL_NOT_ALLOWED"),
            ("", "a.ts?token=secret", "URL_NOT_ALLOWED"),
        ] {
            let text = format!("#EXTM3U\n{tags}\n#EXTINF:4,\n{uri}\n#EXT-X-ENDLIST\n");
            assert_eq!(plan(&base, &text).err(), Some(reason));
        }
        assert_eq!(
            plan(&base, "#EXTM3U\n#EXTINF:4,\na.ts\n").err(),
            Some("HLS_LIVE_UNSUPPORTED")
        );
    }

    #[test]
    fn parses_clear_media_init_and_relative_byte_ranges() {
        let base = Url::parse("https://example.org/folder/media.m3u8").unwrap();
        let text = "#EXTM3U\n#EXT-X-MAP:URI=\"asset,1.mp4\",BYTERANGE=\"12@0\"\n#EXTINF:4,\n#EXT-X-BYTERANGE:12@12\nasset,1.mp4\n#EXTINF:4,\n#EXT-X-BYTERANGE:8\nasset,1.mp4\n#EXT-X-ENDLIST\n";
        let (parts, demux) = plan(&base, text).unwrap();
        assert!(matches!(demux, Demux::Mov));
        assert_eq!(parts[0].range, Some(0..=11));
        assert_eq!(parts[2].range, Some(24..=31));
        assert_eq!(parts[2].url.path(), "/folder/asset,1.mp4");
    }
}
