use lazy_static::lazy_static;
use linkify::{LinkFinder, LinkKind};
use nostr_types::find_nostr_bech32_pos;
use nostr_types::{NostrBech32, NostrUrl};
use regex::Regex;

/// This is like Range<usize>, except we impl offset() on it
/// This is like linkify::Span, except we impl offset() on it and don't need
///   the as_str() or kind() functions.
#[derive(Debug)]
pub struct Span {
    start: usize,
    end: usize,
}

impl Span {
    pub fn offset(&mut self, offset: usize) {
        self.start += offset;
        self.end += offset;
    }
}

/// A segment of content
#[derive(Debug)]
pub enum ContentSegment {
    NostrUrl(NostrUrl),
    TagReference(usize),
    Hyperlink(Span),
    Plain(Span),
}

/// A sequence of content segments
pub struct ShatteredContent {
    pub segments: Vec<ContentSegment>,

    // `Range`s within segments refer to this
    pub allocated: String,
}

impl ShatteredContent {
    pub fn slice<'a>(&'a self, span: &Span) -> Option<&'a str> {
        if span.end <= self.allocated.len() {
            Some(&self.allocated[span.start..span.end])
        } else {
            None
        }
    }
}

/// Break content into meaningful segments
///
/// This avoids reallocation
pub(super) fn shatter_content(content: String) -> ShatteredContent {
    let segments = shatter_content_1(&content);

    ShatteredContent {
        segments,
        allocated: content
    }
}

/// Break content into a linear sequence of `ContentSegment`s
fn shatter_content_1(mut content: &str) -> Vec<ContentSegment> {
    let mut segments: Vec<ContentSegment> = Vec::new();
    let mut offset: usize = 0; // used to adjust Span ranges

    // Pass 1 - `NostrUrl`s
    while let Some((start, end)) = find_nostr_bech32_pos(content) {
        // The stuff before it
        if start >= 6 && content.get(start - 6..start) == Some("nostr:") {
            let mut inner_segments = shatter_content_2(&content[..start - 6]);
            apply_offset(&mut inner_segments, offset);
            segments.append(&mut inner_segments);
        } else {
            let mut inner_segments = shatter_content_2(&content[..start]);
            apply_offset(&mut inner_segments, offset);
            segments.append(&mut inner_segments);
        }

        // The Nostr Bech32 itself
        if let Some(nbech) = NostrBech32::try_from_string(&content[start..end]) {
            segments.push(ContentSegment::NostrUrl(NostrUrl(nbech)));
        } else {
            tracing::error!(
                "PROBLEM PARSING THIS BECH32 MATCHED STRING: {}",
                &content[start..end]
            );
            // something is wrong with find_nostr_bech32_pos() or our code here.
        }

        offset += end;
        content = &content[end..];
    }

    // The stuff after it
    let mut inner_segments = shatter_content_2(content);
    apply_offset(&mut inner_segments, offset);
    segments.append(&mut inner_segments);

    segments
}

// Pass 2 - `TagReference`s
fn shatter_content_2(content: &str) -> Vec<ContentSegment> {
    lazy_static! {
        static ref TAG_RE: Regex = Regex::new(r"(\#\[\d+\])").unwrap();
    }

    let mut segments: Vec<ContentSegment> = Vec::new();

    let mut pos = 0;
    for mat in TAG_RE.find_iter(content) {
        let mut inner_segments = shatter_content_3(&content[pos..mat.start()]);
        apply_offset(&mut inner_segments, pos);
        segments.append(&mut inner_segments);

        // If panics on unwrap, something is wrong with Regex.
        let u: usize = content[mat.start() + 2..mat.end() - 1].parse().unwrap();
        segments.push(ContentSegment::TagReference(u));
        pos = mat.end();
    }

    let mut inner_segments = shatter_content_3(&content[pos..]);
    apply_offset(&mut inner_segments, pos);
    segments.append(&mut inner_segments);

    segments
}

fn shatter_content_3(content: &str) -> Vec<ContentSegment> {
    let mut segments: Vec<ContentSegment> = Vec::new();

    for span in LinkFinder::new().kinds(&[LinkKind::Url]).spans(content) {
        if span.kind().is_some() {
            segments.push(ContentSegment::Hyperlink(Span {
                start: span.start(),
                end: span.end(),
            }));
        } else {
            if !span.as_str().is_empty() {
                segments.push(ContentSegment::Plain(Span {
                    start: span.start(),
                    end: span.end(),
                }));
            }
        }
    }

    segments
}

fn apply_offset(segments: &mut Vec<ContentSegment>, offset: usize) {
    for segment in segments.iter_mut() {
        match segment {
            ContentSegment::Hyperlink(span) => span.offset(offset),
            ContentSegment::Plain(span) => span.offset(offset),
            _ => { }
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_shatter_content() {
        let content = "My friend #[0]  wrote me this note: nostr:note10ttnuuvcs29y3k23gwrcurw2ksvgd7c2rrqlfx7urmt5m963vhss8nja90 and it might have referred to https://github.com/Giszmo/nostr.info/blob/master/assets/js/main.js";
        let pieces = shatter_content(content);
        assert_eq!(pieces.segments.len(), 6);
        assert!(matches!(pieces.segments[0], ContentSegment::Plain(..)));
        assert!(matches!(pieces.segments[1], ContentSegment::TagReference(..)));
        assert!(matches!(pieces.segments[2], ContentSegment::Plain(..)));
        assert!(matches!(pieces.segments[3], ContentSegment::NostrUrl(..)));
        assert!(matches!(pieces.segments[4], ContentSegment::Plain(..)));
        assert!(matches!(pieces.segments[5], ContentSegment::Hyperlink(..)));

        let content = r#"This is a test of NIP-27 posting support referencing this note nostr:nevent1qqsqqqq9wh98g4u6e480vyp6p4w3ux2cd0mxn2rssq0w5cscsgzp2ksprpmhxue69uhkzapwdehhxarjwahhy6mn9e3k7mf0qyt8wumn8ghj7etyv4hzumn0wd68ytnvv9hxgtcpremhxue69uhkummnw3ez6ur4vgh8wetvd3hhyer9wghxuet59uq3kamnwvaz7tmwdaehgu3wd45kketyd9kxwetj9e3k7mf0qy2hwumn8ghj7mn0wd68ytn00p68ytnyv4mz7qgnwaehxw309ahkvenrdpskjm3wwp6kytcpz4mhxue69uhhyetvv9ujuerpd46hxtnfduhsz9mhwden5te0wfjkccte9ehx7um5wghxyctwvshszxthwden5te0wfjkccte9eekummjwsh8xmmrd9skctcnmzajy and again without the url data nostr:note1qqqq2aw2w3te4n2w7cgr5r2arcv4s6lkdx58pqq7af3p3qsyz4dqns2935
And referencing this person nostr:npub1acg6thl5psv62405rljzkj8spesceyfz2c32udakc2ak0dmvfeyse9p35c and again as an nprofile nostr:nprofile1qqswuyd9ml6qcxd92h6pleptfrcqucvvjy39vg4wx7mv9wm8kakyujgprdmhxue69uhkummnw3ezumtfddjkg6tvvajhytnrdakj7qg7waehxw309ahx7um5wgkhqatz9emk2mrvdaexgetj9ehx2ap0qythwumn8ghj7un9d3shjtnwdaehgu3wd9hxvme0qyt8wumn8ghj7etyv4hzumn0wd68ytnvv9hxgtcpzdmhxue69uhk7enxvd5xz6tw9ec82c30qy2hwumn8ghj7mn0wd68ytn00p68ytnyv4mz7qgcwaehxw309ashgtnwdaehgunhdaexkuewvdhk6tczkvt9n all on the same damn line even (I think)."#;
        let pieces = shatter_content(content);
        assert_eq!(pieces.segments.len(), 9);
    }
}
