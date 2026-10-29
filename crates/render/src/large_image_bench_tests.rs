//! Canvas timings with two 16 000 × 10 000 images, the second over a quarter of the first (#1483). Run in release:
//! `cargo test --release -p figcraft-render --lib large_image_canvas_timings -- --ignored --nocapture`.
//!
//! Each row renders a 2400 × 1600 view through `Retained`, as the canvas does, while one image is
//! dragged (an edit) or the view pans, and gives the first frame, the mean of the next ones and the
//! bytes the decoded-image cache holds after them.

use std::sync::Arc;
use std::time::Instant;

use figcraft_color::{ImageScaleMode, Paint};
use figcraft_doc::{Document, ImageData, NodeId, NodeKind};
use figcraft_geom::Affine;

use crate::{Options, Retained, View};

const W: u32 = 16_000;
const H: u32 = 10_000;
const FRAMES: u32 = 6;

/// A `W` × `H` RGB PNG of coloured bands, quick to write.
fn png(seed: u8) -> Vec<u8> {
    let img = image::RgbImage::from_fn(W, H, |x, y| image::Rgb([(x / 400) as u8 ^ seed, (y / 400) as u8, seed.wrapping_mul(7)]));
    let mut out = Vec::new();
    let enc = image::codecs::png::PngEncoder::new_with_quality(
        &mut out,
        image::codecs::png::CompressionType::Fast,
        image::codecs::png::FilterType::NoFilter,
    );
    img.write_with_encoder(enc).unwrap();
    out
}

/// Both images at their own size, the second moved by half its size: (document, page, its layer).
/// Moving it redraws both.
fn scene() -> (Document, NodeId, NodeId) {
    let mut d = Document::new("large images");
    let page = d.first_page().unwrap();
    let mut last = page;
    for (i, seed) in [(0u32, 11u8), (1, 99)] {
        let key = format!("bench-{seed}");
        d.images.insert(key.clone(), ImageData { bytes: Arc::new(png(seed)), width: W, height: H });
        last = d.create(page, NodeKind::Rectangle, "Image", f64::from(i * W / 2), f64::from(i * H / 2), f64::from(W), f64::from(H)).unwrap();
        d.get_mut(last).unwrap().fills = vec![Paint::image(&key, ImageScaleMode::Fill)];
    }
    (d, page, last)
}

/// First frame and mean of the next [`FRAMES`] (ms): an image dragged when `drag`, else a pan.
fn time(d: &mut Arc<Document>, page: NodeId, moved: NodeId, view: &View, drag: bool) -> (f64, f64) {
    let opts = Options { lod: true, magnified_image_pixels: true, ..Default::default() };
    let mut retained = Retained::default();
    let mut frame = |d: &Arc<Document>, v: &View| {
        let t = Instant::now();
        retained.render(d, page, v, &opts).unwrap();
        t.elapsed().as_secs_f64() * 1000.0
    };
    let first = frame(d, view);
    let mut total = 0.0;
    for i in 1..=FRAMES {
        let step = f64::from(i) * 40.0 / view.scale;
        let v = if drag {
            Arc::make_mut(d).get_mut(moved).unwrap().transform = Affine::translate((f64::from(W / 2) + step, f64::from(H / 2) + step));
            view.clone()
        } else {
            View { origin: (view.origin.0 + step, view.origin.1), ..view.clone() }
        };
        total += frame(d, &v);
    }
    (first, total / f64::from(FRAMES))
}

#[test]
#[ignore = "timings: run in release with --ignored --nocapture"]
fn large_image_canvas_timings() {
    let (d, page, moved) = scene();
    let mut d = Arc::new(d);
    // Both whole (fit, 10%), zoomed so each shows 4800 px long, and at 100%, all three around the
    // second image's top left corner.
    let fit = View { origin: (0.0, 0.0), scale: 0.1, width: 2400, height: 1600 };
    let near = View { origin: (f64::from(W / 2) - 4000.0, f64::from(H / 2) - 2600.0), scale: 0.3, width: 2400, height: 1600 };
    let close = View { origin: (f64::from(W / 2) - 1200.0, f64::from(H / 2) - 800.0), scale: 1.0, width: 2400, height: 1600 };
    for (name, view) in [("fit", fit), ("30%", near), ("100%", close)] {
        for drag in [true, false] {
            let (first, mean) = time(&mut d, page, moved, &view, drag);
            let what = if drag { "drag" } else { "pan" };
            eprintln!("{name:>5} {what:<4}: first {first:8.1} ms, then {mean:8.1} ms a frame; cache {} MB", crate::images::cached_bytes() >> 20);
        }
    }
}
