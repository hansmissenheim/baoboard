# BaoBoard

A sticker keyboard for macOS and Windows, modeled on WeChat stickers. Save images, GIFs and short videos (PNG, JPEG, GIF, WebP, AVIF, BMP or MP4), press a global shortcut, pick one, and it gets pasted into the app you were typing in, at a standard sticker size.

## Use

- Press <kbd>⌃⌘B</kbd> on macOS or <kbd>Ctrl+Alt+B</kbd> on Windows, and your stickers open next to the text cursor.
- Pick a sticker with the arrow keys or the mouse. <kbd>Enter</kbd> or a click pastes it, <kbd>Esc</kbd> closes the popup.
- To add stickers, paste an image or file with <kbd>⌘V</kbd> / <kbd>Ctrl+V</kbd> in the popup, or click **+**. To drag images in, choose **Open BaoBoard** from the menu bar (or tray) icon. That opens the popup in a mode where it stays open.
- To delete a sticker, press <kbd>Delete</kbd> twice or right-click it.

Stickers are pasted with their longest side at 240 px. Animated images go out as GIFs, everything else as PNG. To change the size or the shortcut, use ⚙︎ in the popup.

On macOS, BaoBoard asks for Accessibility access on first launch. It needs this to find the text cursor and to paste.

## Install

Download the `.dmg` (macOS) or the `-setup.exe` (Windows) from [Releases](https://github.com/hansmissenheim/baoboard/releases). The macOS build is self-signed and the Windows build is unsigned, so the OS warns on first install (#8):

- macOS: drag BaoBoard to Applications and open it. When macOS says it can't verify the app, go to System Settings → Privacy & Security and click **Open Anyway**.
- Windows: if SmartScreen warns, choose **More info → Run anyway**.

After that, BaoBoard checks for a newer release on launch and updates itself.

## Develop

```sh
mise install
npm ci
mise run dev      # run the app
mise run check    # what CI runs
mise run install  # build and install to /Applications (macOS)
```

`mise run install` builds with your local version number, so the updater replaces it once a newer release is published. Each build is signed ad hoc with a new signature, so macOS drops the Accessibility grant; remove BaoBoard from System Settings → Privacy & Security → Accessibility and add it again after installing.

To release, bump `version` in `src-tauri/tauri.conf.json`, run the **Release** workflow from the Actions tab, then review and publish the draft release it creates.
