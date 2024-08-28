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
use std::time::{Duration, SystemTime};
use tokio::sync::RwLock;

#[derive(Copy, Clone, Debug)]
enum FetchState {
    Queued(bool), // Queued, and whether we are using the temporary cache
    QueuedStale,  // Queued, only fetching because cache is stale (permanent cache)
    InFlight,     // InFlight, and whether we are using the temporary cache
    Failed,
    // If it succeeds, it's fetch state is removed entirely from our url map
}

/// System that fetches HTTP resources
#[derive(Debug, Default)]
pub struct Fetcher {
    cache_dir: RwLock<PathBuf>,
    tmp_cache_dir: RwLock<PathBuf>,

    client: RwLock<Option<Client>>,

    // Here is where we store the current state of each URL being fetched
    urls: RwLock<HashMap<Url, FetchState>>,

    // Load currently applied to a host
    host_load: RwLock<HashMap<String, usize>>,

    // Here is where we put hosts into a penalty box to time them out
    penalty_box: RwLock<HashMap<String, Unixtime>>,
}

impl Fetcher {
    pub(crate) fn new() -> Fetcher {
        Fetcher {
            ..Default::default()
        }
    }

    pub(crate) async fn init() -> Result<(), Error> {
        // Copy profile directories so we don't have to deal with the rare
        // initialization error every time we use them
        *GLOBALS.fetcher.cache_dir.write().await = Profile::cache_dir(false)?;
        *GLOBALS.fetcher.tmp_cache_dir.write().await = Profile::cache_dir(true)?;

        // Create client
        let connect_timeout =
            std::time::Duration::new(GLOBALS.db().read_setting_fetcher_connect_timeout_sec(), 0);

        let timeout = std::time::Duration::new(GLOBALS.db().read_setting_fetcher_timeout_sec(), 0);

        *GLOBALS.fetcher.client.write().await = Some(
            Client::builder()
                .gzip(true)
                .brotli(true)
                .deflate(true)
                .connect_timeout(connect_timeout)
                .timeout(timeout)
                .build()?,
        );

        Ok(())
    }

    /// Count of HTTP requests queued for future fetching
    pub async fn requests_queued(&self) -> usize {
        self.urls
            .read()
            .await
            .iter()
            .filter(|(_u, r)| {
                matches!(r, FetchState::Queued(_)) || matches!(r, FetchState::QueuedStale)
            })
            .count()
    }

    /// Count of HTTP requests currently being serviced
    pub async fn requests_in_flight(&self) -> usize {
        self.urls
            .read()
            .await
            .iter()
            .filter(|(_u, r)| matches!(r, FetchState::InFlight))
            .count()
    }

    pub(crate) async fn process_queue(&self) {
        // Initialize if not already
        if GLOBALS.fetcher.client.read().await.is_none() {
            if let Err(e) = Self::init().await {
                tracing::error!("Fetcher failed to initialize: {e}");
                return;
            }
        }

        let now = Unixtime::now();

        let mut count = 0;

        let mut futures = FuturesUnordered::new();

        for (url, state) in self.urls.read().await.iter() {
            let (doit, use_temp_cache) = if let FetchState::Queued(use_temp_cache) = state {
                (true, *use_temp_cache)
            } else if matches!(state, FetchState::QueuedStale) {
                (true, false)
            } else {
                (false, false)
            };

            if doit {
                if let Some(host) = self.host(url) {
                    {
                        let mut penalty_box = self.penalty_box.write().await;
                        if let Some(time) = penalty_box.get(&*host) {
                            if time < &now {
                                // Remove from penalty box
                                penalty_box.remove(&*host);
                            } else {
                                continue; // We cannot dequeue this one
                            }
                        }
                    }

                    let load = self.fetch_host_load(&host).await;
                    if load >= GLOBALS.db().read_setting_fetcher_max_requests_per_host() {
                        continue; // We cannot overload any given host
                    }

                    count += 1;
                    self.increment_host_load(&host);
                    futures.push(self.fetch(url.clone(), use_temp_cache));
                }
            }
        }

        if count > 0 {
            tracing::debug!("Fetcher de-queued {count} requests");
        }

        // Run them all together
        let mut read_runstate = GLOBALS.read_runstate.clone();
        read_runstate.mark_unchanged();
        while (futures.next().await).is_some() {
            if read_runstate.borrow().going_offline() {
                break;
            }
        }
    }

    /// This is where other parts of the library attempt to get the bytes of a file.
    ///
    /// If it is missing:  You'll get an `Ok(None)` response, but the fetcher will then
    /// work in the background to try to make it available for a future call.
    ///
    /// If it is available: You'll get `Ok(Some(bytes))`. This will read from the file system.
    /// If you call it over and over rapidly (e.g. from the UI), it will read from the filesystem
    /// over and over again, which is bad. So the UI caller should have it's own means of
    /// caching the results from this call.
    pub(crate) async fn try_get(
        &self,
        url: &Url,
        max_age: Duration,
        use_temp_cache: bool,
    ) -> Result<Option<Vec<u8>>, Error> {
        // FIXME - this function is called synchronously, but it makes several
        //         file system calls. This might be pushing the limits of what we should
        //         be blocking on.

        // Get state
        let fetch_result: Option<FetchState> = self.urls.read().await.get(url).copied();

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
            Some(FetchState::Queued(_)) | Some(FetchState::QueuedStale) => {
                tracing::trace!("FETCH {url}: Already queued.");
                return Ok(None);
            }
            _ => {} // fall through
        }

        // Check if a cached file exists and is fresh enough
        let mut stale = false;

        // Look in both permanent and temporary cache paths
        let mut cache_file = self.cache_file(url, false).await;
        let mut md: Result<fs::Metadata, std::io::Error> = fs::metadata(cache_file.as_path());
        if let Err(e) = md {
            if e.kind() != std::io::ErrorKind::NotFound {
                tracing::info!("FETCH {url}: Failed: {e}");
                return Err(e.into());
            }
            cache_file = self.cache_file(url, true).await;
            md = fs::metadata(cache_file.as_path());
            if let Err(ref e) = md {
                if e.kind() != std::io::ErrorKind::NotFound {
                    tracing::info!("FETCH {url}: Failed: {e}");
                    return Err(md.unwrap_err().into());
                }
            }
        }

        // If we found it somewhere
        if let Ok(metadata) = md {
            // We had a bug that put empty cache files in place (maybe we still have it).
            // In any case, if the file is empty, don't honor it and wipe any etag
            if metadata.len() == 0 {
                let etag_file = GLOBALS.fetcher.etag_file(url).await;
                let _ = fs::remove_file(etag_file);
            } else {
                if let Ok(modified) = metadata.modified() {
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

        // We can't fetch as we are not async and we don't want to block the caller.
        // So we queue this request for now.
        let state = if stale {
            FetchState::QueuedStale
        } else {
            FetchState::Queued(use_temp_cache)
        };
        self.urls.write().await.insert(url.to_owned(), state);

        tracing::debug!("FETCH {url}: Queued");

        Ok(None)
    }

    async fn fetch(&self, url: Url, use_temp_cache: bool) {
        // Do not fetch if offline
        if GLOBALS.db().read_setting_offline() {
            tracing::debug!("FETCH {url}: Failed: offline mode");
            self.urls.write().await.insert(url, FetchState::Failed);
            return;
        }

        let etag_file = GLOBALS.fetcher.etag_file(&url).await;
        let cache_file = if use_temp_cache {
            GLOBALS.fetcher.cache_file(&url, true).await
        } else {
            GLOBALS.fetcher.cache_file(&url, false).await
        };

        let etag: Option<Vec<u8>> = match tokio::fs::read(etag_file.as_path()).await {
            Ok(contents) => {
                // etag is only valid if the contents file is present
                if matches!(tokio::fs::try_exists(cache_file.as_path()).await, Ok(true)) {
                    Some(contents)
                } else {
                    None
                }
            }
            Err(_) => None,
        };

        let stale = matches!(
            self.urls
                .read()
                .await
                .get(&url)
                .unwrap_or(&FetchState::Queued(use_temp_cache)),
            FetchState::QueuedStale
        );

        let host = self.host(&url).unwrap();

        // Mark url as in-flight
        self.urls
            .write()
            .await
            .insert(url.clone(), FetchState::InFlight);

        // Fetch the resource
        // it is an Arc internally
        let client = GLOBALS.fetcher.client.read().await.clone().unwrap();

        let mut req = client.get(url.as_str());
        if let Some(ref etag) = etag {
            req = req.header("if-none-match", etag.to_owned());
        }
        if GLOBALS.db().read_setting_set_user_agent() {
            req = req.header("User-Agent", USER_AGENT);
        };

        enum FailOutcome {
            Fail,
            NotModified,
            Requeue,
        }

        // closure to run when finished (if we didn't succeed)
        let finish = async |outcome, message, err: Option<Error>, sinbin_secs| {
            match outcome {
                FailOutcome::Fail => {
                    if stale {
                        if let Some(e) = err {
                            tracing::warn!(
                                "FETCH {url}: Failed (using stale cache): {message}: {e}"
                            );
                        } else {
                            tracing::warn!("FETCH {url}: Failed (using stale cache): {message}");
                        }
                        // FIXME: bumping the mtime might not be the best way to do this.
                        let _ = filetime::set_file_mtime(
                            cache_file.as_path(),
                            filetime::FileTime::now(),
                        );
                        self.urls.write().await.remove(&url);
                    } else {
                        if let Some(e) = err {
                            tracing::warn!("FETCH {url}: Failed: {message}: {e}");
                        } else {
                            tracing::warn!("FETCH {url}: Failed: {message}");
                        }
                        self.urls
                            .write()
                            .await
                            .insert(url.clone(), FetchState::Failed);
                    }
                }
                FailOutcome::NotModified => {
                    tracing::debug!("FETCH {url}: Succeeded: {message}");
                    let _ =
                        filetime::set_file_mtime(cache_file.as_path(), filetime::FileTime::now());
                    self.urls.write().await.remove(&url);
                }
                FailOutcome::Requeue => {
                    if let Some(e) = err {
                        tracing::info!("FETCH {url}: Re-Queued: {message}: {e}");
                    } else {
                        tracing::info!("FETCH {url}: Re-Queued: {message}");
                    }
                    self.urls
                        .write()
                        .await
                        .insert(url.clone(), FetchState::Queued(use_temp_cache));
                }
            }
            if sinbin_secs > 0 {
                self.sinbin(&url, Duration::from_secs(sinbin_secs));
            }
            self.decrement_host_load(&host);
        };

        let mut read_runstate = GLOBALS.read_runstate.clone();
        read_runstate.mark_unchanged();
        if read_runstate.borrow().going_offline() {
            return;
        }

        let maybe_response: Result<reqwest::Response, reqwest::Error>;
        tokio::select! {
            r = req.send() => maybe_response = r,
            _ = read_runstate.wait_for(|runstate| runstate.going_offline()) => return,
        }

        let low_exclusion = GLOBALS
            .db()
            .read_setting_fetcher_host_exclusion_on_low_error_secs();
        let med_exclusion = GLOBALS
            .db()
            .read_setting_fetcher_host_exclusion_on_med_error_secs();
        let high_exclusion = GLOBALS
            .db()
            .read_setting_fetcher_host_exclusion_on_high_error_secs();

        // Deal with response errors
        let response = match maybe_response {
            Ok(r) => r,
            Err(e) => {
                if e.is_builder() {
                    finish(FailOutcome::Fail, "builder error", Some(e.into()), 0);
                } else if e.is_timeout() {
                    finish(
                        FailOutcome::Requeue,
                        "timeout",
                        Some(e.into()),
                        low_exclusion,
                    );
                } else if e.is_request() {
                    finish(FailOutcome::Fail, "request error", Some(e.into()), 0);
                } else if e.is_connect() {
                    finish(FailOutcome::Fail, "connect error", Some(e.into()), 0);
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
            finish(
                FailOutcome::Requeue,
                "informational error",
                None,
                low_exclusion,
            );
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
            // Give the server time to recover
            finish(FailOutcome::Requeue, "server error", None, high_exclusion);
            return;
        } else if status.is_success() {
            // fall through
        } else {
            match status {
                StatusCode::REQUEST_TIMEOUT => {
                    finish(FailOutcome::Requeue, "request timeout", None, low_exclusion);
                }
                StatusCode::TOO_MANY_REQUESTS => {
                    finish(
                        FailOutcome::Requeue,
                        "too many requests",
                        None,
                        med_exclusion,
                    );
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

        self.urls.write().await.remove(&url);
    }

    async fn cache_file(&self, url: &Url, tmp: bool) -> PathBuf {
        // Hash the url into a SHA256 hex string
        let hash = {
            let mut hasher = sha2::Sha256::new();
            hasher.update(url.as_str().as_bytes());
            let result = hasher.finalize();
            hex::encode(result)
        };

        let mut cache_file = if tmp {
            self.tmp_cache_dir.read().await.clone()
        } else {
            self.cache_dir.read().await.clone()
        };
        cache_file.push(hash);
        cache_file
    }

    async fn sinbin(&self, url: &Url, duration: Duration) {
        let now = Unixtime::now();
        let later = now + duration;
        let host = match self.host(url) {
            Some(h) => h,
            None => return,
        };

        // lock penalty box
        let mut penalty_box = self.penalty_box.write().await;

        if let Some(time) = penalty_box.get_mut(&*host) {
            if *time < later {
                *time = later;
            }
        } else {
            penalty_box.insert(host, later);
        }
    }

    fn host(&self, url: &Url) -> Option<String> {
        let u = match url::Url::parse(url.as_str()) {
            Ok(u) => u,
            Err(_) => return None,
        };
        u.host_str().map(|s| s.to_owned())
    }

    async fn etag_file(&self, url: &Url) -> PathBuf {
        self.cache_file(url, false).await.with_extension("etag")
    }

    async fn fetch_host_load(&self, host: &str) -> usize {
        let hashmap = self.host_load.read().await;
        if let Some(load) = hashmap.get(host) {
            *load
        } else {
            0
        }
    }

    async fn increment_host_load(&self, host: &str) {
        let mut hashmap = self.host_load.write().await;
        if let Some(load) = hashmap.get_mut(host) {
            *load += 1;
        } else {
            hashmap.insert(host.to_string(), 1);
        }
    }

    async fn decrement_host_load(&self, host: &str) {
        let mut hashmap = self.host_load.write().await;
        if let Some(load) = hashmap.get_mut(host) {
            if *load == 1 {
                hashmap.remove(host);
            } else {
                *load -= 1;
            }
        }
    }

    pub(crate) async fn prune(&self, age: Duration) -> Result<usize, Error> {
        let mut count: usize = 0;
        let cache_path = self.cache_dir.read().await.to_owned();
        let mut entries = tokio::fs::read_dir(cache_path.as_path()).await?;
        while let Some(entry) = entries.next_entry().await? {
            if let Ok(metadata) = entry.metadata().await {
                if metadata.is_dir() {
                    continue;
                }

                // FIXME - many filesystems do not track access times. We may want
                //         to track these in LMDB (along with etags)
                let file_time = match metadata.accessed() {
                    Ok(st) => st,
                    Err(_) => match metadata.modified() {
                        Ok(st) => st,
                        Err(_) => metadata.created()?,
                    },
                };
                let file_age = match SystemTime::now().duration_since(file_time) {
                    Ok(dur) => dur,
                    Err(_) => continue,
                };
                if file_age > age {
                    tokio::fs::remove_file(entry.path().as_path()).await?;
                    count += 1;
                }
            }
        }

        Ok(count)
    }
}
