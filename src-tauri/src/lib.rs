mod clipboard;
mod stickers;

use std::path::{Path, PathBuf};

use tauri::{
    Manager, State,
    ipc::{InvokeBody, Request},
};

struct Dirs {
    stickers: PathBuf,
    cache: PathBuf,
}

/// Longest side of a pasted sticker, in pixels.
const SIZE: u32 = 240;

fn save_all(dir: &Path, files: Vec<stickers::Result<Vec<u8>>>) -> Result<(), String> {
    let errors: Vec<String> = files
        .into_iter()
        .filter_map(|bytes| bytes.and_then(|b| stickers::save(dir, &b)).err())
        .map(|e| e.to_string())
        .collect();
    match errors.is_empty() {
        true => Ok(()),
        false => Err(errors.join("\n")),
    }
}

#[tauri::command]
fn list_stickers(dirs: State<Dirs>) -> Result<Vec<stickers::Sticker>, String> {
    stickers::list(&dirs.stickers).map_err(|e| e.to_string())
}

#[tauri::command]
async fn import_files(dirs: State<'_, Dirs>, paths: Vec<PathBuf>) -> Result<(), String> {
    let files = paths.iter().map(|p| Ok(std::fs::read(p)?)).collect();
    save_all(&dirs.stickers, files)
}

/// Takes the raw bytes of one image file as the request body.
#[tauri::command]
async fn import_bytes(dirs: State<'_, Dirs>, request: Request<'_>) -> Result<(), String> {
    let InvokeBody::Raw(bytes) = request.body() else {
        return Err("expected raw image bytes".into());
    };
    save_all(&dirs.stickers, vec![Ok(bytes.clone())])
}

#[tauri::command]
async fn import_clipboard(dirs: State<'_, Dirs>) -> Result<(), String> {
    save_all(
        &dirs.stickers,
        clipboard::read_images().map_err(|e| e.to_string())?,
    )
}

#[tauri::command]
fn delete_sticker(dirs: State<Dirs>, id: String) -> Result<(), String> {
    stickers::delete(&dirs.stickers, &dirs.cache, &id).map_err(|e| e.to_string())
}

#[tauri::command]
async fn send_sticker(dirs: State<'_, Dirs>, id: String) -> Result<(), String> {
    let send = || {
        let path = stickers::render(&stickers::find(&dirs.stickers, &id)?, &dirs.cache, SIZE)?;
        clipboard::copy(&path)
    };
    send().map_err(|e| e.to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let stickers = app.path().app_data_dir()?.join("stickers");
            let cache = app.path().app_cache_dir()?;
            std::fs::create_dir_all(&stickers)?;
            std::fs::create_dir_all(&cache)?;
            app.manage(Dirs { stickers, cache });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            list_stickers,
            import_files,
            import_bytes,
            import_clipboard,
            delete_sticker,
            send_sticker,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
