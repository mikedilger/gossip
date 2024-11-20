use crate::error::{Error, ErrorKind};
use crate::globals::GLOBALS;
use base64::Engine;
use memmap2::Mmap;
use mime::Mime;
use nostr_types::{EventKind, PreEvent, Tag, Unixtime};
use reqwest::header::{AUTHORIZATION, CONTENT_LENGTH, CONTENT_TYPE};
use reqwest::{Body, Client, Response};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fmt;
use std::fs::File;
use std::path::Path;
use std::time::Duration;

/// A simple type for a SHA-256 hash output of 32 bytes
pub struct HashOutput([u8; 32]);

impl HashOutput {
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<HashOutput, Error> {
        let sha256hash = {
            let file = File::open(path)?;
            let mmap = unsafe { Mmap::map(&file)? };
            let mut hasher = Sha256::new();
            hasher.update(&mmap[..]);
            hasher.finalize()
        };

        Ok(HashOutput(sha256hash.into()))
    }
}

impl fmt::Display for HashOutput {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", hex::encode(self.0))
    }
}

/// Blossom operation
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BlossomVerb {
    /// Get (download) data
    Get,

    /// Upload data
    Upload,

    /// List data
    List,

    /// Delete an upload
    Delete,
}

impl fmt::Display for BlossomVerb {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            BlossomVerb::Get => write!(f, "get"),
            BlossomVerb::Upload => write!(f, "upload"),
            BlossomVerb::List => write!(f, "list"),
            BlossomVerb::Delete => write!(f, "delete"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlobDescriptor {
    /// A URL that it can be downloaded from
    pub url: String,

    /// The SHA-256 hash
    pub sha256: String,

    /// The size of the file
    pub size: u64,

    /// The mime type
    #[serde(rename = "type")]
    #[serde(default)]
    pub mime_type: Option<String>,

    /// When uploaded
    #[serde(default)]
    pub uploaded: Option<u64>,

    /// When uploaded (alternate)
    #[serde(default)]
    pub created: Option<u64>,
}

pub struct Blossom {
    client: Client,
}

impl Blossom {
    pub fn new() -> Result<Blossom, Error> {
        let connect_timeout =
            Duration::new(GLOBALS.db().read_setting_fetcher_connect_timeout_sec(), 0);
        let timeout = Duration::new(GLOBALS.db().read_setting_fetcher_timeout_sec(), 0);

        let client = Client::builder()
            .gzip(false)
            .brotli(false)
            .deflate(false)
            .connect_timeout(connect_timeout)
            .timeout(timeout)
            .build()?;

        Ok(Blossom { client })
    }

    /// BUD-01 HEAD /<sha256>
    /// Check if the data exists on the blossom server
    pub async fn check_exists(
        &self,
        host: String,
        hash: HashOutput,
        authorize: bool,
    ) -> Result<bool, Error> {
        let url = format!("https://{}/{}", host, hash);
        let mut req_builder = self.client.head(url);

        if authorize {
            let authorization = authorization(
                BlossomVerb::Get,
                "Check if exists".to_owned(),
                Unixtime::now() + Duration::new(60, 0),
                vec![],
            )?;

            req_builder = req_builder.header(AUTHORIZATION, format!("Nostr {}", authorization))
        };

        let response = req_builder.send().await?;

        if response.status().as_u16() == 200 {
            Ok(true)
        } else {
            Err(get_error(&response))
        }
    }

    /// BUD-01 GET /<sha256>
    /// This returns the Response so it can be extracted as the caller desires
    /// with bytes(), bytes_stream(), chunk(), text(), or text_with_charset()
    pub async fn download(
        &self,
        host: String,
        hash: HashOutput,
        authorize: bool,
    ) -> Result<Response, Error> {
        let url = format!("https://{}/{}", host, hash);
        let mut req_builder = self.client.get(url);

        if authorize {
            let authorization = authorization(
                BlossomVerb::Get,
                "Download".to_owned(),
                Unixtime::now() + Duration::new(60, 0),
                vec![],
            )?;

            req_builder = req_builder.header(AUTHORIZATION, format!("Nostr {}", authorization))
        };

        let response = req_builder.send().await?;

        if response.status().as_u16() < 300 {
            Ok(response)
        } else {
            Err(get_error(&response))
        }
    }

    // BUD-08   HEAD /upload
    //pub async fn check_upload() {
    //unimplemented!()
    //}

    /// BUD-02   PUT /upload
    pub async fn upload<T: Into<Body>>(
        &self,
        data: T,
        host: String,
        hash: HashOutput,
        content_type: Mime,
        content_length: u64,
    ) -> Result<BlobDescriptor, Error> {
        let authorization = authorization(
            BlossomVerb::Upload,
            "Upload".to_owned(),
            Unixtime::now() + Duration::new(60, 0),
            vec![hash],
        )?;

        let url = format!("https://{}/upload", host);
        let response = self
            .client
            .put(url)
            .header(AUTHORIZATION, format!("Nostr {}", authorization))
            .header(CONTENT_TYPE, format!("{}", content_type))
            .header(CONTENT_LENGTH, content_length)
            .body(data)
            .send()
            .await?;

        if response.status().as_u16() < 300 {
            let full = response.bytes().await?;
            match serde_json::from_slice::<BlobDescriptor>(&full) {
                Ok(bd) => Ok(bd),
                Err(e) => {
                    let text = String::from_utf8_lossy(&full);
                    tracing::error!("Failed to deserialize Blossom Blob Descriptor: {}", text);
                    return Err(e.into());
                }
            }
        } else {
            Err(get_error(&response))
        }
    }

    // BUD-04  PUT /mirror
    //pub async fn mirror() {
    //
    //}

    // BUD-02  GET /list/<pubkey>
    //pub async fn list() {
    //    unimplemented!()
    //}

    // BUD-02  DELETE /<sha256>
    //pub async fn delete() {
    //    unimplemented!()
    //}
}

// This returns the base64 encoded authorization event
fn authorization(
    verb: BlossomVerb,
    purpose: String,
    expiration: Unixtime,
    hashes: Vec<HashOutput>,
) -> Result<String, Error> {
    let public_key = match GLOBALS.identity.public_key() {
        Some(pk) => pk,
        None => return Err(ErrorKind::NoPublicKey.into()),
    };

    let mut tags: Vec<Tag> = Vec::new();
    tags.push(Tag::new_hashtag(format!("{}", verb)));
    tags.push(Tag::new(&["expiration", &format!("{}", expiration)]));
    for hash in &hashes {
        tags.push(Tag::new(&["x", &format!("{}", hash)]));
    }

    let now = Unixtime::now();
    let pre_event = PreEvent {
        pubkey: public_key,
        created_at: now,
        kind: EventKind::Blossom, // 24242
        tags,
        content: purpose,
    };

    let event = GLOBALS.identity.sign_event(pre_event)?;
    let event_json = serde_json::to_string(&event)?;
    let base64 = base64::engine::general_purpose::STANDARD.encode(&event_json);

    Ok(base64)
}

fn get_error(response: &Response) -> Error {
    if let Some(hval) = response.headers().get("x-reason") {
        if let Ok(error_message) = hval.to_str() {
            ErrorKind::BlossomError(error_message.to_owned()).into()
        } else {
            ErrorKind::BlossomError(format!("{}", response.status())).into()
        }
    } else {
        ErrorKind::BlossomError(format!("{}", response.status())).into()
    }
}

/// This first infers the content-type by the magic number of the content
/// Then it uses the file extension
/// It falls back to application/octet-stream
pub fn get_content_type(path: &Path) -> Result<Mime, Error> {
    if let Some(mime) = infer::get_from_path(path)? {
        Ok(mime.mime_type().parse().unwrap())
    } else {
        let extension_guess = mime_guess::from_path(path);
        Ok(extension_guess
            .first()
            .unwrap_or(mime::APPLICATION_OCTET_STREAM))
    }
}
