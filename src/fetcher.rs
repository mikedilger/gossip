use crate::error::{Error, ErrorKind};
use crate::globals::GLOBALS;
use crate::profile::Profile;
use crate::USER_AGENT;
use futures::stream::{FuturesUnordered, StreamExt};
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
    QueuedStale, // Queued, only fetching because cache is stale
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

    // Load currently applied to a host
    host_load: RwLock<HashMap<String, usize>>,

    // Here is where we put hosts into a penalty box to time them out
    penalty_box: RwLock<HashMap<String, Unixtime>>,
}

impl Fetcher {
    pub fn new() -> Fetcher {
        let connect_timeout = std::time::Duration::new(10, 0);
        let timeout = std::time::Duration::new(15, 0);
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
                // Every 1200 milliseconds...
                tokio::time::sleep(Duration::from_millis(1200)).await;

                // Process the queue
                GLOBALS.fetcher.process_queue().await;

                // Possibly shut down
                if GLOBALS.shutting_down.load(Ordering::Relaxed) {
                    tracing::info!("Fetcher shutting down.");
                    break;
                }
            }
        });
    }

    pub fn requests_queued(&self) -> usize {
        self.urls
            .read()
            .unwrap()
            .iter()
            .filter(|(_u, r)| {
                matches!(r, FetchState::Queued) || matches!(r, FetchState::QueuedStale)
            })
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

        let mut futures = FuturesUnordered::new();

        for (url, state) in self.urls.read().unwrap().iter() {
            if matches!(state, FetchState::Queued) || matches!(state, FetchState::QueuedStale) {
                if let Some(host) = self.host(url) {
                    {
                        let mut penalty_box = self.penalty_box.write().unwrap();
                        if let Some(time) = penalty_box.get(&*host) {
                            if time < &now {
                                // Remove from penalty box
                                penalty_box.remove(&*host);
                            } else {
                                continue; // We cannot dequeue this one
                            }
                        }
                    }

                    let load = self.fetch_host_load(&host);
                    if load >= 3 {
                        continue; // We cannot overload any given host
                    }

                    count += 1;
                    self.increment_host_load(&host);
                    futures.push(self.fetch(url.clone()));
                }
            }
        }

        if count > 0 {
            tracing::debug!("Fetcher de-queued {count} requests");
        }

        // Run them all together
        while (futures.next().await).is_some() {}
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
            Some(FetchState::Queued) | Some(FetchState::QueuedStale) => {
                tracing::trace!("FETCH {url}: Already queued.");
                return Ok(None);
            }
            _ => {} // fall through
        }

        // Check if a cached file exists and is fresh enough
        let cache_file = self.cache_file(url);
        let mut stale = false;
        match fs::metadata(cache_file.as_path()) {
            Ok(md) => {
                // We had a bug that put empty cache files in place (maybe we still have it).
                // In any case, if the file is empty, don't honor it and wipe any etag
                if md.len() == 0 {
                    let etag_file = GLOBALS.fetcher.etag_file(url);
                    let _ = fs::remove_file(etag_file);
                } else {
                    if let Ok(modified) = md.modified() {
                        if let Ok(dur) = modified.elapsed() {
                            if dur < max_age {
                                match fs::read(cache_file.as_path()) {
                                    Ok(contents) => {
                                        tracing::debug!(
                                            "FETCH {url}: Cache Hit age={}s",
                                            dur.as_secs()
                                        );
                                        return Ok(Some(contents));
                                    }
                                    Err(e) => return Err(e.into()),
                                }
                            } else {
                                stale = true;
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
        let state = if stale {
            FetchState::QueuedStale
        } else {
            FetchState::Queued
        };
        self.urls.write().unwrap().insert(url.to_owned(), state);

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
        let etag: Option<Vec<u8>> = match tokio::fs::read(etag_file.as_path()).await {
            Ok(contents) => Some(contents),
            Err(_) => None,
        };

        let stale = matches!(
            self.urls
                .read()
                .unwrap()
                .get(&url)
                .unwrap_or(&FetchState::Queued),
            FetchState::QueuedStale
        );

        let cache_file = GLOBALS.fetcher.cache_file(&url);

        let host = self.host(&url).unwrap();

        // Mark url as in-flight
        self.urls
            .write()
            .unwrap()
            .insert(url.clone(), FetchState::InFlight);

        // Fetch the resource
        let client = GLOBALS.fetcher.client.clone(); // it is an Arc internally
        let mut req = client.get(&url.0);
        if let Some(ref etag) = etag {
            req = req.header("if-none-match", etag.to_owned());
        }
        if GLOBALS.settings.read().set_user_agent {
            req = req.header("User-Agent", USER_AGENT);
        };

        enum FailOutcome {
            Fail,
            NotModified,
            Requeue,
        }

        // closure to run when finished (if we didn't succeed)
        let cache_file2 = cache_file.clone();
        let finish = |outcome, message, err: Option<Error>, sinbin_secs| {
            match outcome {
                FailOutcome::Fail => {
                    if stale {
                        if let Some(e) = err {
                            tracing::info!(
                                "FETCH {url}: Failed (using stale cache): {message}: {e}"
                            );
                        } else {
                            tracing::info!("FETCH {url}: Failed (using stale cache): {message}");
                        }
                        // FIXME: bumping the mtime might not be the best way to do this.
                        let _ = filetime::set_file_mtime(
                            cache_file2.as_path(),
                            filetime::FileTime::now(),
                        );
                        self.urls.write().unwrap().remove(&url);
                    } else {
                        if let Some(e) = err {
                            tracing::info!("FETCH {url}: Failed: {message}: {e}");
                        } else {
                            tracing::info!("FETCH {url}: Failed: {message}");
                        }
                        self.urls
                            .write()
                            .unwrap()
                            .insert(url.clone(), FetchState::Failed);
                    }
                }
                FailOutcome::NotModified => {
                    tracing::info!("FETCH {url}: Succeeded: {message}");
                    let _ =
                        filetime::set_file_mtime(cache_file2.as_path(), filetime::FileTime::now());
                    self.urls.write().unwrap().remove(&url);
                }
                FailOutcome::Requeue => {
                    if let Some(e) = err {
                        tracing::info!("FETCH {url}: Re-Queued: {message}: {e}");
                    } else {
                        tracing::info!("FETCH {url}: Re-Queued: {message}");
                    }
                    self.urls
                        .write()
                        .unwrap()
                        .insert(url.clone(), FetchState::Queued);
                }
            }
            if sinbin_secs > 0 {
                self.sinbin(&url, Duration::from_secs(sinbin_secs));
            }
            self.decrement_host_load(&host);
        };

        let maybe_response = req.send().await;

        // Deal with response errors
        let response = match maybe_response {
            Ok(r) => r,
            Err(e) => {
                if e.is_builder() {
                    finish(FailOutcome::Fail, "builder error", Some(e.into()), 0);
                } else if e.is_timeout() {
                    finish(FailOutcome::Requeue, "timeout", Some(e.into()), 30);
                } else if e.is_request() {
                    finish(FailOutcome::Fail, "request error", Some(e.into()), 0);
                } else if e.is_connect() {
                    finish(FailOutcome::Fail, "connect error", Some(e.into()), 15);
                } else if e.is_body() {
                    finish(FailOutcome::Fail, "body error", Some(e.into()), 0);
                } else if e.is_decode() {
                    finish(FailOutcome::Fail, "decode error", Some(e.into()), 0);
                } else {
                    finish(FailOutcome::Fail, "other response error", Some(e.into()), 0);
                }
                return;
            }
        };

        // Deal with status codes
        let status = response.status();
        if status.is_informational() {
            finish(FailOutcome::Requeue, "informational error", None, 30);
            return;
        } else if status.is_redirection() {
            if status == StatusCode::NOT_MODIFIED {
                finish(FailOutcome::NotModified, "not modified", None, 0);
            } else {
                // Our client follows up to 10 redirects. This is a failure.
                finish(FailOutcome::Fail, "redirection error", None, 0);
            }
            return;
        } else if status.is_server_error() {
            finish(FailOutcome::Requeue, "server error", None, 300);
            return;
            // give them 5 minutes, maybe the server will recover
        } else if status.is_success() {
            // fall through
        } else {
            match status {
                StatusCode::REQUEST_TIMEOUT => {
                    finish(FailOutcome::Requeue, "request timeout", None, 30);
                    // give 30 seconds and try again
                }
                StatusCode::TOO_MANY_REQUESTS => {
                    finish(FailOutcome::Requeue, "too many requests", None, 30);
                    // give 15 seconds and try again
                }
                _ => {
                    finish(FailOutcome::Fail, &format!("{}", status), None, 0);
                }
            }
            return;
        }

        // Only fall through if we expect a response from the status code

        let maybe_etag = response
            .headers()
            .get(ETAG)
            .map(|e| e.as_bytes().to_owned());

        // Convert to bytes
        let maybe_bytes = response.bytes().await;
        let bytes = match maybe_bytes {
            Ok(bytes) => bytes,
            Err(e) => {
                finish(FailOutcome::Fail, "response bytes", Some(e.into()), 0);
                return;
            }
        };

        // Do not accept zero-length files, and don't try again
        if bytes.is_empty() {
            finish(FailOutcome::Fail, "zero length file", None, 10);
            return;
        }

        GLOBALS.bytes_read.fetch_add(bytes.len(), Ordering::Relaxed);

        // Write to the file
        if let Err(e) = tokio::fs::write(cache_file.as_path(), bytes).await {
            finish(
                FailOutcome::Fail,
                "writing to cache file",
                Some(e.into()),
                0,
            );
            return;
        }

        tracing::debug!("FETCH {url}: Cached");

        // Remove from host load
        self.decrement_host_load(&host);

        // If there was an etag, save it
        if let Some(etag) = maybe_etag {
            let _ = tokio::fs::write(etag_file, etag).await;
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

    fn fetch_host_load(&self, host: &str) -> usize {
        let hashmap = self.host_load.read().unwrap();
        if let Some(load) = hashmap.get(host) {
            *load
        } else {
            0
        }
    }

    fn increment_host_load(&self, host: &str) {
        let mut hashmap = self.host_load.write().unwrap();
        if let Some(load) = hashmap.get_mut(host) {
            *load += 1;
        } else {
            hashmap.insert(host.to_string(), 1);
        }
    }

    fn decrement_host_load(&self, host: &str) {
        let mut hashmap = self.host_load.write().unwrap();
        if let Some(load) = hashmap.get_mut(host) {
            if *load == 1 {
                hashmap.remove(host);
            } else {
                *load -= 1;
            }
        }
    }
}
