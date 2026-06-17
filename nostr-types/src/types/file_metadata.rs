use crate::{Event, EventKind, PreEvent, PublicKey, Tag, UncheckedUrl, Unixtime};

/// NIP-92/94 File Metadata
#[derive(Clone, Debug, Hash, PartialEq)]
pub struct FileMetadata {
    /// The URL this metadata applies to
    pub url: UncheckedUrl,

    /// Mime type (lowercase), see https://developer.mozilla.org/en-US/docs/Web/HTTP/MIME_types/Common_types
    pub m: Option<String>,

    /// SHA-256 hex-encoded hash
    pub x: Option<String>,

    /// original SHA-256 hex-encoded hash prior to transformations
    pub ox: Option<String>,

    /// Size of file in bytes
    pub size: Option<u64>,

    /// Dimensions of the image
    pub dim: Option<(usize, usize)>,

    /// Magnet URI
    pub magnet: Option<UncheckedUrl>,

    /// Torrent infohash
    pub i: Option<String>,

    /// Blurhash
    pub blurhash: Option<String>,

    /// Thumbnail URL
    pub thumb: Option<UncheckedUrl>,

    /// Preview image (same dimensions)
    pub image: Option<UncheckedUrl>,

    /// Summary text
    pub summary: Option<String>,

    /// Alt description
    pub alt: Option<String>,

    /// Fallback URLs
    pub fallback: Vec<UncheckedUrl>,

    /// Service
    pub service: Option<String>,
}

impl FileMetadata {
    /// Create a new empty (except the URL) FileMetadata
    pub fn new(url: UncheckedUrl) -> FileMetadata {
        FileMetadata {
            url,
            m: None,
            x: None,
            ox: None,
            size: None,
            dim: None,
            magnet: None,
            i: None,
            blurhash: None,
            thumb: None,
            image: None,
            summary: None,
            alt: None,
            fallback: vec![],
            service: None,
        }
    }

    /// Create a NIP-94 FileMetadata PreEvent from this FileMetadata
    pub fn to_nip94_preevent(&self, pubkey: PublicKey) -> PreEvent {
        let mut tags = vec![Tag::new(&["url", &self.url.0])];

        if let Some(m) = &self.m {
            tags.push(Tag::new(&["m", m]));
        }

        if let Some(x) = &self.x {
            tags.push(Tag::new(&["x", x]));
        }

        if let Some(ox) = &self.ox {
            tags.push(Tag::new(&["ox", ox]));
        }

        if let Some(size) = self.size {
            tags.push(Tag::new(&["size", &format!("{size}")]));
        }

        if let Some(dim) = self.dim {
            tags.push(Tag::new(&["dim", &format!("{}x{}", dim.0, dim.1)]));
        }

        if let Some(magnet) = &self.magnet {
            tags.push(Tag::new(&["magnet", &magnet.0]));
        }

        if let Some(i) = &self.i {
            tags.push(Tag::new(&["i", i]));
        }

        if let Some(blurhash) = &self.blurhash {
            tags.push(Tag::new(&["blurhash", blurhash]));
        }

        if let Some(thumb) = &self.thumb {
            tags.push(Tag::new(&["thumb", &thumb.0]));
        }

        if let Some(image) = &self.image {
            tags.push(Tag::new(&["image", &image.0]));
        }

        if let Some(summary) = &self.summary {
            tags.push(Tag::new(&["summary", summary]));
        }

        if let Some(alt) = &self.alt {
            tags.push(Tag::new(&["alt", alt]));
        }

        for fallback in &self.fallback {
            tags.push(Tag::new(&["fallback", &fallback.0]));
        }

        if let Some(service) = &self.service {
            tags.push(Tag::new(&["service", service]));
        }

        PreEvent {
            pubkey,
            created_at: Unixtime::now(),
            kind: EventKind::FileMetadata,
            content: "".to_owned(),
            tags,
        }
    }

    /// Turn a kind-1063 (FileMetadata) event into a FileMetadata structure
    pub fn from_nip94_event(event: &Event) -> Option<FileMetadata> {
        if event.kind != EventKind::FileMetadata {
            return None;
        }

        let mut fm = FileMetadata::new(UncheckedUrl("".to_owned()));

        for tag in &event.tags {
            match tag.tagname() {
                "url" => fm.url = UncheckedUrl(tag.value().to_owned()),
                "m" => fm.m = Some(tag.value().to_owned()),
                "x" => fm.x = Some(tag.value().to_owned()),
                "ox" => fm.ox = Some(tag.value().to_owned()),
                "size" => {
                    if let Ok(u) = tag.value().parse::<u64>() {
                        fm.size = Some(u);
                    }
                }
                "dim" => {
                    let parts: Vec<&str> = tag.value().split('x').collect();
                    if parts.len() == 2 {
                        if let Ok(w) = parts[0].parse::<usize>() {
                            if let Ok(h) = parts[1].parse::<usize>() {
                                fm.dim = Some((w, h));
                            }
                        }
                    }
                }
                "magnet" => fm.magnet = Some(UncheckedUrl(tag.value().to_owned())),
                "i" => fm.i = Some(tag.value().to_owned()),
                "blurhash" => fm.blurhash = Some(tag.value().to_owned()),
                "thumb" => fm.thumb = Some(UncheckedUrl(tag.value().to_owned())),
                "image" => fm.image = Some(UncheckedUrl(tag.value().to_owned())),
                "summary" => fm.summary = Some(tag.value().to_owned()),
                "alt" => fm.alt = Some(tag.value().to_owned()),
                "fallback" => fm.fallback.push(UncheckedUrl(tag.value().to_owned())),
                "service" => fm.service = Some(tag.value().to_owned()),
                _ => continue,
            }
        }

        if !fm.url.0.is_empty() {
            Some(fm)
        } else {
            None
        }
    }

    /// Convert into an 'imeta' tag
    pub fn to_imeta_tag(&self) -> Tag {
        let mut tag = Tag::new(&["imeta"]);

        tag.push_value(format!("url {}", self.url));

        if let Some(m) = &self.m {
            tag.push_value(format!("m {}", m));
        }

        if let Some(x) = &self.x {
            tag.push_value(format!("x {}", x));
        }

        if let Some(ox) = &self.ox {
            tag.push_value(format!("ox {}", ox));
        }

        if let Some(size) = &self.size {
            tag.push_value(format!("size {}", size));
        }

        if let Some(dim) = &self.dim {
            tag.push_value(format!("dim {}x{}", dim.0, dim.1));
        }

        if let Some(magnet) = &self.magnet {
            tag.push_value(format!("magnet {}", magnet));
        }

        if let Some(i) = &self.i {
            tag.push_value(format!("i {}", i));
        }

        if let Some(blurhash) = &self.blurhash {
            tag.push_value(format!("blurhash {}", blurhash));
        }

        if let Some(thumb) = &self.thumb {
            tag.push_value(format!("thumb {}", thumb));
        }

        if let Some(image) = &self.image {
            tag.push_value(format!("image {}", image));
        }

        if let Some(summary) = &self.summary {
            tag.push_value(format!("summary {}", summary));
        }

        if let Some(alt) = &self.alt {
            tag.push_value(format!("alt {}", alt));
        }

        for fallback in &self.fallback {
            tag.push_value(format!("fallback {}", fallback));
        }

        if let Some(service) = &self.service {
            tag.push_value(format!("service {}", service));
        }

        tag
    }

    /// Import from an 'imeta' tag
    pub fn from_imeta_tag(tag: &Tag) -> Option<FileMetadata> {
        let mut fm = FileMetadata::new(UncheckedUrl("".to_owned()));

        for i in 0..tag.len() {
            let parts: Vec<&str> = tag.get_index(i).splitn(2, ' ').collect();
            if parts.len() < 2 {
                continue;
            }
            match parts[0] {
                "url" => fm.url = UncheckedUrl(parts[1].to_owned()),
                "m" => fm.m = Some(parts[1].to_owned()),
                "x" => fm.x = Some(parts[1].to_owned()),
                "ox" => fm.ox = Some(parts[1].to_owned()),
                "size" => {
                    if let Ok(u) = parts[1].parse::<u64>() {
                        fm.size = Some(u);
                    }
                }
                "dim" => {
                    let parts: Vec<&str> = parts[1].split('x').collect();
                    if parts.len() == 2 {
                        if let Ok(w) = parts[0].parse::<usize>() {
                            if let Ok(h) = parts[1].parse::<usize>() {
                                fm.dim = Some((w, h));
                            }
                        }
                    }
                }
                "magnet" => fm.magnet = Some(UncheckedUrl(parts[1].to_owned())),
                "i" => fm.i = Some(parts[1].to_owned()),
                "blurhash" => fm.blurhash = Some(parts[1].to_owned()),
                "thumb" => fm.thumb = Some(UncheckedUrl(parts[1].to_owned())),
                "image" => fm.image = Some(UncheckedUrl(parts[1].to_owned())),
                "summary" => fm.summary = Some(parts[1].to_owned()),
                "alt" => fm.alt = Some(parts[1].to_owned()),
                "fallback" => fm.fallback.push(UncheckedUrl(parts[1].to_owned())),
                "service" => fm.service = Some(parts[1].to_owned()),
                _ => continue,
            }
        }

        if !fm.url.0.is_empty() {
            Some(fm)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[tokio::test]
    async fn test_nip94_event() {
        let mut fm = FileMetadata::new(UncheckedUrl("https://nostr.build/blahblahblah".to_owned()));
        fm.x = Some("12345".to_owned());
        fm.service = Some("http".to_owned());
        fm.size = Some(10124);
        fm.alt = Some("a crackerjack".to_owned());

        use crate::{PrivateKey, Signer};
        let private_key = PrivateKey::generate();
        let public_key = private_key.public_key();

        let pre_event = fm.to_nip94_preevent(public_key);
        let event = private_key.sign_event(pre_event).await.unwrap();
        let fm2 = FileMetadata::from_nip94_event(&event).unwrap();

        assert_eq!(fm, fm2);
    }

    #[test]
    fn test_imeta_tag() {
        let mut fm = FileMetadata::new(UncheckedUrl("https://nostr.build/blahblahblah".to_owned()));
        fm.x = Some("12345".to_owned());
        fm.service = Some("http".to_owned());
        fm.size = Some(10124);
        fm.alt = Some("a crackerjack".to_owned());

        let tag = fm.to_imeta_tag();
        let fm2 = FileMetadata::from_imeta_tag(&tag).unwrap();
        assert_eq!(fm, fm2);
    }
}
