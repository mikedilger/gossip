use super::{find_nostr_url_pos, NostrBech32, NostrUrl};
use aho_corasick::AhoCorasick;
use lazy_static::lazy_static;
use linkify::{LinkFinder, LinkKind};
use regex::Regex;

/// This is like `Range<usize>`, except we impl offset() on it
/// This is like linkify::Span, except we impl offset() on it and don't need
///   the as_str() or kind() functions.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Span {
    start: usize,
    end: usize,
}

impl Span {
    /// Modify a span by offsetting it from the start by `offset` bytes
    pub fn offset(&mut self, offset: usize) {
        self.start += offset;
        self.end += offset;
    }
}

/// A segment of content
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ContentSegment {
    /// A Nostr URL
    NostrUrl(NostrUrl),

    /// A reference to an event tag by index
    TagReference(usize),

    /// A hyperlink
    Hyperlink(Span),

    /// A hash tag
    Hashtag(String),

    /// Plain text
    Plain(Span),
}

/// A sequence of content segments
#[derive(Clone, Debug)]
pub struct ShatteredContent {
    /// The sequence of `ContentSegment`s
    pub segments: Vec<ContentSegment>,

    /// The original content (the allocated string)
    /// `Range`s within segments refer to this
    pub allocated: String,
}

impl ShatteredContent {
    /// Break content into meaningful segments
    ///
    /// This avoids reallocation
    pub fn new(content: String, replace_app_links: bool) -> ShatteredContent {
        let content = if replace_app_links {
            replace_urls_with_nostr(&content)
        } else {
            content
        };

        let segments = shatter_content_1(&content);

        ShatteredContent {
            segments,
            allocated: content,
        }
    }

    /// View a slice of the original content as specified in a Span
    #[allow(clippy::string_slice)] // the Span is trusted
    pub fn slice<'a>(&'a self, span: &Span) -> Option<&'a str> {
        if self.allocated.is_char_boundary(span.start) && self.allocated.is_char_boundary(span.end)
        {
            Some(&self.allocated[span.start..span.end])
        } else {
            None
        }
    }
}

/// Break content into a linear sequence of `ContentSegment`s
#[allow(clippy::string_slice)] // start/end from find_nostr_url_pos is trusted
fn shatter_content_1(mut content: &str) -> Vec<ContentSegment> {
    let mut segments: Vec<ContentSegment> = Vec::new();
    let mut offset: usize = 0; // used to adjust Span ranges

    // Pass 1 - `NostrUrl`s
    while let Some((start, end)) = find_nostr_url_pos(content) {
        let mut inner_segments = shatter_content_2(&content[..start]);
        apply_offset(&mut inner_segments, offset);
        segments.append(&mut inner_segments);

        // The Nostr Bech32 itself
        if let Some(nbech) = NostrBech32::try_from_string(&content[start + 6..end]) {
            segments.push(ContentSegment::NostrUrl(NostrUrl(nbech)));
        } else {
            segments.push(ContentSegment::Plain(Span { start, end }));
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
#[allow(clippy::string_slice)] // Regex positions are trusted
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

// Pass 3 - URLs
#[allow(clippy::string_slice)]
fn shatter_content_3(content: &str) -> Vec<ContentSegment> {
    let mut segments: Vec<ContentSegment> = Vec::new();

    for span in LinkFinder::new().kinds(&[LinkKind::Url]).spans(content) {
        if span.kind().is_some() {
            segments.push(ContentSegment::Hyperlink(Span {
                start: span.start(),
                end: span.end(),
            }));
        } else if !span.as_str().is_empty() {
            let mut inner_segments = shatter_content_4(&content[span.start()..span.end()]);
            apply_offset(&mut inner_segments, span.start());
            segments.append(&mut inner_segments);
        }
    }

    segments
}

// Pass 4 - Hashtags
#[allow(clippy::string_slice)]
fn shatter_content_4(content: &str) -> Vec<ContentSegment> {
    lazy_static! {
        static ref HTAG_RE: Regex =
            Regex::new(r"(?ms)(?:^|\s)(#[\w\p{Extended_Pictographic}]+)\b").unwrap();
    }

    let mut segments: Vec<ContentSegment> = Vec::new();

    let mut pos = 0;
    for cap in HTAG_RE.captures_iter(content) {
        let mat = cap.get(1).unwrap();
        if mat.start() > pos {
            segments.push(ContentSegment::Plain(Span {
                start: pos,
                end: mat.start(),
            }));
        }
        segments.push(ContentSegment::Hashtag(
            content[mat.start() + 1..mat.end()].to_owned(),
        ));
        pos = mat.end();
    }

    if pos < content.len() {
        segments.push(ContentSegment::Plain(Span {
            start: pos,
            end: content.len(),
        }));
    }

    segments
}

fn apply_offset(segments: &mut [ContentSegment], offset: usize) {
    for segment in segments.iter_mut() {
        match segment {
            ContentSegment::Hyperlink(span) => span.offset(offset),
            ContentSegment::Plain(span) => span.offset(offset),
            _ => {}
        }
    }
}

fn replace_urls_with_nostr(content: &str) -> String {
    const PATTERNS: &[(&str, &str)] = &[
        ("https://njump.me/npub1", "nostr:npub1"),
        ("https://njump.me/nprofile1", "nostr:nprofile1"),
        ("https://njump.me/nevent1", "nostr:nevent1"),
        ("https://njump.me/naddr1", "nostr:naddr1"),
        ("https://primal.net/e/note1", "nostr:note1"),
        ("https://primal.net/e/naddr1", "nostr:naddr1"),
        ("https://primal.net/e/nevent1", "nostr:nevent1"),
        ("https://primal.net/p/npub1", "nostr:npub1"),
        ("https://primal.net/p/nprofile1", "nostr:nprofile1"),
        ("https://nostrudel.ninja/#/u/npub1", "nostr:npub1"),
        ("https://nostrudel.ninja/u/npub1", "nostr:npub1"),
        ("https://yakihonne.com/article/naddr1", "nostr:naddr1"),
        ("https://yakihonne.com/users/npub1", "nostr:npub1"),
        ("https://damus.io/note1", "nostr:note1"),
        ("https://damus.io/npub1", "nostr:npub1"),
        ("https://damus.io/naddr1", "nostr:naddr1"),
        ("https://damus.io/nevent1", "nostr:nevent1"),
        ("https://damus.io/nprofile1", "nostr:nprofile1"),
        ("https://listr.lol/npub1", "nostr:npub1"),
        ("https://nostr.band/npub1", "nostr:npub1"),
        ("https://zap.stream/naddr1", "nostr:naddr1"),
        ("https://tunestr.io/naddr1", "nostr:naddr1"),
        ("https://zap.cooking/recipe/naddr1", "nostr:naddr1"),
        ("https://nostrnests.com/naddr1", "nostr:naddr1"),
        ("https://nostr.com/nprofile1", "nostr:nprofile1"),
        ("https://coracle.social/nprofile1", "nostr:nprofile1"),
    ];

    lazy_static! {
        static ref INPUTS: Vec<&'static str> = PATTERNS.iter().map(|(input, _)| *input).collect();
        static ref OUTPUTS: Vec<&'static str> =
            PATTERNS.iter().map(|(_, output)| *output).collect();
        static ref AHO: AhoCorasick = AhoCorasick::new(INPUTS.iter()).unwrap();
    }

    AHO.replace_all(content, &OUTPUTS)
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_shatter_content() {
        let content_str = "My friend #[0]  wrote me this note: nostr:note10ttnuuvcs29y3k23gwrcurw2ksvgd7c2rrqlfx7urmt5m963vhss8nja90 and it might have referred to https://github.com/Giszmo/nostr.info/blob/master/assets/js/main.js";
        let content = content_str.to_string();
        let pieces = ShatteredContent::new(content, false);
        assert_eq!(pieces.segments.len(), 6);
        assert!(matches!(pieces.segments[0], ContentSegment::Plain(..)));
        assert!(matches!(
            pieces.segments[1],
            ContentSegment::TagReference(..)
        ));
        assert!(matches!(pieces.segments[2], ContentSegment::Plain(..)));
        assert!(matches!(pieces.segments[3], ContentSegment::NostrUrl(..)));
        assert!(matches!(pieces.segments[4], ContentSegment::Plain(..)));
        assert!(matches!(pieces.segments[5], ContentSegment::Hyperlink(..)));

        let content_str = r#"This is a test of NIP-27 posting support referencing this note nostr:nevent1qqsqqqq9wh98g4u6e480vyp6p4w3ux2cd0mxn2rssq0w5cscsgzp2ksprpmhxue69uhkzapwdehhxarjwahhy6mn9e3k7mf0qyt8wumn8ghj7etyv4hzumn0wd68ytnvv9hxgtcpremhxue69uhkummnw3ez6ur4vgh8wetvd3hhyer9wghxuet59uq3kamnwvaz7tmwdaehgu3wd45kketyd9kxwetj9e3k7mf0qy2hwumn8ghj7mn0wd68ytn00p68ytnyv4mz7qgnwaehxw309ahkvenrdpskjm3wwp6kytcpz4mhxue69uhhyetvv9ujuerpd46hxtnfduhsz9mhwden5te0wfjkccte9ehx7um5wghxyctwvshszxthwden5te0wfjkccte9eekummjwsh8xmmrd9skctcnmzajy and again without the url data nostr:note1qqqq2aw2w3te4n2w7cgr5r2arcv4s6lkdx58pqq7af3p3qsyz4dqns2935
And referencing this person nostr:npub1acg6thl5psv62405rljzkj8spesceyfz2c32udakc2ak0dmvfeyse9p35c and again as an nprofile nostr:nprofile1qqswuyd9ml6qcxd92h6pleptfrcqucvvjy39vg4wx7mv9wm8kakyujgprdmhxue69uhkummnw3ezumtfddjkg6tvvajhytnrdakj7qg7waehxw309ahx7um5wgkhqatz9emk2mrvdaexgetj9ehx2ap0qythwumn8ghj7un9d3shjtnwdaehgu3wd9hxvme0qyt8wumn8ghj7etyv4hzumn0wd68ytnvv9hxgtcpzdmhxue69uhk7enxvd5xz6tw9ec82c30qy2hwumn8ghj7mn0wd68ytn00p68ytnyv4mz7qgcwaehxw309ashgtnwdaehgunhdaexkuewvdhk6tczkvt9n all on the same damn line even (I think)."#;
        let content = content_str.to_string();
        let pieces = ShatteredContent::new(content, false);
        assert_eq!(pieces.segments.len(), 9);
    }

    #[test]
    fn test_shatter_content_2() {
        let content_str =
            "Ein wunderschönes langes Wochenende auf der #zitadelle2024 geht zu Ende...
🏰 #einundzwanzig
Hier einige Impressionen mit opsec gewährten Bildern.
Wonderful Long Weekend at a Zitadelle, Here Impressions opsec included
 nostr:npub1vwf2mytkyk22x2gcmr9d7k";
        let content = content_str.to_string();
        let pieces = ShatteredContent::new(content, false);
        assert_eq!(pieces.segments.len(), 6);
        assert!(matches!(pieces.segments[2], ContentSegment::Plain(..)));
        assert!(matches!(pieces.segments[4], ContentSegment::Plain(..)));
        if let ContentSegment::Plain(span) = pieces.segments[5] {
            let _slice = pieces.slice(&span);
        }
    }

    #[test]
    fn test_shatter_content_3() {
        let content_str = "Check this out https://primal.net/e/note10ttnuuvcs29y3k23gwrcurw2ksvgd7c2rrqlfx7urmt5m963vhss8nja90";
        let content = content_str.to_string();
        let pieces = ShatteredContent::new(content, true);
        assert_eq!(pieces.segments.len(), 2);
        assert!(matches!(pieces.segments[1], ContentSegment::NostrUrl(_)));
    }

    #[test]
    fn test_shatter_content_4() {
        let content_str = "#happy this is crazy #mad #dog";
        let content = content_str.to_string();
        let pieces = ShatteredContent::new(content, true);
        for piece in pieces.segments.iter() {
            println!(">{:?}<", piece);
        }
        assert_eq!(pieces.segments.len(), 5);
        assert_eq!(
            pieces.segments[0],
            ContentSegment::Hashtag("happy".to_owned())
        );
        assert_eq!(
            pieces.segments[1],
            ContentSegment::Plain(Span { start: 6, end: 21 })
        );
        assert_eq!(
            pieces.segments[2],
            ContentSegment::Hashtag("mad".to_owned())
        );
        assert_eq!(
            pieces.segments[3],
            ContentSegment::Plain(Span { start: 25, end: 26 })
        );
        assert_eq!(
            pieces.segments[4],
            ContentSegment::Hashtag("dog".to_owned())
        );
    }
}
