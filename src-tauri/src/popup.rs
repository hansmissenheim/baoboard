//! Opening the popup next to the text caret, and pasting into the app it was
//! opened over.

use std::{thread, time::Duration};

use enigo::{Direction, Enigo, Key, Keyboard, Settings};
use tauri::{
    AppHandle, Emitter, LogicalPosition, Manager, PhysicalPosition, Position, WebviewWindow,
};

#[cfg(target_os = "macos")]
use crate::mac as platform;
#[cfg(windows)]
use crate::win as platform;

/// Popup size in logical pixels; keep in sync with `tauri.conf.json`.
const WIDTH: f64 = 360.0;
const HEIGHT: f64 = 320.0;

/// A screen rectangle in the platform's native units: points on macOS,
/// physical pixels on Windows. Those are the units the caret APIs return and
/// `monitor_from_point` takes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

/// Top-left corner for a `w`×`h` popup: above the anchor like WeChat's sticker
/// panel, below it when there is no room above, and always inside `area`.
fn place(anchor: Rect, w: f64, h: f64, area: Rect) -> (f64, f64) {
    const GAP: f64 = 6.0;
    let above = anchor.y - GAP - h;
    let y = if above >= area.y {
        above
    } else {
        anchor.y + anchor.h + GAP
    };
    let x = anchor.x.min(area.x + area.w - w).max(area.x);
    let y = y.min(area.y + area.h - h).max(area.y);
    (x, y)
}

/// Physical pixels per native unit on a monitor with this scale factor.
fn pixels_per_unit(scale: f64) -> f64 {
    if cfg!(target_os = "macos") {
        scale
    } else {
        1.0
    }
}

fn window(app: &AppHandle) -> WebviewWindow {
    app.get_webview_window("main")
        .expect("main window is in tauri.conf.json")
}

/// Opens the popup, or closes it if it is already open.
///
/// From the shortcut it opens at the caret and closes when it loses focus,
/// like the emoji picker. From the tray it opens at the cursor and is `sticky`:
/// it stays open, so files can be dragged onto it from another app.
pub fn toggle(app: &AppHandle, sticky: bool) -> tauri::Result<()> {
    let window = window(app);
    if window.is_visible()? {
        return dismiss(app);
    }
    #[cfg(windows)]
    platform::remember_foreground();

    let caret = if sticky { None } else { platform::caret() };
    let anchor = match caret {
        Some(caret) => caret,
        None => {
            let cursor = app.cursor_position()?;
            // tao converts the macOS cursor with the primary monitor's scale.
            let primary = app.primary_monitor()?.map_or(1.0, |m| m.scale_factor());
            let ppu = pixels_per_unit(primary);
            Rect {
                x: cursor.x / ppu,
                y: cursor.y / ppu,
                w: 0.0,
                h: 0.0,
            }
        }
    };

    if let Some(monitor) = app
        .monitor_from_point(anchor.x, anchor.y)?
        .or(app.primary_monitor()?)
    {
        let scale = monitor.scale_factor();
        let ppu = pixels_per_unit(scale);
        let work = monitor.work_area();
        let area = Rect {
            x: work.position.x as f64 / ppu,
            y: work.position.y as f64 / ppu,
            w: work.size.width as f64 / ppu,
            h: work.size.height as f64 / ppu,
        };
        let size = scale / ppu;
        let (x, y) = place(anchor, WIDTH * size, HEIGHT * size, area);
        window.set_position(match cfg!(target_os = "macos") {
            true => Position::Logical(LogicalPosition::new(x, y)),
            false => Position::Physical(PhysicalPosition::new(x as i32, y as i32)),
        })?;
    }

    window.emit("opened", sticky)?;
    #[cfg(target_os = "macos")]
    app.show()?;
    window.show()?;
    window.set_focus()
}

/// Hides the popup and hands focus back to the app it was opened over.
pub fn dismiss(app: &AppHandle) -> tauri::Result<()> {
    window(app).hide()?;
    // Hiding the whole app makes macOS reactivate the previous one.
    #[cfg(target_os = "macos")]
    app.hide()?;
    #[cfg(windows)]
    platform::restore_foreground();
    Ok(())
}

/// Dismisses the popup and sends ⌘V / Ctrl+V to the app underneath.
pub fn paste_into_previous(app: &AppHandle) -> tauri::Result<()> {
    dismiss(app)?;
    // ponytail: fixed wait for the previous app to take focus; poll the
    // frontmost app instead if pastes land too early on slow machines.
    thread::sleep(Duration::from_millis(150));
    // enigo reads the keyboard layout through APIs that must run on the main
    // thread on macOS.
    app.run_on_main_thread(|| {
        if let Err(e) = send_paste() {
            // The sticker is still on the clipboard for a manual paste.
            eprintln!("paste failed: {e}");
        }
    })
}

fn send_paste() -> Result<(), Box<dyn std::error::Error>> {
    let mut enigo = Enigo::new(&Settings::default())?;
    let modifier = if cfg!(target_os = "macos") {
        Key::Meta
    } else {
        Key::Control
    };
    enigo.key(modifier, Direction::Press)?;
    let typed = enigo.key(Key::Unicode('v'), Direction::Click);
    enigo.key(modifier, Direction::Release)?;
    Ok(typed?)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SCREEN: Rect = Rect {
        x: 0.0,
        y: 25.0,
        w: 1440.0,
        h: 875.0,
    };

    fn caret(x: f64, y: f64) -> Rect {
        Rect {
            x,
            y,
            w: 1.0,
            h: 18.0,
        }
    }

    #[test]
    fn opens_above_the_caret() {
        assert_eq!(
            place(caret(100.0, 700.0), 360.0, 320.0, SCREEN),
            (100.0, 374.0)
        );
    }

    #[test]
    fn opens_below_when_the_caret_is_near_the_top() {
        assert_eq!(
            place(caret(100.0, 100.0), 360.0, 320.0, SCREEN),
            (100.0, 124.0)
        );
    }

    #[test]
    fn stays_inside_the_screen() {
        assert_eq!(
            place(caret(1400.0, 700.0), 360.0, 320.0, SCREEN),
            (1080.0, 374.0)
        );
        let tiny = Rect {
            x: 0.0,
            y: 0.0,
            w: 300.0,
            h: 300.0,
        };
        assert_eq!(place(caret(50.0, 50.0), 360.0, 320.0, tiny), (0.0, 0.0));
        // Second monitor to the left of the primary one.
        let left = Rect {
            x: -1920.0,
            y: 0.0,
            w: 1920.0,
            h: 1080.0,
        };
        assert_eq!(
            place(caret(-10.0, 900.0), 360.0, 320.0, left),
            (-360.0, 574.0)
        );
    }
}
