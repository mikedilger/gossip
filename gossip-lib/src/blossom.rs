use crate::error::{Error, ErrorKind};
use crate::globals::GLOBALS;
use base64::Engine;
use memmap2::Mmap;
use nostr_types::{EventKind, PreEvent, Tag, Unixtime};
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
    #[serde(rename = "uploaded")]
    pub created_at: u64,
}

pub struct Blossom {
    host: String,
    client: Client,
}

impl Blossom {
    pub fn new(host: String) -> Result<Blossom, Error> {
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

        Ok(Blossom { host, client })
    }

    /// BUD-01 HEAD /<sha256>
    /// Check if the data exists on the blossom server
    pub async fn check_exists(&self, hash: HashOutput, authorize: bool) -> Result<bool, Error> {
        let url = format!("https://{}/{}", self.host, hash);
        let mut req_builder = self.client.head(url);

        if authorize {
            let authorization = authorization(
                BlossomVerb::Get,
                "Check if exists".to_owned(),
                Unixtime::now() + Duration::new(60, 0),
                vec![],
            )?;

            req_builder = req_builder.header("Authorization", format!("Nostr {}", authorization))
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
    pub async fn download(&self, hash: HashOutput, authorize: bool) -> Result<Response, Error> {
        let url = format!("https://{}/{}", self.host, hash);
        let mut req_builder = self.client.get(url);

        if authorize {
            let authorization = authorization(
                BlossomVerb::Get,
                "Download".to_owned(),
                Unixtime::now() + Duration::new(60, 0),
                vec![],
            )?;

            req_builder = req_builder.header("Authorization", format!("Nostr {}", authorization))
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
        hash: HashOutput,
    ) -> Result<BlobDescriptor, Error> {
        let authorization = authorization(
            BlossomVerb::Upload,
            "Upload".to_owned(),
            Unixtime::now() + Duration::new(60, 0),
            vec![hash],
        )?;

        let url = format!("https://{}/upload", self.host);
        let response = self
            .client
            .put(url)
            .header("Authorization", format!("Nostr {}", authorization))
            .body(data)
            .send()
            .await?;

        if response.status().as_u16() < 300 {
            Ok(response.json::<BlobDescriptor>().await?)
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

    let pre_event = PreEvent {
        pubkey: public_key,
        created_at: Unixtime::now(),
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
