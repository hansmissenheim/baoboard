mod clipboard;
#[cfg(target_os = "macos")]
mod mac;
mod popup;
mod stickers;
#[cfg(windows)]
mod win;

use std::path::{Path, PathBuf};

use tauri::{
    AppHandle, Manager, State,
    image::Image,
    ipc::{InvokeBody, Request},
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

struct Dirs {
    stickers: PathBuf,
    cache: PathBuf,
}

/// Longest side of a pasted sticker, in pixels.
const SIZE: u32 = 240;

#[cfg(target_os = "macos")]
const SHORTCUT: (&str, &str) = ("Control+Super+KeyB", "⌃⌘B");
#[cfg(not(target_os = "macos"))]
const SHORTCUT: (&str, &str) = ("Control+Alt+KeyB", "Ctrl+Alt+B");

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
async fn send_sticker(app: AppHandle, dirs: State<'_, Dirs>, id: String) -> Result<(), String> {
    let copy = || {
        let path = stickers::render(&stickers::find(&dirs.stickers, &id)?, &dirs.cache, SIZE)?;
        clipboard::copy(&path)
    };
    copy().map_err(|e| e.to_string())?;
    popup::paste_into_previous(&app).map_err(|e| e.to_string())
}

#[tauri::command]
fn dismiss(app: AppHandle) -> Result<(), String> {
    popup::dismiss(&app).map_err(|e| e.to_string())
}

fn tray(app: &tauri::App) -> tauri::Result<()> {
    let open = MenuItem::with_id(app, "open", "Open BaoBoard", true, None::<&str>)?;
    let hint = format!("Paste a sticker: {}", SHORTCUT.1);
    let hint = MenuItem::with_id(app, "hint", hint, false, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit BaoBoard", true, None::<&str>)?;
    // macOS wants a monochrome template image in the menu bar; Windows shows
    // the full-color app icon in the notification area.
    #[cfg(target_os = "macos")]
    let icon = Image::from_bytes(include_bytes!("../icons/tray.png"))?;
    #[cfg(not(target_os = "macos"))]
    let icon = app.default_window_icon().unwrap().clone();
    TrayIconBuilder::new()
        .icon(icon)
        .icon_as_template(true)
        .tooltip("BaoBoard")
        .menu(&Menu::with_items(app, &[&open, &hint, &quit])?)
        .show_menu_on_left_click(true)
        .on_menu_event(|app, event| match event.id().as_ref() {
            "open" => {
                let _ = popup::toggle(app, true);
            }
            "quit" => app.exit(0),
            _ => {}
        })
        .build(app)?;
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .setup(|app| {
            #[cfg(target_os = "macos")]
            {
                // No Dock icon: BaoBoard lives in the menu bar.
                app.set_activation_policy(tauri::ActivationPolicy::Accessory);
                mac::request_access();
            }
            tray(app)?;
            let toggle =
                |app: &AppHandle, _: &_, event: tauri_plugin_global_shortcut::ShortcutEvent| {
                    if event.state == ShortcutState::Pressed {
                        let _ = popup::toggle(app, false);
                    }
                };
            if let Err(e) = app.global_shortcut().on_shortcut(SHORTCUT.0, toggle) {
                eprintln!("could not register {}: {e}", SHORTCUT.0);
            }

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
            dismiss,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
