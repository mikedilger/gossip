use crate::error::{Error, ErrorKind};
use crate::fetcher::FetchResult;
use crate::globals::GLOBALS;
use dashmap::{DashMap, DashSet};
use image::imageops;
use image::imageops::FilterType;
use image::{DynamicImage, Rgba, RgbaImage};
use nostr_types::{FileMetadata, UncheckedUrl, Url};
use std::fmt;
use std::sync::atomic::Ordering;

pub enum MediaLoadingResult<T> {
    Disabled,
    Loading,
    Ready(T),
    Failed(String),
}

impl<T> fmt::Display for MediaLoadingResult<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            MediaLoadingResult::Disabled => write!(f, "Media loading is disabled"),
            MediaLoadingResult::Loading => write!(f, "Loading..."),
            MediaLoadingResult::Ready(_) => write!(f, "Ready"),
            MediaLoadingResult::Failed(ref s) => write!(f, "{s}"),
        }
    }
}

/// System that processes media fetched from the internet
pub struct Media {
    // We fetch (with Fetcher), process, and temporarily hold media
    // until the UI next asks for them, at which point we remove them
    // and hand them over. This way we can do the work that takes
    // longer and the UI can do as little work as possible.
    image_temp: DashMap<Url, RgbaImage>,
    media_pending_processing: DashSet<Url>,
    failed_media: DashMap<UncheckedUrl, String>,
}

impl Default for Media {
    fn default() -> Self {
        Self::new()
    }
}

impl Media {
    pub(crate) fn new() -> Media {
        Media {
            image_temp: DashMap::new(),
            media_pending_processing: DashSet::new(),
            failed_media: DashMap::new(),
        }
    }

    /// Check if a Url is a valid HTTP Url
    pub fn check_url(&self, unchecked_url: UncheckedUrl) -> Option<Url> {
        // Fail permanently if the URL is bad
        let url = match Url::try_from_unchecked_url(&unchecked_url) {
            Ok(url) => url,
            Err(e) => {
                // this cannot recover without new metadata
                let error = format!("{e}");
                self.set_has_failed(&unchecked_url, error);
                return None;
            }
        };
        Some(url)
    }

    /// Check if a Url has failed
    pub fn has_failed(&self, unchecked_url: &UncheckedUrl) -> Option<String> {
        self.failed_media.get(unchecked_url).map(|r| r.to_owned())
    }

    /// Set that a Url has failed
    pub fn set_has_failed(&self, unchecked_url: &UncheckedUrl, failure: String) {
        self.failed_media.insert(unchecked_url.to_owned(), failure);
    }

    /// Retry a failed Url
    pub fn retry_failed(&self, unchecked_url: &UncheckedUrl) {
        self.failed_media.remove(unchecked_url);
    }

    /// Get an image by Url
    ///
    /// This returns immediately, usually with None if never called on that Url before.
    /// Call it again later to try to pick up the result.
    ///
    /// FIXME: this API doesn't serve async clients well.
    pub fn get_image(
        &self,
        url: &Url,
        volatile: bool,
        file_metadata: Option<&FileMetadata>,
    ) -> MediaLoadingResult<RgbaImage> {
        // If we have it, hand it over (we won't need a copy anymore)
        if let Some(th) = self.image_temp.remove(url) {
            return MediaLoadingResult::Ready(th.1);
        }

        // If it is pending processing, don't get it again
        if self.media_pending_processing.contains(url) {
            return MediaLoadingResult::Loading;
        }

        match self.get_data(url, volatile, file_metadata) {
            MediaLoadingResult::Disabled => MediaLoadingResult::Disabled,
            MediaLoadingResult::Loading => MediaLoadingResult::Loading,
            MediaLoadingResult::Ready(bytes) => {
                // If it is already pending processing, respond now
                if self.media_pending_processing.contains(url) {
                    return MediaLoadingResult::Loading;
                } else {
                    self.media_pending_processing.insert(url.clone());
                }

                // Finish this later (spawn)
                let aurl = url.to_owned();
                tokio::spawn(async move {
                    let start = std::time::Instant::now();
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
                        Err(e) => {
                            let error = format!("{e}");
                            GLOBALS
                                .media
                                .set_has_failed(&aurl.to_unchecked_url(), error);
                        }
                    }
                    let end = std::time::Instant::now();
                    tracing::debug!(target: "fetcher", "Media processing took {}ms", (end - start).as_millis());
                });
                MediaLoadingResult::Loading
            }
            MediaLoadingResult::Failed(s) => MediaLoadingResult::Failed(s),
        }
    }

    /// Get data by Url
    ///
    /// This returns immediately, usually with None if never called on that Url before.
    /// Call it again later to try to pick up the result.
    ///
    /// FIXME: this API doesn't serve async clients well.
    ///
    /// DO NOT CALL FROM LIB, ONLY FROM UI
    pub fn get_data(
        &self,
        url: &Url,
        volatile: bool,
        file_metadata: Option<&FileMetadata>,
    ) -> MediaLoadingResult<Vec<u8>> {
        // If it failed before, error out now
        if let Some(s) = self.failed_media.get(&url.to_unchecked_url()) {
            return MediaLoadingResult::Failed(s.to_string());
        }

        // Do not fetch if disabled
        if !GLOBALS.db().read_setting_load_media() {
            return MediaLoadingResult::Disabled;
        }

        let use_cache = !volatile;
        match GLOBALS.fetcher.try_get(url.clone(), use_cache) {
            Ok(FetchResult::Processing(_)) => MediaLoadingResult::Loading,
            Ok(FetchResult::Ready(bytes)) => {
                // Verify metadata hash
                if let Some(file_metadata) = &file_metadata {
                    if let Some(x) = &file_metadata.x {
                        use sha2::{Digest, Sha256};
                        let mut hasher = Sha256::new();
                        hasher.update(&bytes);
                        let sha256hash = hasher.finalize();
                        let hash_str = hex::encode(sha256hash);
                        if hash_str != *x {
                            let error = "Hash Mismatch".to_string();
                            self.set_has_failed(&url.to_unchecked_url(), error.clone());
                            return MediaLoadingResult::Failed(error);
                        }
                    }
                }
                MediaLoadingResult::Ready(bytes)
            }
            Ok(FetchResult::Taken) => {
                let error = "Already taken (internal bug, media)".to_string();
                tracing::error!("{}", error);
                self.set_has_failed(&url.to_unchecked_url(), error.clone());
                MediaLoadingResult::Failed(error)
            }
            Ok(FetchResult::Failed(error)) => {
                tracing::error!("{}", error);
                self.set_has_failed(&url.to_unchecked_url(), error.clone());
                MediaLoadingResult::Failed(error)
            }
            Err(e) => {
                let error = format!("{e}");
                tracing::error!("{}", error);
                // this cannot recover without new metadata
                self.set_has_failed(&url.to_unchecked_url(), error.clone());
                MediaLoadingResult::Failed(error)
            }
        }
    }
}

// Note: size is required for SVG which has no inherent size, even if we don't resize
pub(crate) fn load_image_bytes(
    image_bytes: &[u8],
    square: bool,
    mut default_size: u32,
    force_resize: bool,
    round: bool,
) -> Result<RgbaImage, Error> {
    let max_image_side = GLOBALS.max_image_side.load(Ordering::Relaxed) as u32;
    if default_size > max_image_side {
        default_size = max_image_side;
    }
    if let Ok(mut image) = image::load_from_memory(image_bytes) {
        image = adjust_orientation(image_bytes, image);
        if square {
            image = crop_square(image);
        }
        if force_resize || image.width() > max_image_side || image.height() > max_image_side {
            // https://docs.rs/image/latest/image/imageops/enum.FilterType.html
            let algo = match &*GLOBALS.db().read_setting_image_resize_algorithm() {
                "Nearest" => FilterType::Nearest,
                "Triangle" => FilterType::Triangle,
                "CatmullRom" => FilterType::CatmullRom,
                "Gaussian" => FilterType::Gaussian,
                "Lanczos3" => FilterType::Lanczos3,
                _ => FilterType::Triangle,
            };

            // This preserves aspect ratio. The sizes represent bounds.
            image = image.resize(default_size, default_size, algo);
        }
        let mut image = image.into_rgba8();
        if round {
            round_image(&mut image);
        }
        Ok(image)
    } else {
        let opt = usvg::Options::default();
        let rtree = usvg::Tree::from_data(image_bytes, &opt)?;
        let pixmap_size = rtree.size().to_int_size();
        let [w, h] = if force_resize
            || pixmap_size.width() > max_image_side
            || pixmap_size.height() > max_image_side
        {
            [default_size, default_size]
        } else {
            [pixmap_size.width(), pixmap_size.height()]
        };
        let mut pixmap = tiny_skia::Pixmap::new(w, h)
            .ok_or::<Error>(ErrorKind::General("Invalid image size".to_owned()).into())?;
        resvg::render(&rtree, Default::default(), &mut pixmap.as_mut());
        let image = RgbaImage::from_raw(w, h, pixmap.take())
            .ok_or::<Error>(ErrorKind::ImageFailure.into())?;
        Ok(image)
    }
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

fn round_image(image: &mut RgbaImage) {
    // The radius to the edge of of the avatar circle
    let edge_radius = image.width() as f32 / 2.0;
    let edge_radius_squared = edge_radius * edge_radius;

    let w = image.width();
    for (pixnum, pixel) in image.pixels_mut().enumerate() {
        // y coordinate
        let uy = pixnum as u32 / w;
        let y = uy as f32;
        let y_offset = edge_radius - y;

        // x coordinate
        let ux = pixnum as u32 % w;
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
                *pixel = Rgba([
                    (pixel[0] as f32 * distance) as u8,
                    (pixel[1] as f32 * distance) as u8,
                    (pixel[2] as f32 * distance) as u8,
                    (pixel[3] as f32 * distance) as u8,
                ]);
            }
        } else {
            // Outside of the avatar circle
            *pixel = Rgba([0, 0, 0, 0]);
        }
    }
}

pub fn media_url_mimetype(s: &str) -> Option<&'static str> {
    let lower = s.to_lowercase();
    if lower.ends_with(".jpg") || lower.ends_with(".jpeg") {
        Some("image/jpeg")
    } else if lower.ends_with(".png") {
        Some("image/png")
    } else if lower.ends_with(".gif") {
        Some("image/gif")
    } else if lower.ends_with(".webp") {
        Some("image/webp")
    } else if lower.ends_with(".mov") {
        Some("video/quicktime")
    } else if lower.ends_with(".mp4") {
        Some("video/mp4")
    } else if lower.ends_with(".webm") {
        Some("video/webm")
    } else if lower.ends_with(".mkv") {
        Some("video/x-matroska")
    } else if lower.ends_with(".avi") {
        Some("video/x-msvideo")
    } else if lower.ends_with(".wmv") {
        Some("video/x-ms-wmv")
    } else if lower.ends_with(".3gp") {
        Some("video/3gpp")
    } else {
        None
    }
}
