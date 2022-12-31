use crate::error::Error;
use crate::globals::GLOBALS;
use nostr_types::Url;
use reqwest::Client;
use sha2::Digest;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::ErrorKind;
use std::path::PathBuf;
use tokio::task;

pub enum FetcherResult {
    Success(Vec<u8>),
    Pending,
    Error(String),
}

pub struct Fetcher {
    // we don't want new() to fail in lazy_static init, so we just mark it dead if there was an error
    // on creation
    dead: Option<String>,

    cache_dir: PathBuf,
    client: Client,
    pending: HashSet<Url>,
    failed: HashMap<Url, Error>,
}

impl Fetcher {
    pub fn new() -> Fetcher {
        let mut f = Fetcher {
            dead: None,
            cache_dir: PathBuf::new(),
            client: Client::new(),
            pending: HashSet::new(),
            failed: HashMap::new(),
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
            hasher.update(url.inner().as_bytes());
            let result = hasher.finalize();
            hex::encode(result)
        };

        let mut cache_file = self.cache_dir.clone();
        cache_file.push(hash);
        cache_file
    }

    pub fn try_get(&mut self, url: Url) -> FetcherResult {
        // Error if we are dead
        if let Some(reason) = &self.dead {
            return FetcherResult::Error(format!("Fetcher is dead: {}", reason));
        }

        // Error if we couldn't fetch this item
        if let Some(error) = self.failed.get(&url) {
            return FetcherResult::Error(format!("{}", error));
        }

        // Pending if we are trying to fetch this item
        if self.pending.contains(&url) {
            return FetcherResult::Pending;
        }

        // Try to get it from the cache file
        // FIXME - even this can be time consuming and should be synced instead of tried
        //         directly, especially on spinning hard drives.
        let cache_file = self.cache_file(&url);
        match fs::read(&cache_file) {
            Ok(contents) => {
                tracing::debug!("Found web content at {}", cache_file.display());
                return FetcherResult::Success(contents);
            }
            Err(e) => {
                // Any error other than this falls through
                if e.kind() != ErrorKind::NotFound {
                    return FetcherResult::Error(format!("{}", e));
                }
            }
        }

        // We can't fetch as we are not async and we don't want to block the caller.
        // So we save this request as pending, and ask the syncer to sync us.
        self.pending.insert(url);
        let _ = GLOBALS.to_syncer.send("sync_fetcher".to_owned());
        FetcherResult::Pending
    }

    pub async fn sync(&mut self) -> Result<(), Error> {
        // Error if we are dead
        if let Some(reason) = &self.dead {
            return Err(Error::General(format!("Fetcher is dead: {}", reason)));
        }

        // FIXME: do these in parallel
        let urls = self.pending.clone();
        for url in urls.iter() {
            if let Err(e) = self.sync_inner(url.clone()).await {
                tracing::error!("{}", e);
                self.failed.insert(url.clone(), e);
                // leave it in pending too, it won't matter
            } else {
                // Remove it from pending
                self.pending.remove(url);
            }
        }
        Ok(())
    }

    async fn sync_inner(&mut self, url: Url) -> Result<(), Error> {
        let timeout = std::time::Duration::new(10, 0);

        let client = self.client.clone();
        let url2 = url.clone();
        let bytes =
            task::spawn(async move { client.get(url2.inner()).timeout(timeout).send().await })
                .await??
                .bytes()
                .await?;
        let cache_file = self.cache_file(&url);
        fs::write(cache_file, bytes)?;
        Ok(())
    }
}
