//! User settings, stored as JSON in the app config dir.

use std::{fs, io, ops::RangeInclusive, path::Path};

use serde::{Deserialize, Serialize};

/// Sticker sizes worth rendering, in pixels.
pub const SIZES: RangeInclusive<u32> = 16..=1024;

#[derive(Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// Longest side of a pasted sticker, in pixels.
    pub size: u32,
    /// Global shortcut that opens the popup, as a Tauri accelerator.
    pub shortcut: String,
}

impl Default for Settings {
    fn default() -> Self {
        let shortcut = if cfg!(target_os = "macos") {
            "Control+Super+KeyB"
        } else {
            "Control+Alt+KeyB"
        };
        Self {
            size: 240,
            shortcut: shortcut.into(),
        }
    }
}

/// A missing, unreadable or out-of-range file gives the defaults.
pub fn load(path: &Path) -> Settings {
    fs::read(path)
        .ok()
        .and_then(|bytes| serde_json::from_slice::<Settings>(&bytes).ok())
        .filter(|s| SIZES.contains(&s.size))
        .unwrap_or_default()
}

pub fn save(path: &Path, settings: &Settings) -> io::Result<()> {
    fs::write(path, serde_json::to_vec_pretty(settings)?)
}

/// How a shortcut reads on screen: `⌃⌘B` on macOS, `Ctrl+Alt+B` on Windows.
pub fn label(shortcut: &str, mac: bool) -> String {
    let keys: Vec<&str> = shortcut
        .split('+')
        .map(|key| match (key, mac) {
            ("Control", true) => "⌃",
            ("Control", false) => "Ctrl",
            ("Alt", true) => "⌥",
            ("Shift", true) => "⇧",
            ("Super", true) => "⌘",
            ("Super", false) => "Win",
            _ => key
                .strip_prefix("Key")
                .or(key.strip_prefix("Digit"))
                .unwrap_or(key),
        })
        .collect();
    keys.join(if mac { "" } else { "+" })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labels_read_like_the_platform_writes_them() {
        assert_eq!(label("Control+Super+KeyB", true), "⌃⌘B");
        assert_eq!(label("Control+Alt+Shift+Digit1", true), "⌃⌥⇧1");
        assert_eq!(label("Control+Alt+KeyB", false), "Ctrl+Alt+B");
        assert_eq!(label("Super+Shift+Space", false), "Win+Shift+Space");
    }

    #[test]
    fn missing_or_partial_files_fall_back_to_defaults() {
        let dir = std::env::temp_dir().join(format!("baoboard-settings-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("settings.json");
        assert_eq!(load(&path).size, 240);

        fs::write(&path, r#"{"size": 160}"#).unwrap();
        let partial = load(&path);
        assert_eq!(partial.size, 160);
        assert_eq!(partial.shortcut, Settings::default().shortcut);

        save(
            &path,
            &Settings {
                size: 320,
                ..partial
            },
        )
        .unwrap();
        assert_eq!(load(&path).size, 320);
    }
}
