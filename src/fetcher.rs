use crate::error::{Error, ErrorKind};
use crate::globals::GLOBALS;
use crate::profile::Profile;
use crate::USER_AGENT;
use nostr_types::{Unixtime, Url};
use reqwest::header::ETAG;
use reqwest::Client;
use reqwest::StatusCode;
use sha2::Digest;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::sync::RwLock;
use std::time::Duration;

#[derive(Copy, Clone, Debug)]
pub enum FetchState {
    Queued,
    InFlight,
    Failed,
    // If it succeeds, it is removed entirely.
}

#[derive(Debug, Default)]
pub struct Fetcher {
    // we don't want new() to fail in lazy_static init, so we just mark it dead if there was an error
    // on creation
    dead: Option<String>,
    cache_dir: PathBuf,
    client: Client,

    // Here is where we store the current state of each URL being fetched
    urls: RwLock<HashMap<Url, FetchState>>,

    // Here is where we put hosts into a penalty box to time them out
    penalty_box: RwLock<HashMap<String, Unixtime>>,
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

    pub fn start() {
        // Setup periodic queue management
        tokio::task::spawn(async {
            loop {
                // Every second...
                tokio::time::sleep(Duration::from_millis(1000)).await;

                // Process the queue
                GLOBALS.fetcher.process_queue().await;
            }
        });
    }

    pub fn requests_queued(&self) -> usize {
        self.urls
            .read()
            .unwrap()
            .iter()
            .filter(|(_u, r)| matches!(r, FetchState::Queued))
            .count()
    }

    pub fn requests_in_flight(&self) -> usize {
        self.urls
            .read()
            .unwrap()
            .iter()
            .filter(|(_u, r)| matches!(r, FetchState::InFlight))
            .count()
    }

    pub async fn process_queue(&self) {
        if self.dead.is_some() {
            return;
        }
        if GLOBALS.settings.read().offline {
            return;
        }

        let now = Unixtime::now().unwrap();

        let mut count = 0;

        let mut queued_urls: Vec<Url> = Vec::new();

        {
            for (url, state) in self.urls.read().unwrap().iter() {
                if matches!(state, FetchState::Queued) {
                    if let Some(host) = self.host(url) {
                        let mut penalty_box = self.penalty_box.write().unwrap();
                        if let Some(time) = penalty_box.get(&*host) {
                            if time < &now {
                                // Remove from penalty box
                                penalty_box.remove(&*host);
                            } else {
                                continue; // We cannot dequeue this one
                            }
                        }

                        queued_urls.push(url.to_owned());
                    }
                }
            }
        }

        for url in queued_urls.drain(..) {
            count += 1;
            self.fetch(url).await;
        }

        if count > 0 {
            tracing::debug!("Fetcher de-queued {count} requests");
        }
    }

    /// This is where external code attempts to get the bytes of a file.
    pub fn try_get(&self, url: &Url, max_age: Duration) -> Result<Option<Vec<u8>>, Error> {
        // FIXME - this function is called synchronously, but it makes several
        //         file system calls. This might be pushing the limits of what we should
        //         be blocking on.

        // Error if we are dead
        if let Some(reason) = &self.dead {
            return Err((format!("Fetcher is dead: {}", reason), file!(), line!()).into());
        }

        // Do not fetch if offline
        if GLOBALS.settings.read().offline {
            return Ok(None);
        }

        // Get state
        let fetch_result: Option<FetchState> = self.urls.read().unwrap().get(url).copied();

        match fetch_result {
            Some(FetchState::InFlight) => {
                tracing::trace!("FETCH {url}: Already in flight");
                return Ok(None);
            }
            Some(FetchState::Failed) => {
                tracing::debug!("FETCH {url}: Resource failed last time. Not trying again.");
                return Err(ErrorKind::General(
                    "A previous attempt to fetch this resource failed.".to_string(),
                )
                .into());
            }
            Some(FetchState::Queued) => {
                tracing::trace!("FETCH {url}: Already queued.");
                return Ok(None);
            }
            _ => {} // fall through
        }

        // Check if a cached file exists and is fresh enough
        let cache_file = self.cache_file(url);
        match fs::metadata(cache_file.as_path()) {
            Ok(md) => {
                if let Ok(modified) = md.modified() {
                    if let Ok(dur) = modified.elapsed() {
                        if dur < max_age {
                            match fs::read(cache_file) {
                                Ok(contents) => {
                                    tracing::debug!(
                                        "FETCH {url}: Cache Hit age={}s",
                                        dur.as_secs()
                                    );
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
                // NotFound falls through
                if e.kind() != std::io::ErrorKind::NotFound {
                    tracing::info!("FETCH {url}: Failed: {e}");
                    return Err(e.into());
                }
            }
        }

        // We can't fetch as we are not async and we don't want to block the caller.
        // So we queue this request for now.
        self.urls
            .write()
            .unwrap()
            .insert(url.to_owned(), FetchState::Queued);
        tracing::debug!("FETCH {url}: Queued");

        Ok(None)
    }

    pub async fn fetch(&self, url: Url) {
        // Error if we are dead
        if GLOBALS.fetcher.dead.is_some() {
            // mark as failed
            tracing::debug!("FETCH {url}: Failed: fetcher is dead");
            self.urls.write().unwrap().insert(url, FetchState::Failed);
            return;
        }

        // Do not fetch if offline
        if GLOBALS.settings.read().offline {
            tracing::debug!("FETCH {url}: Failed: offline mode");
            self.urls.write().unwrap().insert(url, FetchState::Failed);
            return;
        }

        let etag_file = GLOBALS.fetcher.etag_file(&url);
        let etag: Option<Vec<u8>> = match fs::read(etag_file.as_path()) {
            Ok(contents) => Some(contents),
            Err(_) => None,
        };

        let client = GLOBALS.fetcher.client.clone(); // it is an Arc internally

        // Mark url as in-flight
        self.urls
            .write()
            .unwrap()
            .insert(url.clone(), FetchState::InFlight);

        // Fetch the resource
        let mut req = client.get(&url.0);
        if let Some(ref etag) = etag {
            req = req.header("if-none-match", etag.to_owned());
        }
        if GLOBALS.settings.read().set_user_agent {
            req = req.header("User-Agent", USER_AGENT);
        };

        let maybe_response = req.send().await;

        let cache_file = GLOBALS.fetcher.cache_file(&url);

        // Deal with response errors
        let response = match maybe_response {
            Ok(r) => r,
            Err(e) => {
                if e.is_builder() {
                    tracing::info!("FETCH {url}: Failed: {e}");
                    self.urls.write().unwrap().insert(url, FetchState::Failed);
                } else if e.is_timeout() {
                    tracing::info!("FETCH {url}: Re-Queued: timeout: {e}");
                    self.urls.write().unwrap().insert(url, FetchState::Queued);
                } else if e.is_request() {
                    tracing::info!("FETCH {url}: Failed: request error: {e}");
                    self.urls.write().unwrap().insert(url, FetchState::Failed);
                } else if e.is_connect() {
                    tracing::info!("FETCH {url}: Failed: connect error: {e}");
                    self.sinbin(&url, Duration::from_secs(60));
                    self.urls.write().unwrap().insert(url, FetchState::Failed);
                } else if e.is_body() {
                    tracing::info!("FETCH {url}: Failed: body error: {e}");
                    self.urls.write().unwrap().insert(url, FetchState::Failed);
                } else if e.is_decode() {
                    tracing::info!("FETCH {url}: Failed: decode error: {e}");
                    self.urls.write().unwrap().insert(url, FetchState::Failed);
                } else if let Some(status) = e.status() {
                    if status.is_informational() {
                        tracing::info!("FETCH {url}: Re-Queued: informational error: {e}");
                        self.urls.write().unwrap().insert(url, FetchState::Queued);
                    } else if status.is_success() {
                        tracing::info!("FETCH {url}: Re-Queued: success error: {e}");
                        self.urls.write().unwrap().insert(url, FetchState::Queued);
                    } else if status.is_redirection() {
                        if status == StatusCode::NOT_MODIFIED {
                            tracing::info!("FETCH {url}: Succeeded with NOT MODIFIED");
                            // Touch our cache file
                            let _ = filetime::set_file_mtime(cache_file, filetime::FileTime::now());
                            self.urls.write().unwrap().remove(&url);
                            return;
                        } else {
                            // Our client follows up to 10 redirects. This is a failure.
                            tracing::info!("FETCH {url}: Failed: redirection error: {e}");
                            self.urls.write().unwrap().insert(url, FetchState::Failed);
                        }
                    } else if status.is_server_error() {
                        tracing::info!("FETCH {url}: Re-Queued(600): server error: {e}");
                        self.sinbin(&url, Duration::from_secs(600)); // 10 minutes
                        self.urls.write().unwrap().insert(url, FetchState::Queued);
                    } else {
                        match status {
                            StatusCode::REQUEST_TIMEOUT => {
                                tracing::info!("FETCH {url}: Re-Queued(60): request timeout: {e}");
                                self.sinbin(&url, Duration::from_secs(60)); // 1 minutes
                                self.urls.write().unwrap().insert(url, FetchState::Queued);
                            }
                            StatusCode::TOO_MANY_REQUESTS => {
                                tracing::info!(
                                    "FETCH {url}: Re-Queued(30): too many requests: {e}"
                                );
                                self.sinbin(&url, Duration::from_secs(30)); // 30 seconds
                                self.urls.write().unwrap().insert(url, FetchState::Queued);
                            }
                            e => {
                                tracing::info!("FETCH {url}: Failed: other: {e}");
                                self.urls.write().unwrap().insert(url, FetchState::Failed);
                            }
                        }
                    }
                } else {
                    tracing::info!("FETCH {url}: Failed: other: {e}");
                    self.urls.write().unwrap().insert(url, FetchState::Failed);
                }
                return;
            }
        };

        let maybe_etag = response
            .headers()
            .get(ETAG)
            .map(|e| e.as_bytes().to_owned());

        // Convert to bytes
        let maybe_bytes = response.bytes().await;
        let bytes = match maybe_bytes {
            Ok(bytes) => bytes,
            Err(e) => {
                tracing::info!("FETCH {url}: Failed: response bytes: {e}");
                self.urls.write().unwrap().insert(url, FetchState::Failed);
                return;
            }
        };

        GLOBALS.bytes_read.fetch_add(bytes.len(), Ordering::Relaxed);

        // Write to the file
        if let Err(e) = fs::write(cache_file, bytes) {
            tracing::info!("FETCH {url}: Failed: writing to cache file: {e}");
            self.urls.write().unwrap().insert(url, FetchState::Failed);
            return;
        }

        tracing::debug!("FETCH {url}: Cached");

        // If there was an etag, save it
        if let Some(etag) = maybe_etag {
            let _ = fs::write(etag_file, etag);
        }

        self.urls.write().unwrap().remove(&url);
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

    fn sinbin(&self, url: &Url, duration: Duration) {
        let now = Unixtime::now().unwrap();
        let later = now + duration;
        let host = match self.host(url) {
            Some(h) => h,
            None => return,
        };

        // lock penalty box
        let mut penalty_box = self.penalty_box.write().unwrap();

        if let Some(time) = penalty_box.get_mut(&*host) {
            if *time < later {
                *time = later;
            }
        } else {
            penalty_box.insert(host, later);
        }
    }

    fn host(&self, url: &Url) -> Option<String> {
        let u = match url::Url::parse(&url.0) {
            Ok(u) => u,
            Err(_) => return None,
        };
        u.host_str().map(|s| s.to_owned())
    }

    fn etag_file(&self, url: &Url) -> PathBuf {
        self.cache_file(url).with_extension("etag")
    }
}
