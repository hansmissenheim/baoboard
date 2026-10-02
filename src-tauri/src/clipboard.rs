//! Moving stickers through the system clipboard.

use std::{fs, path::Path};

use clipboard_rs::{
    Clipboard, ClipboardContent, ClipboardContext, RustImageData, common::RustImage,
};

use crate::stickers::Result;

/// Puts a rendered sticker on the clipboard as a file, which is how chat apps
/// like Slack accept an animated GIF. A plain image goes alongside it for apps
/// that only paste bitmaps; Chromium-based apps ignore it when a file is there.
pub fn copy(path: &Path) -> Result<()> {
    let image = ClipboardContent::Image(RustImageData::from_path(&path.to_string_lossy())?);
    // `Files` would become a separate pasteboard item, so write the file URL
    // into the image's item. It has to come first or AppKit drops it.
    #[cfg(target_os = "macos")]
    let contents = vec![
        ClipboardContent::Other(
            "public.file-url".into(),
            tauri::Url::from_file_path(path)
                .map_err(|_| "sticker path is not absolute")?
                .to_string()
                .into_bytes(),
        ),
        image,
    ];
    // Setting an image empties the Windows clipboard, so it goes first.
    #[cfg(not(target_os = "macos"))]
    let contents = vec![
        image,
        ClipboardContent::Files(vec![path.to_string_lossy().into_owned()]),
    ];
    ClipboardContext::new()?.set(contents)
}

/// The images on the clipboard: copied files if there are any, otherwise the
/// copied image as a PNG.
pub fn read_images() -> Result<Vec<Result<Vec<u8>>>> {
    let ctx = ClipboardContext::new()?;
    if let Ok(files) = ctx.get_files()
        && !files.is_empty()
    {
        return Ok(files.iter().map(|f| Ok(fs::read(f)?)).collect());
    }
    let png = ctx
        .get_image()
        .map_err(|_| "the clipboard has no image")?
        .to_png()?;
    Ok(vec![Ok(png.get_bytes().to_vec())])
}
