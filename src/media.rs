use crate::globals::GLOBALS;
use dashmap::{DashMap, DashSet};
use eframe::egui::ColorImage;
use egui_extras::image::FitTo;
use nostr_types::{UncheckedUrl, Url};
use std::collections::HashSet;
use std::sync::atomic::Ordering;
use tokio::sync::RwLock;

pub struct Media {
    // We fetch (with Fetcher), process, and temporarily hold media
    // until the UI next asks for them, at which point we remove them
    // and hand them over. This way we can do the work that takes
    // longer and the UI can do as little work as possible.
    image_temp: DashMap<Url, ColorImage>,
    data_temp: DashMap<Url, Vec<u8>>,
    media_pending_processing: DashSet<Url>,
    failed_media: RwLock<HashSet<UncheckedUrl>>,
}

impl Media {
    pub fn new() -> Media {
        Media {
            image_temp: DashMap::new(),
            data_temp: DashMap::new(),
            media_pending_processing: DashSet::new(),
            failed_media: RwLock::new(HashSet::new()),
        }
    }

    pub fn check_url(&self, unchecked_url: UncheckedUrl) -> Option<Url> {
        // Fail permanently if the URL is bad
        let url = match Url::try_from_unchecked_url(&unchecked_url) {
            Ok(url) => url,
            Err(_) => {
                // this cannot recover without new metadata
                self.failed_media.blocking_write().insert(unchecked_url);
                return None;
            }
        };
        Some(url)
    }

    pub fn has_failed(&self, unchecked_url: &UncheckedUrl) -> bool {
        return self.failed_media.blocking_read().contains(unchecked_url);
    }

    pub fn retry_failed(&self, unchecked_url: &UncheckedUrl) {
        self.failed_media.blocking_write().remove(unchecked_url);
    }

    pub fn get_image(&self, url: &Url) -> Option<ColorImage> {
        // If we have it, hand it over (we won't need a copy anymore)
        if let Some(th) = self.image_temp.remove(url) {
            return Some(th.1);
        }

        // If it is pending processing, respond now
        if self.media_pending_processing.contains(url) {
            return None; // will recover after processing completes
        }

        match self.get_data(url) {
            Some(bytes) => {
                // Finish this later (spawn)
                let aurl = url.to_owned();
                tokio::spawn(async move {
                    let size = 800 * 3 // 3x feed size, 1x Media page size
                        * GLOBALS
                            .pixels_per_point_times_100
                            .load(Ordering::Relaxed)
                        / 100;
                    if let Ok(color_image) = egui_extras::image::load_image_bytes(&bytes) {
                        GLOBALS.media.image_temp.insert(aurl, color_image);
                    } else if let Ok(color_image) = egui_extras::image::load_svg_bytes_with_size(
                        &bytes,
                        FitTo::Size(size, size),
                    ) {
                        GLOBALS.media.image_temp.insert(aurl, color_image);
                    } else {
                        // this cannot recover without new metadata
                        GLOBALS
                            .media
                            .failed_media
                            .write()
                            .await
                            .insert(aurl.to_unchecked_url());
                    };
                });
                self.media_pending_processing.insert(url.clone());
                None
            }
            None => None,
        }
    }

    pub fn get_data(&self, url: &Url) -> Option<Vec<u8>> {
        // If it failed before, error out now
        if self
            .failed_media
            .blocking_read()
            .contains(&url.to_unchecked_url())
        {
            return None; // cannot recover.
        }

        // If we have it, hand it over (we won't need a copy anymore)
        if let Some(th) = self.data_temp.remove(url) {
            return Some(th.1);
        }

        // Do not fetch if disabled
        if !GLOBALS.settings.read().load_media {
            return None; // can recover if the setting is switched
        }

        match GLOBALS.fetcher.try_get(url.clone()) {
            Ok(None) => None,
            Ok(Some(bytes)) => {
                self.data_temp.insert(url.clone(), bytes);
                None
            }
            Err(e) => {
                tracing::error!("{}", e);
                // this cannot recover without new metadata
                self.failed_media
                    .blocking_write()
                    .insert(url.to_unchecked_url());
                None
            }
        }
    }
}
