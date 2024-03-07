use egui_winit::egui::{ColorImage, Context, TextureHandle, TextureOptions};
use tiny_skia::Transform;
use usvg::TreeParsing;

pub const SVG_OVERSAMPLE: f32 = 2.0;

pub struct Assets {
    pub options_symbol: TextureHandle,
    pub magnifyingglass_symbol: TextureHandle,
}

impl Assets {
    pub fn init(ctx: &Context) -> Self {
        // how to load an svg
        let ppt = ctx.pixels_per_point();
        let dpi = ppt * 72.0;
        let options_symbol = {
            let bytes = include_bytes!("../../../assets/option.svg");
            let opt = usvg::Options {
                dpi,
                ..Default::default()
            };
            let rtree = usvg::Tree::from_data(bytes, &opt).unwrap();
            let [w, h] = [
                (rtree.size.width() * ppt * SVG_OVERSAMPLE) as u32,
                (rtree.size.height() * ppt * SVG_OVERSAMPLE) as u32,
            ];
            let mut pixmap = tiny_skia::Pixmap::new(w, h).unwrap();
            let tree = resvg::Tree::from_usvg(&rtree);
            tree.render(
                Transform::from_scale(ppt * SVG_OVERSAMPLE, ppt * SVG_OVERSAMPLE),
                &mut pixmap.as_mut(),
            );
            let color_image = ColorImage::from_rgba_unmultiplied([w as _, h as _], pixmap.data());
            ctx.load_texture("options_symbol", color_image, TextureOptions::LINEAR)
        };

        let magnifyingglass_symbol = {
            let bytes = include_bytes!("../../../assets/magnifyingglass.svg");
            let opt = usvg::Options {
                dpi,
                ..Default::default()
            };
            let rtree = usvg::Tree::from_data(bytes, &opt).unwrap();
            let [w, h] = [
                (rtree.size.width() * ppt * SVG_OVERSAMPLE) as u32,
                (rtree.size.height() * ppt * SVG_OVERSAMPLE) as u32,
            ];
            let mut pixmap = tiny_skia::Pixmap::new(w, h).unwrap();
            let tree = resvg::Tree::from_usvg(&rtree);
            tree.render(
                Transform::from_scale(ppt * SVG_OVERSAMPLE, ppt * SVG_OVERSAMPLE),
                &mut pixmap.as_mut(),
            );
            let color_image = ColorImage::from_rgba_unmultiplied([w as _, h as _], pixmap.data());
            ctx.load_texture(
                "magnifyingglass_symbol",
                color_image,
                TextureOptions::LINEAR,
            )
        };

        Self {
            magnifyingglass_symbol,
            options_symbol,
        }
    }
}
