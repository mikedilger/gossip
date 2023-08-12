use crate::globals::GLOBALS;
use dashmap::{DashMap, DashSet};
use eframe::egui::{Color32, ColorImage};
use egui_extras::image::FitTo;
use image::imageops;
use image::imageops::FilterType;
use image::DynamicImage;
use nostr_types::{UncheckedUrl, Url};
use std::collections::HashSet;
use std::sync::atomic::Ordering;
use std::time::Duration;
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

                    match load_image_bytes(
                        &bytes, false, // don't crop square
                        size,  // default size,
                        false, // don't force that size
                        false, // don't round
                    ) {
                        Ok(color_image) => {
                            GLOBALS.media.image_temp.insert(aurl, color_image);
                        }
                        Err(_) => {
                            GLOBALS
                                .media
                                .failed_media
                                .write()
                                .await
                                .insert(aurl.to_unchecked_url());
                        }
                    }
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

        match GLOBALS
            .fetcher
            .try_get(url, Duration::from_secs(60 * 60 * 24 * 3))
        {
            // cache expires in 3 days
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

// Note: size is required for SVG which has no inherent size, even if we don't resize
pub(crate) fn load_image_bytes(
    image_bytes: &[u8],
    square: bool,
    default_size: u32,
    force_resize: bool,
    round: bool,
) -> Result<ColorImage, String> {
    let mut color_image = match image::load_from_memory(image_bytes) {
        Ok(mut image) => {
            image = adjust_orientation(image_bytes, image);
            if square {
                image = crop_square(image);
            }
            if force_resize || image.width() > 16384 || image.height() > 16384 {
                // This preserves aspect ratio. The sizes represent bounds.
                image = image.resize(default_size, default_size, FilterType::Triangle);
            }
            let current_size = [image.width() as _, image.height() as _];
            let image_buffer = image.into_rgba8();
            let pixels = image_buffer.as_flat_samples();
            ColorImage::from_rgba_unmultiplied(current_size, pixels.as_slice())
        }
        Err(_) => {
            // With SVG, we set the size so no resize.
            // And there is no exif orientation data.
            egui_extras::image::load_svg_bytes_with_size(
                image_bytes,
                FitTo::Size(default_size, default_size),
            )?
        }
    };

    if round {
        round_image(&mut color_image);
    }

    Ok(color_image)
}

fn adjust_orientation(image_bytes: &[u8], image: DynamicImage) -> DynamicImage {
    match get_orientation(image_bytes) {
        1 => image,
        2 => DynamicImage::ImageRgba8(imageops::flip_horizontal(&image)),
        3 => DynamicImage::ImageRgba8(imageops::rotate180(&image)),
        4 => DynamicImage::ImageRgba8(imageops::flip_horizontal(&image)),
        5 => {
            let image = DynamicImage::ImageRgba8(imageops::rotate90(&image));
            DynamicImage::ImageRgba8(imageops::flip_horizontal(&image))
        }
        6 => DynamicImage::ImageRgba8(imageops::rotate90(&image)),
        7 => {
            let image = DynamicImage::ImageRgba8(imageops::rotate270(&image));
            DynamicImage::ImageRgba8(imageops::flip_horizontal(&image))
        }
        8 => DynamicImage::ImageRgba8(imageops::rotate270(&image)),
        _ => image,
    }
}

fn get_orientation(image_bytes: &[u8]) -> u32 {
    let mut cursor = std::io::Cursor::new(image_bytes);
    let exifreader = exif::Reader::new();
    let exif = match exifreader.read_from_container(&mut cursor) {
        Ok(exif) => exif,
        Err(_) => return 1,
    };
    if let Some(field) = exif.get_field(exif::Tag::Orientation, exif::In::PRIMARY) {
        if let Some(orientation) = field.value.get_uint(0) {
            return orientation;
        }
    }
    1
}

fn crop_square(image: DynamicImage) -> DynamicImage {
    let smaller = image.width().min(image.height());
    if image.width() > smaller {
        let excess = image.width() - smaller;
        image.crop_imm(excess / 2, 0, image.width() - excess, image.height())
    } else if image.height() > smaller {
        let excess = image.height() - smaller;
        image.crop_imm(0, excess / 2, image.width(), image.height() - excess)
    } else {
        image
    }
}

fn round_image(image: &mut ColorImage) {
    // The radius to the edge of of the avatar circle
    let edge_radius = image.size[0] as f32 / 2.0;
    let edge_radius_squared = edge_radius * edge_radius;

    for (pixnum, pixel) in image.pixels.iter_mut().enumerate() {
        // y coordinate
        let uy = pixnum / image.size[0];
        let y = uy as f32;
        let y_offset = edge_radius - y;

        // x coordinate
        let ux = pixnum % image.size[0];
        let x = ux as f32;
        let x_offset = edge_radius - x;

        // The radius to this pixel (may be inside or outside the circle)
        let pixel_radius_squared: f32 = x_offset * x_offset + y_offset * y_offset;

        // If inside of the avatar circle
        if pixel_radius_squared <= edge_radius_squared {
            // squareroot to find how many pixels we are from the edge
            let pixel_radius: f32 = pixel_radius_squared.sqrt();
            let distance = edge_radius - pixel_radius;

            // If we are within 1 pixel of the edge, we should fade, to
            // antialias the edge of the circle. 1 pixel from the edge should
            // be 100% of the original color, and right on the edge should be
            // 0% of the original color.
            if distance <= 1.0 {
                *pixel = Color32::from_rgba_premultiplied(
                    (pixel.r() as f32 * distance) as u8,
                    (pixel.g() as f32 * distance) as u8,
                    (pixel.b() as f32 * distance) as u8,
                    (pixel.a() as f32 * distance) as u8,
                );
            }
        } else {
            // Outside of the avatar circle
            *pixel = Color32::TRANSPARENT;
        }
    }
}
