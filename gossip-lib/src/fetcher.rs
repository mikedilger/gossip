use crate::error::Error;
use crate::globals::GLOBALS;
use crate::profile::Profile;
use crate::USER_AGENT;
use dashmap::DashMap;
use nostr_types::{Unixtime, Url};
use reqwest::header::ETAG;
use reqwest::{Client, StatusCode};
use sha2::Digest;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, RwLock};
use std::time::{Duration, SystemTime};
use tokio::sync::Semaphore;

impl Fetcher {
    /// This creates a new fetcher with lazy initialization
    pub(crate) fn new() -> Fetcher {
        Fetcher {
            ..Default::default()
        }
    }

    pub fn stats(&self) -> String {
        let starting = self
            .url_data
            .iter()
            .filter(|rm| rm.value().state == FetchState::Starting)
            .count();

        let checking_cache = self
            .url_data
            .iter()
            .filter(|rm| rm.value().state == FetchState::CheckingCache)
            .count();

        let loading_from_cache = self
            .url_data
            .iter()
            .filter(|rm| rm.value().state == FetchState::LoadingFromCache)
            .count();

        let queued = self
            .url_data
            .iter()
            .filter(|rm| rm.value().state == FetchState::Queued)
            .count();

        let fetching = self
            .url_data
            .iter()
            .filter(|rm| rm.value().state == FetchState::Fetching)
            .count();

        let ready = self
            .url_data
            .iter()
            .filter(|rm| rm.value().state == FetchState::Ready)
            .count();

        let taken = self
            .url_data
            .iter()
            .filter(|rm| rm.value().state == FetchState::Taken)
            .count();

        let failed = self
            .url_data
            .iter()
            .filter(|rm| rm.value().state == FetchState::Failed)
            .count();

        format!(
            "s{}/c{}/l{}/q{}/f{}/r{}/t{}/f{}",
            starting, checking_cache, loading_from_cache, queued, fetching, ready, taken, failed
        )
    }

    /// Statistics about requests that are stalled
    pub fn num_requests_stalled(&self) -> usize {
        self.url_data
            .iter()
            .filter(|rm| rm.value().state.is_stalled())
            .count()
    }

    /// Statistics about requests that are in flight
    pub fn num_requests_in_flight(&self) -> usize {
        self.url_data
            .iter()
            .filter(|rm| rm.value().state.is_in_flight())
            .count()
    }

    /// Statistics about total requests
    pub fn num_requests_completed(&self) -> usize {
        self.url_data
            .iter()
            .filter(|rm| rm.value().state.is_completed())
            .count()
    }

    /// This is where a client attempts to get data synchronously
    pub fn try_get(&self, url: Url, use_cache: bool) -> Result<FetchResult, Error> {
        // Maybe initialize
        if self.client.read().unwrap().is_none() {
            self.init()?;
        }

        // Lock this url record
        let mut refmut = self.url_data.entry(url.clone()).or_insert(UrlData {
            state: FetchState::Starting,
            bytes: None,
            error: None,
            use_cache,
        });

        match refmut.value().state {
            FetchState::Starting => {
                // Note: This state only occurs if we just created this entry newly, it
                //       should not persist after this function call completes
                std::mem::drop(tokio::spawn(Box::pin(async move {
                    GLOBALS.fetcher.process(url).await;

                    // Notify the UI to redraw now that the image loading is complete
                    GLOBALS.notify_ui_redraw.notify_waiters();
                })));

                Ok(FetchResult::Processing(FetchState::Starting))
            }
            FetchState::Ready => {
                let bytes = std::mem::take(&mut refmut.value_mut().bytes);
                refmut.value_mut().state = FetchState::Taken;
                Ok(FetchResult::Ready(bytes.unwrap()))
            }
            FetchState::Taken => {
                // It has already been taken by some code, but some other code also wants it.
                // So start the fetch over (this time probably cached)
                refmut.value_mut().state = FetchState::Starting;
                std::mem::drop(tokio::spawn(Box::pin(async move {
                    GLOBALS.fetcher.process(url).await;

                    // Notify the UI to redraw now that the image loading is complete
                    GLOBALS.notify_ui_redraw.notify_waiters();
                })));

                Ok(FetchResult::Processing(FetchState::Starting))
            }
            FetchState::Failed => {
                if let Some(ref e) = refmut.value().error {
                    Ok(FetchResult::Failed(e.to_string()))
                } else {
                    Ok(FetchResult::Failed("Unknown error".to_string()))
                }
            }
            _ => Ok(FetchResult::Processing(refmut.value().state)),
        }
    }

    /// This is where a client attempts to get data asynchronously
    ///
    /// This should never return FetchResult::Processing
    pub async fn get(&self, url: Url, use_cache: bool) -> Result<FetchResult, Error> {
        // Maybe initialize
        if self.client.read().unwrap().is_none() {
            self.init()?;
        }

        // Create UrlData if missing
        let mut start = !self.url_data.contains_key(&url);
        if start {
            // Create the record
            {
                self.url_data.entry(url.clone()).or_insert(UrlData {
                    state: FetchState::Starting,
                    bytes: None,
                    error: None,
                    use_cache,
                });
            }
        }

        // Run the fetch if we are starting (or starting again after Taken)
        if let Some(mut refmut) = self.url_data.get_mut(&url) {
            if refmut.value().state == FetchState::Taken {
                refmut.value_mut().state = FetchState::Starting;
                start = true;
            }
        }
        if start {
            // Run the fetch
            GLOBALS.fetcher.process(url.clone()).await;

            // Notify the UI to redraw now that the image loading is complete
            GLOBALS.notify_ui_redraw.notify_waiters();
        }

        loop {
            {
                let mut refmut = self.url_data.get_mut(&url).unwrap();
                match refmut.value().state {
                    FetchState::Ready => {
                        let bytes = std::mem::take(&mut refmut.value_mut().bytes);
                        refmut.value_mut().state = FetchState::Taken;
                        return Ok(FetchResult::Ready(bytes.unwrap()));
                    }
                    FetchState::Taken => {
                        tracing::warn!("Fetcher: stolen by another process!");
                        return Ok(FetchResult::Taken);
                    }
                    FetchState::Failed => {
                        if let Some(ref e) = refmut.value().error {
                            return Ok(FetchResult::Failed(e.to_string()));
                        } else {
                            return Ok(FetchResult::Failed("Unknown error".to_string()));
                        }
                    }
                    _ => {}
                }
            }

            // Still processing...
            // sleep a little bit
            // FIXME ideally we would have some flag.await() [not a semaphore]
            tokio::time::sleep(Duration::from_millis(15)).await;
        }
    }

    /// If a resource has failed and you want to retry, clear the failure first
    pub fn clear_for_retry(&self, url: Url) {
        // Must be in Failed state
        if let Some(refmut) = self.url_data.get_mut(&url) {
            if refmut.state != FetchState::Failed {
                return;
            }
        }

        self.url_data.remove(&url);
    }

    /// Prune
    pub async fn prune(&self, age: Duration) -> Result<usize, Error> {
        // Maybe partially initialize
        if self.client.read().unwrap().is_none() {
            *self.cache_dir.write().unwrap() = Profile::cache_dir(false)?;
        }

        let mut count: usize = 0;
        let cache_path = self.cache_dir.read().unwrap().to_owned();
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

/// Information about a URL including it's fetched data or the state the fetch is in
#[derive(Debug, Default)]
pub struct UrlData {
    /// The state of fetching.  See `FetchState`.
    pub state: FetchState,

    /// The bytes if they are available
    ///
    /// SAFETY: if State is Ready, this must be Some
    pub bytes: Option<Vec<u8>>,

    /// Optional error when FetchState::Error
    ///
    /// SAFETY: If State is Failed, this must be Some
    pub error: Option<String>,

    /// Whether or not the data should be cached
    pub use_cache: bool,
}

/// This is the state of processing of a given URL
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum FetchState {
    /// Starting
    #[default]
    Starting,

    /// CheckingCache
    CheckingCache,

    /// Loading from the cache
    LoadingFromCache,

    /// Not in cache, and server not ready so we are queued
    Queued,

    /// Not in cache, fetching from the remote server
    Fetching,

    /// Ready
    Ready,

    /// Taken (by the UI).
    ///
    /// The bytes are now gone.  If for any odd reason the UI forgets it and asks
    /// for this same URL again, we will have to get it from the cache.
    Taken,

    /// Failed to load, with an error
    Failed,
}

impl FetchState {
    pub fn is_stalled(&self) -> bool {
        matches!(*self, FetchState::Queued)
    }

    pub fn is_completed(&self) -> bool {
        matches!(
            *self,
            FetchState::Ready | FetchState::Taken | FetchState::Failed
        )
    }

    pub fn is_in_flight(&self) -> bool {
        matches!(
            *self,
            FetchState::Starting
                | FetchState::CheckingCache
                | FetchState::LoadingFromCache
                | FetchState::Fetching
        )
    }
}

/// This is the result returned when you try to get a URL's bytes
#[derive(Debug)]
pub enum FetchResult {
    /// In Process
    Processing(FetchState),

    /// The data is ready
    Ready(Vec<u8>),

    /// You already took the data!
    Taken,

    /// The fetch failed
    Failed(String),
}

// -----------------------------------------------------------------------------------
//
// Grotty details below this line

/// System that fetches HTTP resources
#[derive(Debug, Default)]
pub struct Fetcher {
    /// per-url data and/or the state of fetching the data
    url_data: DashMap<Url, UrlData>,

    /// HTTP client
    client: RwLock<Option<Client>>,

    /// Persistent filesystem cache of network objects. This is faster than fetching
    /// over the network, but the data still needs to be loaded into memory
    cache_dir: RwLock<PathBuf>,

    /// The load currently applied to a host
    host_load: DashMap<String, Arc<Semaphore>>,

    /// Penalized hosts; upon certain errors we time out the host and try again later
    penalty_box: DashMap<String, Unixtime>,

    // Warned about lack of modification time
    warned_already: AtomicBool,
}

impl Fetcher {
    /// This initializes the fetcher, which is called internally when it is first used
    fn init(&self) -> Result<(), Error> {
        // Do not init() if already initialized
        if self.client.read().unwrap().is_some() {
            return Ok(());
        }

        // Copy profile directory so we don't have to deal with the rare
        // initialization error every time we use them
        *self.cache_dir.write().unwrap() = Profile::cache_dir(false)?;

        // Create client
        let connect_timeout =
            std::time::Duration::new(GLOBALS.db().read_setting_fetcher_connect_timeout_sec(), 0);
        let timeout = std::time::Duration::new(GLOBALS.db().read_setting_fetcher_timeout_sec(), 0);

        *self.client.write().unwrap() = Some(
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

    // NOTE: this is only called from try_get() in one place where we are guaranteed
    // that the `FetchState` is `FetchState::Starting`. So we don't have to check the
    // fetch state.  We are the only process changing it, but we drop the lock often
    // so that other processes can check on our progress.
    async fn process(&self, url: Url) {
        let cache_file = self.cache_file(&url);
        let etag_file = cache_file.with_extension("etag");

        // Do not fetch if offline
        if GLOBALS.db().read_setting_offline() {
            tracing::debug!("FETCH {url}: Failed: offline mode");
            self.failed(&url, "Offline".to_string());
            return;
        }

        // Possibly check the cache
        let use_cache = {
            let mut refmut = self.url_data.get_mut(&url).unwrap();
            let use_cache = refmut.value().use_cache;
            if use_cache {
                refmut.value_mut().state = FetchState::CheckingCache;
            }
            use_cache
        };
        if use_cache {
            let mut cache_is_usable: bool = false;
            if let Ok(md) = tokio::fs::metadata(cache_file.as_path()).await {
                if md.len() == 0 {
                    // Remove the file and it's etag
                    let _ = tokio::fs::remove_file(cache_file.as_path()).await;
                    let _ = tokio::fs::remove_file(etag_file.as_path()).await;
                    // falls through to fetching
                } else {
                    // We do this every time in case settings change on the fly
                    let max_age = Duration::from_secs(
                        60 * 60 * GLOBALS.db().read_setting_media_becomes_stale_hours(),
                    );

                    if let Ok(modified) = md.modified() {
                        match modified.elapsed() {
                            Ok(dur) => {
                                if dur < max_age {
                                    cache_is_usable = true;
                                }
                            }
                            Err(_) => cache_is_usable = true, // date in future
                        }
                    } else {
                        if !self.warned_already.load(Ordering::Relaxed) {
                            tracing::error!("This system does not have file modification times. Our file cache logic depends on it. As a result files will not be cached between gossip runs.");
                            self.warned_already.store(true, Ordering::Relaxed);
                        }
                    }

                    if cache_is_usable {
                        self.set_state(&url, FetchState::LoadingFromCache);
                        match tokio::fs::read(cache_file.as_path()).await {
                            Ok(bytes) => {
                                self.finish(&url, bytes);
                                return;
                            }
                            Err(_) => {
                                // Remove the file and it's etag
                                let _ = tokio::fs::remove_file(cache_file.as_path()).await;
                                let _ = tokio::fs::remove_file(etag_file.as_path()).await;
                                // falls through to fetching
                            }
                        }
                    }
                }
            }
        }

        // Get the ETAG if available
        let mut etag: Option<Vec<u8>> = match tokio::fs::read(etag_file.as_path()).await {
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

        // Get the host
        let Some(host) = self.host(&url) else {
            self.failed(&url, "Invalid Host".to_string());
            return;
        };

        let low_exclusion = GLOBALS
            .db()
            .read_setting_fetcher_host_exclusion_on_low_error_secs();
        let med_exclusion = GLOBALS
            .db()
            .read_setting_fetcher_host_exclusion_on_med_error_secs();
        let high_exclusion = GLOBALS
            .db()
            .read_setting_fetcher_host_exclusion_on_high_error_secs();

        loop {
            // Moved to Queued
            self.set_state(&url, FetchState::Queued);

            let semaphore = self.acquire_host(host.as_str()).await;
            let _permit = semaphore.acquire().await.unwrap();

            // Move to Fetching
            self.set_state(&url, FetchState::Fetching);

            // Get the client
            // (Client is internally an Arc so we can just clone it)
            let client = self.client.read().unwrap().clone().unwrap();

            // Build the request
            let mut req = client.get(url.as_str());
            if let Some(ref etag) = etag {
                req = req.header("if-none-match", etag.to_owned());
            }
            if GLOBALS.db().read_setting_set_user_agent() {
                req = req.header("User-Agent", USER_AGENT);
            };

            // Make sure we exit any of these fetches if we suddenly go offline
            let mut read_runstate = GLOBALS.read_runstate.clone();
            read_runstate.mark_unchanged();
            if read_runstate.borrow().going_offline() {
                tracing::debug!("FETCH {url}: Failed: going offline");
                self.failed(&url, "Going Offline".to_string());
                return;
            }

            let maybe_response: Result<reqwest::Response, reqwest::Error>;
            tokio::select! {
                r = req.send() => maybe_response = r,
                _ = read_runstate.wait_for(|runstate| runstate.going_offline()) => {
                    tracing::debug!("FETCH {url}: Failed: going offline");
                    self.failed(&url, "Going Offline".to_string());
                    return;
                },
            }

            // Deal with response errors
            let response = match maybe_response {
                Ok(r) => r,
                Err(e) => {
                    if e.is_builder() {
                        self.failed(&url, format!("Builder error: {e}"));
                        return;
                    } else if e.is_timeout() {
                        // Sinbin and try again later
                        self.sinbin(&url, Duration::from_secs(med_exclusion));
                        continue;
                    } else if e.is_request() {
                        self.failed(&url, format!("Request error: {e}"));
                        return;
                    } else if e.is_connect() {
                        self.failed(&url, format!("Connect error: {e}"));
                        return;
                    } else if e.is_body() {
                        self.failed(&url, format!("Body error: {e}"));
                        return;
                    } else if e.is_decode() {
                        self.failed(&url, format!("Decode error: {e}"));
                        return;
                    } else {
                        self.failed(&url, format!("Error: {e}"));
                        return;
                    }
                }
            };

            // Deal with status codes
            let status = response.status();
            if status.is_informational() {
                // Sinbin and try again later
                self.sinbin(&url, Duration::from_secs(med_exclusion));
                continue;
            } else if status.is_redirection() {
                if status == StatusCode::NOT_MODIFIED {
                    // Touch the file to freshen it
                    // There is no good async way to do this that I have found.
                    let _ =
                        filetime::set_file_mtime(cache_file.as_path(), filetime::FileTime::now());

                    // Read from the file
                    match tokio::fs::read(cache_file.as_path()).await {
                        Ok(bytes) => {
                            self.finish(&url, bytes);
                            return;
                        }
                        Err(_) => {
                            // Remove the file and it's etag
                            let _ = tokio::fs::remove_file(cache_file.as_path()).await;
                            let _ = tokio::fs::remove_file(etag_file.as_path()).await;
                            etag = None;

                            // Loop around and try again, this time without etag
                            continue;
                        }
                    }
                } else {
                    // Our client follows up to 10 redirects. This is a failure.
                    self.failed(&url, "Redirected more than 10 times.".to_string());
                    return;
                }
            } else if status.is_server_error() {
                // Sinbin the server long-time, and fail this particular one (we aren't
                // going to wait for the default 6 minutes to try again).
                // Instead TODO:  Let the user manually try again with a button click.
                self.sinbin(&url, Duration::from_secs(high_exclusion));
                self.failed(&url, format!("Server failed twice: {}", status));
                return;
            } else if status == StatusCode::REQUEST_TIMEOUT {
                self.sinbin(&url, Duration::from_secs(low_exclusion));
                continue;
            } else if status == StatusCode::TOO_MANY_REQUESTS {
                self.sinbin(&url, Duration::from_secs(med_exclusion));
                continue;
            } else if !status.is_success() {
                self.failed(&url, format!("{}", status));
                return;
            }

            let maybe_etag = response
                .headers()
                .get(ETAG)
                .map(|e| e.as_bytes().to_owned());

            // Convert to bytes
            let maybe_bytes = response.bytes().await;
            let bytes = match maybe_bytes {
                Ok(bytes) => bytes,
                Err(e) => {
                    self.failed(&url, format!("Response bytes: {e}"));
                    return;
                }
            };

            // Do not accept zero-length files, and don't try again
            if bytes.is_empty() {
                self.failed(&url, "Zero length file".to_owned());
                return;
            }

            GLOBALS.bytes_read.fetch_add(bytes.len(), Ordering::Relaxed);

            // Write to the cache file
            // ignore any error in caching
            let _ = tokio::fs::write(cache_file.as_path(), &bytes).await;

            // Make available
            self.finish(&url, bytes.to_vec());

            // If there was an etag, save it
            if let Some(etag) = maybe_etag {
                let _ = tokio::fs::write(etag_file.as_path(), etag).await;
            }

            return;
        }
    }

    fn set_state(&self, url: &Url, state: FetchState) {
        if let Some(mut refmut) = self.url_data.get_mut(url) {
            refmut.value_mut().state = state;
        }
    }

    fn failed(&self, url: &Url, error: String) {
        if let Some(mut refmut) = self.url_data.get_mut(url) {
            refmut.value_mut().error = Some(error);
            refmut.value_mut().state = FetchState::Failed;
        }
    }

    fn finish(&self, url: &Url, bytes: Vec<u8>) {
        tracing::debug!(target: "fetcher", "FETCHER DEBUG: {} is ready", url);
        if let Some(mut refmut) = self.url_data.get_mut(url) {
            refmut.value_mut().bytes = Some(bytes);
            refmut.value_mut().state = FetchState::Ready;
        }
    }

    fn cache_file(&self, url: &Url) -> PathBuf {
        // Hash the url into a SHA256 hex string
        let hash = {
            let mut hasher = sha2::Sha256::new();
            hasher.update(url.as_str().as_bytes());
            let result = hasher.finalize();
            hex::encode(result)
        };

        let mut cache_file = self.cache_dir.read().unwrap().clone();
        cache_file.push(hash);
        cache_file
    }

    fn host(&self, url: &Url) -> Option<String> {
        let u = match url::Url::parse(url.as_str()) {
            Ok(u) => u,
            Err(_) => return None,
        };
        u.host_str().map(|s| s.to_owned())
    }

    async fn acquire_host(&self, host: &str) -> Arc<Semaphore> {
        // Wait for the host to be available if it is in the penalty box
        loop {
            let Some(time) = self.penalty_box.get(host).map(|r| *r.value()) else {
                break;
            };
            let now = Unixtime::now();
            if time < now {
                // Remove from penalty box
                self.penalty_box.remove(host);
                break;
            } else {
                // Wait until the penalty expires
                tokio::time::sleep(now - time).await;
            }
        }

        // Make sure we have an entry for the host in the host_load
        // NOTE: if the setting changes, we don't update existing hosts because
        //       replacing the semaphore is too complex.
        {
            if !self.host_load.contains_key(host) {
                let num_permits = GLOBALS.db().read_setting_fetcher_max_requests_per_host();
                self.host_load
                    .entry(host.to_string())
                    .or_insert(Arc::new(Semaphore::new(num_permits)));
            }
        }

        // get the semaphore
        self.host_load.get(host).unwrap().value().clone()
    }

    fn sinbin(&self, url: &Url, duration: Duration) {
        let now = Unixtime::now();
        let later = now + duration;
        let host = match self.host(url) {
            Some(h) => h,
            None => return,
        };

        if let Some(mut time) = self.penalty_box.get_mut(&*host) {
            if *time < later {
                *time = later;
            }
        } else {
            self.penalty_box.insert(host, later);
        }
    }
}
