use crate::error::Error;
use crate::globals::GLOBALS;
use crate::USER_AGENT;
use nostr_types::Url;
use reqwest::Client;
use sha2::Digest;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::ErrorKind;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::RwLock;
use tokio::task;

#[derive(Debug, Default)]
pub struct Fetcher {
    // we don't want new() to fail in lazy_static init, so we just mark it dead if there was an error
    // on creation
    dead: Option<String>,

    cache_dir: PathBuf,
    client: Client,

    // We use std::sync::RwLock because this isn't used in async code
    pending: RwLock<HashSet<Url>>,
    failed: RwLock<HashMap<Url, Error>>,

    pub requests_in_flight: AtomicUsize,
}

impl Fetcher {
    pub fn new() -> Fetcher {
        let connect_timeout = std::time::Duration::new(30, 0);
        let timeout = std::time::Duration::new(300, 0);
        let client = match Client::builder()
            .gzip(true)
            .brotli(true)
            .deflate(true)
            .connect_timeout(connect_timeout)
            .timeout(timeout)
            .build()
        {
            Ok(c) => c,
            Err(e) => {
                return Fetcher {
                    dead: Some(format!("{}", e)),
                    ..Default::default()
                }
            }
        };

        let mut f: Fetcher = Fetcher {
            client,
            ..Default::default()
        };

        // Setup the cache directory
        let mut cache_dir = match dirs::data_dir() {
            Some(d) => d,
            None => {
                f.dead = Some("No Data Directory.".to_owned());
                return f;
            }
        };
        cache_dir.push("gossip");
        cache_dir.push("cache");

        // Create our data directory only if it doesn't exist
        if let Err(e) = fs::create_dir_all(&cache_dir) {
            f.dead = Some(format!("{}", e));
            return f;
        }

        f.cache_dir = cache_dir;
        f
    }

    fn cache_file(&self, url: &Url) -> PathBuf {
        // Hash the url into a SHA256 hex string
        let hash = {
            let mut hasher = sha2::Sha256::new();
            hasher.update(url.0.as_bytes());
            let result = hasher.finalize();
            hex::encode(result)
        };

        let mut cache_file = self.cache_dir.clone();
        cache_file.push(hash);
        cache_file
    }

    pub fn try_get(&self, url: Url) -> Result<Option<Vec<u8>>, Error> {
        // Error if we are dead
        if let Some(reason) = &self.dead {
            return Err((format!("Fetcher is dead: {}", reason), file!(), line!()).into());
        }

        // Error if we couldn't fetch this item
        if let Some(error) = self.failed.read().unwrap().get(&url) {
            return Err((format!("{}", error), file!(), line!()).into());
        }

        // Pending if we are trying to fetch this item
        if self.pending.read().unwrap().contains(&url) {
            return Ok(None);
        }

        // Try to get it from the cache file
        // FIXME - even this can be time consuming and should be synced instead of tried
        //         directly, especially on spinning hard drives.
        let cache_file = self.cache_file(&url);
        match fs::read(cache_file) {
            Ok(contents) => {
                return Ok(Some(contents));
            }
            Err(e) => {
                // Any error other than this falls through
                if e.kind() != ErrorKind::NotFound {
                    return Err(e.into());
                }
            }
        }

        // Do not fetch if offline
        if GLOBALS.settings.read().offline {
            return Ok(None);
        }

        // We can't fetch as we are not async and we don't want to block the caller.
        // So we save this request as pending, and then spawn a task to get it.
        self.pending.write().unwrap().insert(url.clone());

        task::spawn(async move {
            if let Err(e) = Fetcher::fetch(url.clone()).await {
                tracing::error!("Problem fetching from web: {}: {}", e, &url);
                // Add to errors
                GLOBALS
                    .fetcher
                    .failed
                    .write()
                    .unwrap()
                    .insert(url.clone(), e);
            }
            // Remove from pending
            GLOBALS.fetcher.pending.write().unwrap().remove(&url);
        });

        Ok(None)
    }

    pub async fn fetch(url: Url) -> Result<(), Error> {
        // Error if we are dead
        if let Some(reason) = &GLOBALS.fetcher.dead {
            return Err((format!("Fetcher is dead: {}", reason), file!(), line!()).into());
        }


        let client = GLOBALS.fetcher.client.clone();

        GLOBALS
            .fetcher
            .requests_in_flight
            .fetch_add(1, Ordering::SeqCst);

        // Fetch the resource
        let req = client.get(&url.0);

        let req = if GLOBALS.settings.read().set_user_agent {
            req.header("User-Agent", USER_AGENT)
        } else {
            req
        };

        let maybe_response = req.send().await;

        // Deal with response errors
        let response = match maybe_response {
            Ok(r) => r,
            Err(e) => {
                GLOBALS
                    .fetcher
                    .requests_in_flight
                    .fetch_sub(1, Ordering::SeqCst);
                return Err(e.into());
            }
        };

        // Convert to bytes
        let maybe_bytes = response.bytes().await;

        GLOBALS
            .fetcher
            .requests_in_flight
            .fetch_sub(1, Ordering::SeqCst);

        let bytes = maybe_bytes?;

        GLOBALS.bytes_read.fetch_add(bytes.len(), Ordering::Relaxed);

        let cache_file = GLOBALS.fetcher.cache_file(&url);

        // Write to the file
        fs::write(cache_file, bytes)?;

        Ok(())
    }
}
