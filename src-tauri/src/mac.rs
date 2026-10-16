//! macOS-only calls: Accessibility, which needs the user to have granted
//! BaoBoard (or, under `mise run dev`, the terminal) access, and ImageIO.

use std::ptr::{self, NonNull};

use objc2_application_services::{
    AXError, AXIsProcessTrustedWithOptions, AXUIElement, AXValue, AXValueType,
    kAXTrustedCheckOptionPrompt,
};
use objc2_core_foundation::{
    CFBoolean, CFData, CFDictionary, CFMutableData, CFRetained, CFString, CFType, CGRect,
};
use objc2_image_io::{
    CGImageDestination, CGImageSource, kCGImagePropertyAVISDictionary,
    kCGImagePropertyPNGDictionary,
};

use crate::popup::Rect;

/// Shows the system prompt for Accessibility access if it is missing.
pub fn request_access() {
    let options = CFDictionary::from_slices(
        &[unsafe { kAXTrustedCheckOptionPrompt }],
        &[CFBoolean::new(true)],
    );
    unsafe { AXIsProcessTrustedWithOptions(Some(options.as_opaque())) };
}

fn attribute(element: &AXUIElement, name: &str) -> Option<CFRetained<CFType>> {
    let mut value: *const CFType = ptr::null();
    let err = unsafe {
        element.copy_attribute_value(&CFString::from_str(name), NonNull::from(&mut value))
    };
    if err != AXError::Success {
        return None;
    }
    NonNull::new(value.cast_mut()).map(|v| unsafe { CFRetained::from_raw(v) })
}

/// The text caret of the focused app, in screen points.
pub fn caret() -> Option<Rect> {
    let system = unsafe { AXUIElement::new_system_wide() };
    // A hung app would otherwise block the shortcut for the default 6 s.
    unsafe { system.set_messaging_timeout(0.2) };

    // Chromium and Electron apps such as Slack only build their accessibility
    // tree once asked, so the first lookup in them may fall back to the cursor.
    if let Some(app) =
        attribute(&system, "AXFocusedApplication").and_then(|a| a.downcast::<AXUIElement>().ok())
    {
        let flag = CFString::from_str("AXManualAccessibility");
        unsafe { app.set_attribute_value(&flag, CFBoolean::new(true)) };
    }

    let focused = attribute(&system, "AXFocusedUIElement")?
        .downcast::<AXUIElement>()
        .ok()?;
    let range = attribute(&focused, "AXSelectedTextRange")?;
    let mut bounds: *const CFType = ptr::null();
    let err = unsafe {
        focused.copy_parameterized_attribute_value(
            &CFString::from_str("AXBoundsForRange"),
            &range,
            NonNull::from(&mut bounds),
        )
    };
    if err != AXError::Success {
        return None;
    }
    let bounds = unsafe { CFRetained::from_raw(NonNull::new(bounds.cast_mut())?) }
        .downcast::<AXValue>()
        .ok()?;
    let mut rect = CGRect::default();
    let ok = unsafe { bounds.value(AXValueType::CGRect, NonNull::from(&mut rect).cast()) };
    (ok && rect.size.height > 0.0).then_some(Rect {
        x: rect.origin.x,
        y: rect.origin.y,
        w: rect.size.width,
        h: rect.size.height,
    })
}

/// Re-encodes an AVIF as a PNG, animated (APNG) when the AVIF is, since the
/// `image` crate can't decode AV1. ImageIO reads AVIF from macOS 13.
pub fn avif_to_png(avif: &[u8]) -> Option<Vec<u8>> {
    let src = unsafe { CGImageSource::with_data(&CFData::from_bytes(avif), None) }?;
    let count = unsafe { src.count() };
    let png = CFMutableData::new(None, 0)?;
    let dest = unsafe {
        CGImageDestination::with_data(&png, &CFString::from_static_str("public.png"), count, None)
    }?;
    for i in 0..count {
        // AVIS frame timing uses the same keys as APNG's, so it carries over.
        let timing = unsafe { src.properties_at_index(i, None) }
            .and_then(|p| {
                unsafe { p.cast_unchecked::<CFString, CFType>() }
                    .get(unsafe { kCGImagePropertyAVISDictionary })
            })
            .map(|t| {
                CFDictionary::from_slices(&[unsafe { kCGImagePropertyPNGDictionary }], &[&*t])
            });
        unsafe { dest.add_image_from_source(&src, i, timing.as_deref().map(|t| t.as_opaque())) };
    }
    unsafe { dest.finalize() }.then(|| png.to_vec())
}
