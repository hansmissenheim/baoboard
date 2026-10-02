mod clipboard;
#[cfg(target_os = "macos")]
mod mac;
mod popup;
mod settings;
mod stickers;
#[cfg(windows)]
mod win;

use std::{
    path::{Path, PathBuf},
    sync::Mutex,
};

use tauri::{
    AppHandle, Manager, State,
    ipc::{InvokeBody, Request},
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

use settings::Settings;

struct Dirs {
    stickers: PathBuf,
    cache: PathBuf,
    settings: PathBuf,
}

fn register_shortcut(
    app: &AppHandle,
    shortcut: &str,
) -> Result<(), tauri_plugin_global_shortcut::Error> {
    app.global_shortcut()
        .on_shortcut(shortcut, |app, _, event| {
            if event.state == ShortcutState::Pressed {
                let _ = popup::toggle(app, false);
            }
        })
}

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
async fn send_sticker(
    app: AppHandle,
    dirs: State<'_, Dirs>,
    settings: State<'_, Mutex<Settings>>,
    id: String,
) -> Result<(), String> {
    let size = settings.lock().unwrap().size;
    let copy = || {
        let path = stickers::render(&stickers::find(&dirs.stickers, &id)?, &dirs.cache, size)?;
        clipboard::copy(&path)
    };
    copy().map_err(|e| e.to_string())?;
    popup::paste_into_previous(&app).map_err(|e| e.to_string())
}

#[tauri::command]
fn dismiss(app: AppHandle) -> Result<(), String> {
    popup::dismiss(&app).map_err(|e| e.to_string())
}

#[derive(serde::Serialize)]
struct SettingsView {
    size: u32,
    shortcut: String,
    label: String,
}

impl From<&Settings> for SettingsView {
    fn from(s: &Settings) -> Self {
        Self {
            size: s.size,
            shortcut: s.shortcut.clone(),
            label: settings::label(&s.shortcut, cfg!(target_os = "macos")),
        }
    }
}

#[tauri::command]
fn get_settings(settings: State<Mutex<Settings>>) -> SettingsView {
    (&*settings.lock().unwrap()).into()
}

#[tauri::command]
fn set_size(
    dirs: State<Dirs>,
    settings: State<Mutex<Settings>>,
    size: u32,
) -> Result<SettingsView, String> {
    if !settings::SIZES.contains(&size) {
        return Err(format!(
            "Size must be {} to {} px",
            settings::SIZES.start(),
            settings::SIZES.end()
        ));
    }
    let mut s = settings.lock().unwrap();
    let old = std::mem::replace(&mut s.size, size);
    settings::save(&dirs.settings, &s).map_err(|e| e.to_string())?;
    if old != size {
        // Renders at the old size are never read again.
        let _ = std::fs::remove_dir_all(dirs.cache.join(old.to_string()));
    }
    Ok((&*s).into())
}

/// Async so the plugin's hop to the main thread never waits on itself.
#[tauri::command]
async fn set_shortcut(
    app: AppHandle,
    dirs: State<'_, Dirs>,
    settings: State<'_, Mutex<Settings>>,
    shortcut: String,
) -> Result<SettingsView, String> {
    // Never hold the lock across registration: it waits on the main thread,
    // which may be waiting on the lock.
    let old = settings.lock().unwrap().shortcut.clone();
    if shortcut != old {
        // Register first, so a shortcut another app owns leaves the old one working.
        register_shortcut(&app, &shortcut).map_err(|e| {
            let label = settings::label(&shortcut, cfg!(target_os = "macos"));
            format!("{label} is not available: {e}")
        })?;
        let _ = app.global_shortcut().unregister(old.as_str());
    }
    let mut s = settings.lock().unwrap();
    s.shortcut = shortcut;
    settings::save(&dirs.settings, &s).map_err(|e| e.to_string())?;
    Ok((&*s).into())
}

fn tray(app: &tauri::App) -> tauri::Result<()> {
    let open = MenuItem::with_id(app, "open", "Open BaoBoard", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit BaoBoard", true, None::<&str>)?;
    // macOS wants a monochrome template image in the menu bar; Windows shows
    // the full-color app icon in the notification area.
    #[cfg(target_os = "macos")]
    let icon = tauri::image::Image::from_bytes(include_bytes!("../icons/tray.png"))?;
    #[cfg(not(target_os = "macos"))]
    let icon = app.default_window_icon().unwrap().clone();
    TrayIconBuilder::new()
        .icon(icon)
        .icon_as_template(true)
        .tooltip("BaoBoard")
        .menu(&Menu::with_items(app, &[&open, &quit])?)
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

            let stickers = app.path().app_data_dir()?.join("stickers");
            let cache = app.path().app_cache_dir()?;
            let config = app.path().app_config_dir()?;
            for dir in [&stickers, &cache, &config] {
                std::fs::create_dir_all(dir)?;
            }
            let settings_path = config.join("settings.json");
            let settings = settings::load(&settings_path);
            if let Err(e) = register_shortcut(app.handle(), &settings.shortcut) {
                // The tray menu still opens the popup.
                eprintln!("could not register {}: {e}", settings.shortcut);
            }
            app.manage(Mutex::new(settings));
            app.manage(Dirs {
                stickers,
                cache,
                settings: settings_path,
            });
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
            get_settings,
            set_size,
            set_shortcut,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
