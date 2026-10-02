# BaoBoard

A sticker keyboard for macOS and Windows, modeled on WeChat stickers. Save images and GIFs, press a global shortcut, pick one, and it gets pasted into the app you were typing in, at a standard sticker size.

## Use

- Press <kbd>⌃⌘B</kbd> on macOS or <kbd>Ctrl+Alt+B</kbd> on Windows, and your stickers open next to the text cursor.
- Pick a sticker with the arrow keys or the mouse. <kbd>Enter</kbd> or a click pastes it, <kbd>Esc</kbd> closes the popup.
- To add stickers, paste an image or file with <kbd>⌘V</kbd> / <kbd>Ctrl+V</kbd> in the popup, or click **+**. To drag images in, choose **Open BaoBoard** from the menu bar (or tray) icon. That opens the popup in a mode where it stays open.
- To delete a sticker, press <kbd>Delete</kbd> twice or right-click it.

Stickers are pasted with their longest side at 240 px. Animated images go out as GIFs, everything else as PNG. To change the size or the shortcut, use ⚙︎ in the popup.

On macOS, BaoBoard asks for Accessibility access on first launch. It needs this to find the text cursor and to paste.

## Develop

```sh
mise install
npm ci
mise run dev    # run the app
mise run check  # what CI runs
```
