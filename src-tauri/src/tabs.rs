//! Tabs group stickers into collections shown above the grid. The All tab
//! isn't stored: it always holds every sticker, and a sticker can also be in
//! any number of these tabs. They live in `tabs.json` in the sticker folder.

use std::{collections::HashSet, fs, path::Path, sync::Mutex};

use crate::stickers::{self, Result};

const FILE: &str = "tabs.json";
/// Uploaded tab icons, in a subfolder so they never show up as stickers.
const ICONS: &str = "icons";

#[derive(serde::Serialize, serde::Deserialize)]
pub struct Tab {
    pub id: String,
    pub name: String,
    /// The icon when it isn't an image. Empty falls back to the name's first letter.
    pub emoji: String,
    /// The icon image's file name in `icons/`. `list` returns its full path.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image: Option<String>,
    /// Sticker ids in the order this tab shows them. Deleted stickers can
    /// linger here, so readers skip ids they don't know.
    pub stickers: Vec<String>,
}

/// All tabs in display order, with icon images as full paths.
pub fn list(dir: &Path) -> Vec<Tab> {
    let mut tabs = load(dir).unwrap_or_default();
    for tab in &mut tabs {
        tab.image = tab
            .image
            .take()
            .map(|name| dir.join(ICONS).join(name).to_string_lossy().into_owned());
    }
    tabs
}

/// Creates a tab when `id` is `None`, otherwise renames and re-icons that
/// one, and returns its id. A non-empty emoji replaces any icon image.
pub fn save(dir: &Path, id: Option<&str>, name: &str, emoji: &str) -> Result<String> {
    let (name, emoji) = (name.trim(), emoji.trim());
    if name.is_empty() {
        return Err("Give the tab a name".into());
    }
    edit(dir, |tabs| {
        let tab = match id {
            Some(id) => find(tabs, id)?,
            None => {
                tabs.push(Tab {
                    id: stickers::next_id().to_string(),
                    name: String::new(),
                    emoji: String::new(),
                    image: None,
                    stickers: Vec::new(),
                });
                tabs.last_mut().unwrap()
            }
        };
        tab.name = name.into();
        tab.emoji = emoji.into();
        if !emoji.is_empty() {
            tab.image = None;
        }
        Ok(tab.id.clone())
    })
}

/// Makes the image in `bytes` the tab's icon, replacing its emoji.
pub fn set_image(dir: &Path, id: &str, bytes: &[u8]) -> Result<()> {
    let icons = dir.join(ICONS);
    fs::create_dir_all(&icons)?;
    edit(dir, |tabs| {
        let tab = find(tabs, id)?;
        let path = stickers::save(&icons, bytes)?;
        tab.image = path.file_name().and_then(|n| n.to_str()).map(Into::into);
        tab.emoji.clear();
        Ok(())
    })
}

/// Removes a tab. Its stickers stay in the All tab.
pub fn delete(dir: &Path, id: &str) -> Result<()> {
    edit(dir, |tabs| {
        find(tabs, id)?;
        tabs.retain(|t| t.id != id);
        Ok(())
    })
}

/// Adds stickers to the front of a tab, last one first, or removes them.
/// Stickers already in the tab keep their place.
pub fn tag(dir: &Path, id: &str, ids: &[String], on: bool) -> Result<()> {
    edit(dir, |tabs| {
        let tab = find(tabs, id)?;
        if !on {
            tab.stickers.retain(|s| !ids.contains(s));
            return Ok(());
        }
        for s in ids {
            if !tab.stickers.contains(s) {
                tab.stickers.insert(0, s.clone());
            }
        }
        Ok(())
    })
}

/// Moves a sticker to `index` among the tab's stickers, counting only the
/// ones that still exist, as the grid shows them.
pub fn reorder(dir: &Path, id: &str, sticker: &str, index: usize) -> Result<()> {
    let existing: HashSet<String> = stickers::list(dir)?.into_iter().map(|s| s.id).collect();
    edit(dir, |tabs| {
        let tab = find(tabs, id)?;
        tab.stickers.retain(|s| existing.contains(s));
        stickers::move_to(&mut tab.stickers, sticker, index)
    })
}

/// The stored tabs: none when the file is missing, an error when it can't be read.
fn load(dir: &Path) -> Result<Vec<Tab>> {
    match fs::read(dir.join(FILE)) {
        Ok(bytes) => Ok(serde_json::from_slice(&bytes)?),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(e) => Err(e.into()),
    }
}

/// Loads the tabs, changes them with `f`, and saves them unless `f` fails.
/// Icon images no tab uses any more are deleted once the save succeeds.
fn edit<T>(dir: &Path, f: impl FnOnce(&mut Vec<Tab>) -> Result<T>) -> Result<T> {
    // Imports run off the main thread, so two edits can overlap.
    static LOCK: Mutex<()> = Mutex::new(());
    let _guard = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    // An unreadable file is an error rather than no tabs, so it never gets overwritten.
    let mut tabs = load(dir)?;
    let before: Vec<String> = tabs.iter().filter_map(|t| t.image.clone()).collect();
    let out = f(&mut tabs)?;
    stickers::write_atomic(&dir.join(FILE), &serde_json::to_vec(&tabs)?)?;
    for name in before {
        if !tabs.iter().any(|t| t.image.as_ref() == Some(&name)) {
            let _ = fs::remove_file(dir.join(ICONS).join(name));
        }
    }
    Ok(out)
}

fn find<'a>(tabs: &'a mut [Tab], id: &str) -> Result<&'a mut Tab> {
    tabs.iter_mut()
        .find(|t| t.id == id)
        .ok_or_else(|| format!("no tab {id}").into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{io::Cursor, path::PathBuf};

    fn tempdir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("baoboard-tabs-{}", stickers::next_id()));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn png() -> Vec<u8> {
        let mut bytes = Vec::new();
        image::RgbaImage::new(4, 4)
            .write_to(&mut Cursor::new(&mut bytes), image::ImageFormat::Png)
            .unwrap();
        bytes
    }

    fn sticker(dir: &Path) -> String {
        let path = stickers::save(dir, &png()).unwrap();
        path.file_stem().unwrap().to_str().unwrap().to_owned()
    }

    #[test]
    fn tag_reorder_and_delete() {
        let dir = tempdir();
        let (a, b, c) = (sticker(&dir), sticker(&dir), sticker(&dir));
        let id = save(&dir, None, " Cats ", "🐱").unwrap();
        let shown = || list(&dir).remove(0).stickers;

        tag(&dir, &id, &[a.clone(), b.clone()], true).unwrap();
        assert_eq!(shown(), [&b, &a].map(String::as_str));
        // Tagging again keeps the place; new ones go first.
        tag(&dir, &id, &[a.clone(), c.clone()], true).unwrap();
        assert_eq!(shown(), [&c, &b, &a].map(String::as_str));

        // Index 1 counts only stickers that still exist.
        stickers::delete(&dir, &tempdir(), &b).unwrap();
        reorder(&dir, &id, &a, 1).unwrap();
        assert_eq!(shown(), [&c, &a].map(String::as_str));

        tag(&dir, &id, std::slice::from_ref(&c), false).unwrap();
        assert_eq!(shown(), [&a].map(String::as_str));
        assert!(tag(&dir, "missing", std::slice::from_ref(&a), true).is_err());

        delete(&dir, &id).unwrap();
        assert!(list(&dir).is_empty());
        assert_eq!(stickers::list(&dir).unwrap().len(), 2);
    }

    #[test]
    fn an_unreadable_file_is_never_overwritten() {
        let dir = tempdir();
        fs::write(dir.join(FILE), b"{ broken").unwrap();
        assert!(save(&dir, None, "Cats", "").is_err());
        assert_eq!(fs::read(dir.join(FILE)).unwrap(), b"{ broken");
        assert!(list(&dir).is_empty());
    }

    #[test]
    fn icons_are_an_emoji_or_an_image() {
        let dir = tempdir();
        assert!(save(&dir, None, "  ", "").is_err());
        let id = save(&dir, None, "Cats", "🐱").unwrap();

        set_image(&dir, &id, &png()).unwrap();
        let tab = list(&dir).remove(0);
        assert_eq!(tab.emoji, "");
        let image = PathBuf::from(tab.image.unwrap());
        assert!(image.exists());
        assert!(set_image(&dir, &id, b"not an image").is_err());
        // Icons never show up as stickers.
        assert!(stickers::list(&dir).unwrap().is_empty());

        // Renaming without an emoji keeps the image; an emoji replaces it.
        save(&dir, Some(&id), "Kittens", "").unwrap();
        assert!(list(&dir)[0].image.is_some());
        save(&dir, Some(&id), "Kittens", "😺").unwrap();
        let tab = list(&dir).remove(0);
        assert_eq!((tab.name.as_str(), tab.emoji.as_str()), ("Kittens", "😺"));
        assert!(tab.image.is_none() && !image.exists());
    }
}
