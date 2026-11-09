use super::stream::StreamClient;
use crate::analysis::extract::Media;
use quick_xml::{
    events::{BytesStart, Event},
    Reader,
};

const UNSUPPORTED: &str = "DASH_MANIFEST_UNSUPPORTED";

pub(super) fn check_manifest(client: &StreamClient<'_>, media: &Media) -> Result<(), &'static str> {
    let (_, manifest) = client.playlist(media.manifest.as_ref().ok_or(UNSUPPORTED)?)?;
    validate(&manifest)
}

fn validate(text: &str) -> Result<(), &'static str> {
    let mut reader = Reader::from_str(text);
    let mut depth = 0;
    let mut roots = 0;
    let mut periods = 0;
    loop {
        match reader.read_event().map_err(|_| UNSUPPORTED)? {
            Event::Start(element) => {
                element_policy(&element, depth, &mut roots, &mut periods)?;
                depth += 1;
            }
            Event::Empty(element) => element_policy(&element, depth, &mut roots, &mut periods)?,
            Event::End(_) if depth > 0 => depth -= 1,
            Event::End(_) | Event::DocType(_) => return Err(UNSUPPORTED),
            Event::Eof => break,
            _ => {}
        }
    }
    if depth != 0 || roots != 1 || periods != 1 {
        return Err(UNSUPPORTED);
    }
    Ok(())
}

fn element_policy(
    element: &BytesStart<'_>,
    depth: usize,
    roots: &mut usize,
    periods: &mut usize,
) -> Result<(), &'static str> {
    let name = element.local_name();
    if depth == 0 {
        if name.as_ref() != "MPD" {
            return Err(UNSUPPORTED);
        }
        *roots += 1;
    }
    match name.as_ref() {
        "ContentProtection" => return Err("DRM_PROTECTED"),
        "Location" | "PatchLocation" => return Err(UNSUPPORTED),
        "Period" if depth == 1 => *periods += 1,
        _ => {}
    }
    for attribute in element.attributes() {
        let attribute = attribute.map_err(|_| UNSUPPORTED)?;
        if depth == 0 && attribute.key.as_ref() == "type" && attribute.value.as_ref() != "static" {
            return Err("DASH_LIVE_UNSUPPORTED");
        }
        if matches!(
            attribute.key.local_name().as_ref(),
            "href" | "mediaRange" | "indexRange" | "range"
        ) {
            return Err(UNSUPPORTED);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_clear_static_single_period_mpd_is_accepted_for_segment_transfer() {
        assert_eq!(
            validate(r#"<MPD type="static"><Period><SegmentTemplate /></Period></MPD>"#),
            Ok(())
        );
        assert_eq!(
            validate(r#"<MPD type="dynamic"><Period /></MPD>"#),
            Err("DASH_LIVE_UNSUPPORTED")
        );
        assert_eq!(
            validate(r#"<MPD><Period><c:ContentProtection /></Period></MPD>"#),
            Err("DRM_PROTECTED")
        );
        for manifest in [
            r#"<MPD><Period /><Period /></MPD>"#,
            r#"<MPD><Period xlink:href="period.xml" /></MPD>"#,
            r#"<MPD><Period><SegmentURL mediaRange="0-100" /></Period></MPD>"#,
            r#"<!DOCTYPE MPD><MPD><Period /></MPD>"#,
            r#"<MPD><Period />"#,
        ] {
            assert_eq!(validate(manifest), Err(UNSUPPORTED));
        }
    }
}
