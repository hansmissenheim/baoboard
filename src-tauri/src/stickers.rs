//! The sticker library: originals live in one folder as `<id>.<ext>`, and
//! copies scaled to the paste size are rendered into a cache on demand.

use std::{
    error::Error,
    fs,
    io::Cursor,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use image::{
    AnimationDecoder, Frame, ImageFormat, ImageReader, RgbaImage,
    codecs::{
        gif::{GifDecoder, GifEncoder, Repeat},
        png::PngDecoder,
        webp::WebPDecoder,
    },
    imageops::{self, FilterType},
};

pub type Result<T> = std::result::Result<T, Box<dyn Error + Send + Sync>>;

const EXTENSIONS: [&str; 5] = ["png", "jpg", "gif", "webp", "bmp"];

#[derive(serde::Serialize)]
pub struct Sticker {
    pub id: String,
    pub path: PathBuf,
}

/// All stickers, newest first.
pub fn list(dir: &Path) -> Result<Vec<Sticker>> {
    let mut stickers: Vec<Sticker> = fs::read_dir(dir)?
        .filter_map(|entry| entry.ok().map(|e| e.path()))
        .filter(|path| {
            path.extension()
                .and_then(|e| e.to_str())
                .is_some_and(|e| EXTENSIONS.contains(&e))
        })
        .filter_map(|path| {
            let id = path.file_stem()?.to_str()?.to_owned();
            Some(Sticker { id, path })
        })
        .collect();
    // Ids are 13-digit millisecond timestamps, so string order is age order.
    stickers.sort_by(|a, b| b.id.cmp(&a.id));
    Ok(stickers)
}

/// Looks an id up among the stored files, so a caller-supplied id can never
/// point outside `dir`.
pub fn find(dir: &Path, id: &str) -> Result<PathBuf> {
    list(dir)?
        .into_iter()
        .find(|s| s.id == id)
        .map(|s| s.path)
        .ok_or_else(|| format!("no sticker {id}").into())
}

pub fn save(dir: &Path, bytes: &[u8]) -> Result<PathBuf> {
    let format = image::guess_format(bytes)?;
    let ext = format.extensions_str()[0];
    if !EXTENSIONS.contains(&ext) {
        return Err(format!("{format:?} images are not supported").into());
    }
    // Reject truncated or mislabeled files now rather than at paste time.
    ImageReader::with_format(Cursor::new(bytes), format).into_dimensions()?;
    let path = dir.join(format!("{}.{ext}", next_id()));
    fs::write(&path, bytes)?;
    Ok(path)
}

pub fn delete(dir: &Path, cache: &Path, id: &str) -> Result<()> {
    fs::remove_file(find(dir, id)?)?;
    for size in fs::read_dir(cache).into_iter().flatten().flatten() {
        let _ = fs::remove_dir_all(size.path().join(id));
    }
    Ok(())
}

fn next_id() -> u64 {
    static LAST: AtomicU64 = AtomicU64::new(0);
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64;
    let last = LAST
        .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |last| {
            Some(now.max(last + 1))
        })
        .unwrap();
    now.max(last + 1)
}

/// Returns a copy of `src` whose longest side is at most `size`: a GIF when
/// the source is animated, otherwise a PNG. Smaller images are never upscaled.
pub fn render(src: &Path, cache: &Path, size: u32) -> Result<PathBuf> {
    let id = src.file_stem().ok_or("sticker has no name")?;
    let dir = cache.join(size.to_string()).join(id);
    // Check the cache before decoding: decoding every frame is the slow part.
    for ext in ["gif", "png"] {
        let out = dir.join(format!("sticker.{ext}"));
        if out.exists() {
            return Ok(out);
        }
    }

    let bytes = fs::read(src)?;
    let format = image::guess_format(&bytes)?;
    let frames = animation(&bytes, format)?;
    let ext = if frames.is_some() { "gif" } else { "png" };
    let out = dir.join(format!("sticker.{ext}"));
    fs::create_dir_all(&dir)?;
    // Write under a unique temp name: a background render and a paste can
    // render the same sticker at once, and a crash never leaves a half file.
    let tmp = dir.join(format!("{}.tmp", next_id()));

    match frames {
        Some(frames) => {
            let (w, h) = frames[0].buffer().dimensions();
            let (fw, fh) = fit(w, h, size);
            if format == ImageFormat::Gif && (fw, fh) == (w, h) {
                fs::write(&tmp, &bytes)?;
            } else {
                let mut encoder = GifEncoder::new_with_speed(fs::File::create(&tmp)?, 10);
                encoder.set_repeat(Repeat::Infinite)?;
                encoder.encode_frames(frames.into_iter().map(|frame| {
                    let mut buf = resize(frame.buffer(), fw, fh);
                    // GIF has 1-bit transparency, and the encoder treats any
                    // alpha above 0 as opaque, so cut at half instead.
                    for p in buf.pixels_mut() {
                        p[3] = if p[3] < 128 { 0 } else { 255 };
                    }
                    Frame::from_parts(buf, 0, 0, frame.delay())
                }))?;
            }
        }
        None => {
            let img = image::load_from_memory_with_format(&bytes, format)?.into_rgba8();
            let (w, h) = img.dimensions();
            let (fw, fh) = fit(w, h, size);
            if format == ImageFormat::Png && (fw, fh) == (w, h) {
                fs::write(&tmp, &bytes)?;
            } else {
                resize(&img, fw, fh).save_with_format(&tmp, ImageFormat::Png)?;
            }
        }
    }
    fs::rename(&tmp, &out)?;
    Ok(out)
}

/// The frames of an animated image, or `None` for a still one.
fn animation(bytes: &[u8], format: ImageFormat) -> Result<Option<Vec<Frame>>> {
    let reader = Cursor::new(bytes);
    let frames = match format {
        ImageFormat::Gif => GifDecoder::new(reader)?.into_frames().collect_frames()?,
        ImageFormat::Png => {
            let decoder = PngDecoder::new(reader)?;
            if !decoder.is_apng()? {
                return Ok(None);
            }
            decoder.apng()?.into_frames().collect_frames()?
        }
        ImageFormat::WebP => {
            let decoder = WebPDecoder::new(reader)?;
            if !decoder.has_animation() {
                return Ok(None);
            }
            decoder.into_frames().collect_frames()?
        }
        _ => return Ok(None),
    };
    Ok((frames.len() > 1).then_some(frames))
}

fn fit(w: u32, h: u32, size: u32) -> (u32, u32) {
    let longest = w.max(h);
    if longest <= size {
        return (w, h);
    }
    let scale = |side: u32| {
        ((side as u64 * size as u64 + longest as u64 / 2) / longest as u64).max(1) as u32
    };
    (scale(w), scale(h))
}

/// Resizes with premultiplied alpha, so transparent pixels (often stored as
/// black) don't bleed a dark fringe into the edges.
fn resize(img: &RgbaImage, w: u32, h: u32) -> RgbaImage {
    if img.dimensions() == (w, h) {
        return img.clone();
    }
    let mut premultiplied = img.clone();
    for p in premultiplied.pixels_mut() {
        for c in 0..3 {
            p[c] = (p[c] as u16 * p[3] as u16 / 255) as u8;
        }
    }
    let mut out = imageops::resize(&premultiplied, w, h, FilterType::CatmullRom);
    for p in out.pixels_mut() {
        if p[3] > 0 {
            for c in 0..3 {
                p[c] = (p[c] as u16 * 255 / p[3] as u16).min(255) as u8;
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{Delay, Rgba};

    fn tempdir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("baoboard-{name}-{}", next_id()));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn png(w: u32, h: u32) -> Vec<u8> {
        let mut bytes = Vec::new();
        RgbaImage::from_pixel(w, h, Rgba([255, 0, 0, 255]))
            .write_to(&mut Cursor::new(&mut bytes), ImageFormat::Png)
            .unwrap();
        bytes
    }

    fn gif(w: u32, h: u32, frames: u32) -> Vec<u8> {
        let mut bytes = Vec::new();
        let mut encoder = GifEncoder::new(&mut bytes);
        encoder
            .encode_frames((0..frames).map(|i| {
                let px = Rgba([(i * 60) as u8, 0, 0, 255]);
                let delay = Delay::from_numer_denom_ms(100, 1);
                Frame::from_parts(RgbaImage::from_pixel(w, h, px), 0, 0, delay)
            }))
            .unwrap();
        drop(encoder);
        bytes
    }

    #[test]
    fn fit_scales_longest_side_and_never_upscales() {
        assert_eq!(fit(480, 240, 240), (240, 120));
        assert_eq!(fit(100, 1000, 240), (24, 240));
        assert_eq!(fit(5000, 1, 240), (240, 1));
        assert_eq!(fit(64, 32, 240), (64, 32));
    }

    #[test]
    fn resize_keeps_transparent_edges_from_going_dark() {
        let mut img = RgbaImage::from_pixel(4, 4, Rgba([0, 0, 0, 0]));
        for x in 0..2 {
            for y in 0..4 {
                img.put_pixel(x, y, Rgba([255, 255, 255, 255]));
            }
        }
        let out = resize(&img, 2, 2);
        for p in out.pixels().filter(|p| p[3] > 0) {
            assert!(p[0] > 200, "dark fringe: {p:?}");
        }
    }

    #[test]
    fn save_list_find_delete() {
        let (dir, cache) = (tempdir("lib"), tempdir("cache"));
        let first = save(&dir, &png(8, 8)).unwrap();
        let second = save(&dir, &gif(8, 8, 2)).unwrap();
        assert_eq!(second.extension().unwrap(), "gif");
        assert!(save(&dir, b"not an image").is_err());

        let ids: Vec<_> = list(&dir).unwrap().into_iter().map(|s| s.path).collect();
        assert_eq!(ids, [second.clone(), first.clone()]);

        let id = first.file_stem().unwrap().to_str().unwrap();
        assert_eq!(find(&dir, id).unwrap(), first);
        assert!(find(&dir, "../../etc/passwd").is_err());

        render(&first, &cache, 4).unwrap();
        delete(&dir, &cache, id).unwrap();
        assert!(!first.exists());
        assert!(!cache.join("4").join(id).exists());
    }

    #[test]
    fn render_outputs_png_for_stills_and_gif_for_animations() {
        let (dir, cache) = (tempdir("lib"), tempdir("cache"));

        let still = render(&save(&dir, &png(480, 240)).unwrap(), &cache, 240).unwrap();
        assert_eq!(image::image_dimensions(&still).unwrap(), (240, 120));
        assert_eq!(still.extension().unwrap(), "png");

        let src = save(&dir, &gif(480, 480, 3)).unwrap();
        let anim = render(&src, &cache, 240).unwrap();
        assert_eq!(anim.extension().unwrap(), "gif");
        let decoder = GifDecoder::new(Cursor::new(fs::read(&anim).unwrap())).unwrap();
        let frames = decoder.into_frames().collect_frames().unwrap();
        assert_eq!(frames.len(), 3);
        assert_eq!(frames[0].buffer().dimensions(), (240, 240));

        // A cache hit never decodes the source.
        fs::write(&src, b"not an image any more").unwrap();
        assert_eq!(render(&src, &cache, 240).unwrap(), anim);

        // Small GIFs are passed through untouched.
        let small = save(&dir, &gif(32, 32, 2)).unwrap();
        let out = render(&small, &cache, 240).unwrap();
        assert_eq!(fs::read(out).unwrap(), fs::read(small).unwrap());
    }
}
