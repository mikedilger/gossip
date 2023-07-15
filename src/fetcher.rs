use crate::error::{Error, ErrorKind};
use crate::globals::GLOBALS;
use crate::profile::Profile;
use crate::USER_AGENT;
use nostr_types::Url;
use rand::Rng;
use reqwest::header::ETAG;
use reqwest::{Client, StatusCode};
use sha2::Digest;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::RwLock;
use std::time::Duration;
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
        let cache_dir = match Profile::current() {
            Ok(p) => p.cache_dir,
            Err(_) => {
                f.dead = Some("No Data Directory.".to_owned());
                return f;
            }
        };

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

    fn etag_file(&self, url: &Url) -> PathBuf {
        self.cache_file(url).with_extension("etag")
    }

    // This is used so that all the previously fetched data doesn't expire
    // at the same time (actually, after a few days it does anyways)
    pub fn random_age(min_secs: u64, max_secs: u64) -> Duration {
        let mut rng = rand::thread_rng();
        let range = rand::distributions::Uniform::new(min_secs, max_secs);
        Duration::from_secs(rng.sample(range))
    }

    pub fn try_get(&self, url: Url, max_age: Duration) -> Result<Option<Vec<u8>>, Error> {
        // FIXME - this function is called synchronously, but it makes several
        //         file system calls. This might be pushing the limits of what we should
        //         be blocking on.

        // Error if we are dead
        if let Some(reason) = &self.dead {
            return Err((format!("Fetcher is dead: {}", reason), file!(), line!()).into());
        }

        // Error if we already couldn't fetch this item (we don't try again until restart)
        if let Some(error) = self.failed.read().unwrap().get(&url) {
            return Err((format!("{}", error), file!(), line!()).into());
        }

        // Pending if we are already trying to fetch this item
        if self.pending.read().unwrap().contains(&url) {
            return Ok(None);
        }

        // Check if a cached file exists and is fresh enough
        let cache_file = self.cache_file(&url);
        match fs::metadata(cache_file.as_path()) {
            Ok(md) => {
                if let Ok(modified) = md.modified() {
                    if let Ok(dur) = modified.elapsed() {
                        if dur < max_age {
                            match fs::read(cache_file) {
                                Ok(contents) => {
                                    return Ok(Some(contents));
                                }
                                Err(e) => return Err(e.into()),
                            }
                        }
                    }
                }
                // fall through
            }
            Err(e) => {
                // Probably NotFound, fail otherwise
                if e.kind() != std::io::ErrorKind::NotFound {
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

        let etag_file = GLOBALS.fetcher.etag_file(&url);
        let etag: Option<Vec<u8>> = match fs::read(etag_file.as_path()) {
            Ok(contents) => Some(contents),
            Err(_) => None,
        };

        let client = GLOBALS.fetcher.client.clone();

        GLOBALS
            .fetcher
            .requests_in_flight
            .fetch_add(1, Ordering::SeqCst);

        let mut req = client.get(&url.0);
        if let Some(ref etag) = etag {
            req = req.header("if-none-match", etag.to_owned());
        }
        if GLOBALS.settings.read().set_user_agent {
            req = req.header("User-Agent", USER_AGENT);
        };

        // Fetch the resource
        let maybe_response = req.send().await;

        GLOBALS
            .fetcher
            .requests_in_flight
            .fetch_sub(1, Ordering::SeqCst);

        // Deal with response errors
        let response = maybe_response?;

        if etag.is_some() && response.status() == StatusCode::NOT_MODIFIED {
            // It is already in the cache file, the etag matched

            // Update the file time to now, so we don't check again for a while
            filetime::set_file_mtime(etag_file.as_path(), filetime::FileTime::now())?;

            return Ok(());
        }

        if !response.status().is_success() {
            return Err(ErrorKind::General(format!(
                "Failed to fetch HTTP resource: {}",
                response.status()
            ))
            .into());
        }

        let maybe_etag = response
            .headers()
            .get(ETAG)
            .map(|e| e.as_bytes().to_owned());

        // Convert to bytes
        let maybe_bytes = response.bytes().await;
        let bytes = maybe_bytes?;
        GLOBALS.bytes_read.fetch_add(bytes.len(), Ordering::Relaxed);

        // Write to the file
        let cache_file = GLOBALS.fetcher.cache_file(&url);
        fs::write(cache_file, bytes)?;

        // If there was an etag, save it
        if let Some(etag) = maybe_etag {
            fs::write(etag_file, etag)?;
        }

        Ok(())
    }
}
