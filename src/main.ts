import { convertFileSrc, invoke, type InvokeArgs, type InvokeOptions } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { Menu, MenuItem } from "@tauri-apps/api/menu";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { move } from "./grid.ts";
import { toFrames } from "./media.ts";
import { accelerator } from "./shortcut.ts";

type Sticker = { id: string; path: string };
type Settings = { size: number; shortcut: string; label: string };

const COLS = 5; // matches grid-template-columns in styles.css
const PASTE = navigator.userAgent.includes("Mac") ? "⌘V" : "Ctrl+V";

const grid = document.querySelector<HTMLElement>("#grid")!;
const status = document.querySelector<HTMLElement>("#status")!;
const picker = document.querySelector<HTMLInputElement>("#picker")!;
const settings = document.querySelector<HTMLElement>("#settings")!;
const sizeSelect = document.querySelector<HTMLSelectElement>("#size")!;
const shortcutButton = document.querySelector<HTMLButtonElement>("#shortcut")!;

let stickers: Sticker[] = [];
// Cell 0 is the add button, so sticker i sits in cell i + 1.
let selected = 1;
let pendingDelete: string | null = null;
// Opened from the tray: stays open when it loses focus, so files can be dropped on it.
let sticky = false;
// The file picker steals focus without meaning "close the popup".
let picking = false;
// The shortcut button is waiting for a key combination.
let recording = false;
// The sticker the context menu was opened on.
let menuTarget = "";
// Items passed inline to Menu.new lose their action once the menu is built,
// so the item is created on its own first.
const stickerMenu = MenuItem.new({ text: "Delete", action: () => remove(menuTarget) }).then((item) =>
  Menu.new({ items: [item] }),
);

function say(text = "") {
  status.textContent = text;
}

async function call(cmd: string, args?: InvokeArgs, options?: InvokeOptions): Promise<boolean> {
  try {
    await invoke(cmd, args, options);
    return true;
  } catch (e) {
    say(String(e));
    return false;
  }
}

function cell(i: number, label: string, child: Node): HTMLElement {
  const el = document.createElement("div");
  el.className = "cell";
  el.role = "option";
  el.ariaLabel = label;
  el.append(child);
  el.addEventListener("mouseenter", () => select(i));
  el.addEventListener("click", () => activate(i));
  return el;
}

function stickerCell(sticker: Sticker, i: number): HTMLElement {
  const img = document.createElement("img");
  img.src = convertFileSrc(sticker.path);
  img.alt = "";
  img.draggable = false;
  img.loading = "lazy";
  const el = cell(i + 1, `Sticker ${i + 1}`, img);
  el.addEventListener("contextmenu", async () => {
    menuTarget = sticker.id;
    await (await stickerMenu).popup();
  });
  return el;
}

async function refresh() {
  stickers = await invoke<Sticker[]>("list_stickers");
  grid.replaceChildren(cell(0, "Add stickers", document.createTextNode("+")), ...stickers.map(stickerCell));
  grid.firstElementChild!.classList.add("add");
  select(Math.min(selected, stickers.length));
  if (!stickers.length) say(`Add stickers with ${PASTE} or +`);
}

function select(i: number) {
  grid.children[selected]?.setAttribute("aria-selected", "false");
  selected = i;
  const el = grid.children[i];
  el?.setAttribute("aria-selected", "true");
  el?.scrollIntoView({ block: "nearest" });
  pendingDelete = null;
}

function activate(i: number) {
  if (i === 0) {
    picking = true;
    picker.click();
  } else {
    // The backend hides the popup and pastes into the app underneath.
    call("send_sticker", { id: stickers[i - 1].id });
  }
}

async function remove(id: string) {
  if (await call("delete_sticker", { id })) say();
  await refresh();
}

async function importFrom(cmd: string, args?: InvokeArgs, options?: InvokeOptions) {
  if (await call(cmd, args, options)) {
    selected = 1;
    say();
  }
  // Refresh either way: a batch can partly succeed.
  await refresh();
}

async function importFiles(files: Iterable<File>) {
  for (const file of files) {
    const bytes = new Uint8Array(await file.arrayBuffer());
    say(`Adding ${file.name}…`);
    try {
      const frames = await toFrames(bytes);
      if (!frames) {
        await importFrom("import_bytes", bytes);
        continue;
      }
      const { width, height, delays, pixels } = frames;
      const headers = { width: String(width), height: String(height), delays: delays.join(",") };
      await importFrom("import_frames", pixels, { headers });
    } catch (e) {
      say(`${file.name}: ${e}`);
    }
  }
}

function showSettings(open: boolean) {
  settings.hidden = !open;
  grid.hidden = open;
  recording = false;
  say();
  if (open) invoke<Settings>("get_settings").then(showValues);
}

function showValues(s: Settings) {
  sizeSelect.value = String(s.size);
  shortcutButton.textContent = s.label;
}

async function update(cmd: string, args: InvokeArgs) {
  try {
    showValues(await invoke<Settings>(cmd, args));
    say();
  } catch (e) {
    say(String(e));
    showValues(await invoke<Settings>("get_settings"));
  }
}

function settingsKey(e: KeyboardEvent) {
  if (!recording) {
    if (e.key === "Escape") showSettings(false);
    return; // let the form controls handle everything else
  }
  e.preventDefault();
  if (e.key === "Escape") {
    recording = false;
    invoke<Settings>("get_settings").then(showValues);
    return;
  }
  const shortcut = accelerator(e);
  if (shortcut) {
    recording = false;
    update("set_shortcut", { shortcut });
  }
}

sizeSelect.addEventListener("change", () => update("set_size", { size: Number(sizeSelect.value) }));
shortcutButton.addEventListener("click", () => {
  recording = true;
  shortcutButton.textContent = "Press keys…";
});
document.querySelector("#gear")!.addEventListener("click", () => showSettings(Boolean(settings.hidden)));

window.addEventListener("keydown", (e) => {
  if (!settings.hidden) {
    settingsKey(e);
  } else if ((e.metaKey || e.ctrlKey) && e.key === "v") {
    e.preventDefault();
    importFrom("import_clipboard");
  } else if (e.key.startsWith("Arrow")) {
    e.preventDefault();
    select(move(selected, e.key, COLS, stickers.length + 1));
    say();
  } else if (e.key === "Escape") {
    invoke("dismiss");
  } else if (e.key === "Enter") {
    e.preventDefault();
    activate(selected);
  } else if ((e.key === "Delete" || e.key === "Backspace") && selected > 0) {
    const id = stickers[selected - 1].id;
    if (pendingDelete === id) {
      remove(id);
    } else {
      pendingDelete = id;
      say("Press Delete again to remove this sticker");
    }
  }
});

picker.addEventListener("cancel", () => (picking = false));
picker.addEventListener("change", async () => {
  picking = false;
  await importFiles(picker.files ?? []);
  picker.value = "";
});

// HTML drops hand over file contents, which formats like MP4 need to be
// decoded here, where Tauri's drop event only gives paths.
document.addEventListener("dragover", (e) => {
  e.preventDefault();
  document.body.classList.add("dropping");
});
document.addEventListener("dragleave", (e) => {
  if (!e.relatedTarget) document.body.classList.remove("dropping");
});
document.addEventListener("drop", (e) => {
  e.preventDefault();
  document.body.classList.remove("dropping");
  importFiles(e.dataTransfer?.files ?? []);
});

listen<boolean>("opened", ({ payload }) => {
  showSettings(false);
  sticky = payload;
  say(sticky ? "Drop images here to add them" : "");
});

getCurrentWindow().onFocusChanged(({ payload: focused }) => {
  if (!focused && !sticky && !picking) getCurrentWindow().hide();
});

document.addEventListener("contextmenu", (e) => e.preventDefault());

refresh();
