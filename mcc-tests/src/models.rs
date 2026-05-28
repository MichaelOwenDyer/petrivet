/// Fast path: scan for the NUPN `<size places="N"` attribute without parsing XML.
#[must_use]
fn place_count_from_nupn(pnml: &str) -> Option<u32> {
    // Find `<size` then scan forward for `places="N"`
    let size_pos = pnml.find("<size")?;
    let after_size = &pnml[size_pos..];
    // Find the closing `>` of this tag to bound our search
    let tag_end = after_size.find('>')?;
    let tag = &after_size[..tag_end];
    // Extract places="N"
    let places_pos = tag.find("places=\"")?;
    let after_attr = &tag[places_pos + 8..]; // skip 'places="'
    let end = after_attr.find('"')?;
    after_attr[..end].parse().ok()
}

/// For models without NUPN data, count occurrences of `<place` in the PNML to estimate place count.
#[expect(clippy::cast_possible_truncation)]
fn place_count_by_tag_scan(pnml: &str) -> u32 {
    // Count occurrences of `<place` (covers both `<place ` and `<place>`)
    pnml.match_indices("<place").count() as u32
}

#[must_use] 
pub fn place_count(pnml: &str) -> u32 {
    place_count_from_nupn(pnml).unwrap_or_else(|| place_count_by_tag_scan(pnml))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nupn_fast_path_extracts_place_count() {
        let pnml = r#"<pnml><net><toolspecific tool="nupn" version="1.1"><size places="285" transitions="351" arcs="820"/></toolspecific></net></pnml>"#;
        assert_eq!(place_count_from_nupn(pnml), Some(285));
    }

    #[test]
    fn nupn_fast_path_returns_none_when_absent() {
        let pnml = r#"<pnml><net><place id="p1"/></net></pnml>"#;
        assert_eq!(place_count_from_nupn(pnml), None);
    }

    #[test]
    fn fallback_counts_place_tags() {
        let pnml = r#"<net><place id="p1"/><place id="p2"/><place id="p3"/></net>"#;
        assert_eq!(place_count_by_tag_scan(pnml), 3);
    }
}