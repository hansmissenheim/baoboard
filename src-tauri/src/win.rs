//! Win32 calls for finding the caret and handing focus back.

use std::{
    ffi::c_void,
    sync::atomic::{AtomicIsize, Ordering},
};

use windows::{
    Win32::{
        Foundation::{HWND, POINT},
        Graphics::Gdi::ClientToScreen,
        System::Variant::VARIANT,
        UI::{
            Accessibility::{AccessibleObjectFromWindow, IAccessible},
            WindowsAndMessaging::{
                CHILDID_SELF, GUITHREADINFO, GetForegroundWindow, GetGUIThreadInfo,
                GetWindowThreadProcessId, OBJID_CARET, SetForegroundWindow,
            },
        },
    },
    core::Interface,
};

use crate::popup::Rect;

static PREVIOUS: AtomicIsize = AtomicIsize::new(0);

/// Remembers the window that had focus before the popup opened.
pub fn remember_foreground() {
    PREVIOUS.store(
        unsafe { GetForegroundWindow() }.0 as isize,
        Ordering::SeqCst,
    );
}

pub fn restore_foreground() {
    let hwnd = HWND(PREVIOUS.load(Ordering::SeqCst) as *mut c_void);
    if !hwnd.is_invalid() {
        let _ = unsafe { SetForegroundWindow(hwnd) };
    }
}

/// The text caret of the foreground window, in physical screen pixels.
pub fn caret() -> Option<Rect> {
    unsafe {
        let foreground = GetForegroundWindow();
        let thread = GetWindowThreadProcessId(foreground, None);
        let mut info = GUITHREADINFO {
            cbSize: size_of::<GUITHREADINFO>() as u32,
            ..Default::default()
        };
        GetGUIThreadInfo(thread, &mut info).ok()?;

        if !info.hwndCaret.is_invalid() {
            let r = info.rcCaret;
            let mut origin = POINT {
                x: r.left,
                y: r.top,
            };
            ClientToScreen(info.hwndCaret, &mut origin).ok().ok()?;
            return Some(Rect {
                x: origin.x as f64,
                y: origin.y as f64,
                w: (r.right - r.left) as f64,
                h: (r.bottom - r.top) as f64,
            });
        }

        // Chromium and Electron apps such as Slack draw their own caret and
        // only report it through MSAA.
        let hwnd = match info.hwndFocus.is_invalid() {
            true => foreground,
            false => info.hwndFocus,
        };
        let mut acc: *mut c_void = std::ptr::null_mut();
        AccessibleObjectFromWindow(hwnd, OBJID_CARET.0 as u32, &IAccessible::IID, &mut acc).ok()?;
        let acc = IAccessible::from_raw(acc);
        let (mut x, mut y, mut w, mut h) = (0, 0, 0, 0);
        let me = VARIANT::from(CHILDID_SELF as i32);
        acc.accLocation(&mut x, &mut y, &mut w, &mut h, &me).ok()?;
        (h > 0).then_some(Rect {
            x: x as f64,
            y: y as f64,
            w: w as f64,
            h: h as f64,
        })
    }
}
