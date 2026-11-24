import { convertFileSrc, invoke, type InvokeArgs, type InvokeOptions } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { CheckMenuItem, Menu, MenuItem, Submenu } from "@tauri-apps/api/menu";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { move } from "./grid.ts";
import { toFrames } from "./media.ts";
import { accelerator } from "./shortcut.ts";
import { cycle, visible, type Sticker, type Tab } from "./tabs.ts";

type Settings = { size: number; shortcut: string; label: string };

const COLS = 5; // matches grid-template-columns in styles.css
const PASTE = navigator.userAgent.includes("Mac") ? "⌘V" : "Ctrl+V";
const EMOJIS = "❤️ ⭐ 😂 🥺 😭 😡 👍 🙏 🎉 🔥 🐱 🐶 🐧 🐻 🐰 🐸 🍔 🍰 ☕ 🌸 🌙 💤 💩 ✨".split(" ");

const grid = document.querySelector<HTMLElement>("#grid")!;
const status = document.querySelector<HTMLElement>("#status")!;
const picker = document.querySelector<HTMLInputElement>("#picker")!;
const settings = document.querySelector<HTMLElement>("#settings")!;
const sizeSelect = document.querySelector<HTMLSelectElement>("#size")!;
const shortcutButton = document.querySelector<HTMLButtonElement>("#shortcut")!;
const tabBar = document.querySelector<HTMLElement>("#tabs")!;
const editor = document.querySelector<HTMLFormElement>("#editor")!;
const tabName = document.querySelector<HTMLInputElement>("#tab-name")!;
const tabEmoji = document.querySelector<HTMLInputElement>("#tab-emoji")!;
const iconPreview = document.querySelector<HTMLElement>("#icon-preview")!;
const iconPicker = document.querySelector<HTMLInputElement>("#icon-picker")!;

let all: Sticker[] = [];
let tabs: Tab[] = [];
// The open tab's id, or null for the All tab.
let current: string | null = null;
// The stickers the open tab shows.
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
// The sticker being dragged to a new spot.
let dragging: string | null = null;
// The tab the editor is changing, or null when it's making a new one.
let editing: string | null = null;
// An icon image picked in the editor and not saved yet.
let iconFile: File | null = null;
// iconFile's preview URL.
let iconUrl = "";
// A tab save is in flight, so a second Enter can't create the tab twice.
let saving = false;

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
  el.draggable = true;
  el.addEventListener("dragstart", (e) => {
    dragging = sticker.id;
    // Some WebKit builds start no drag without data.
    e.dataTransfer?.setData("text/plain", sticker.id);
  });
  el.addEventListener("dragend", () => (dragging = null));
  el.addEventListener("contextmenu", () => stickerMenu(sticker.id));
  return el;
}

// Items passed inline to Menu.new lose their action once the menu is built,
// so the items are created on their own first.
async function stickerMenu(id: string) {
  const tab = current;
  const items: (MenuItem | Submenu)[] = [await MenuItem.new({ text: "Move to Front", action: () => reorder(id, 0) })];
  if (tabs.length) {
    const checks = tabs.map((t) => {
      const on = t.stickers.includes(id);
      return CheckMenuItem.new({ text: t.name, checked: on, action: () => tag(t.id, id, !on) });
    });
    items.push(await Submenu.new({ text: "Tabs", items: await Promise.all(checks) }));
  }
  if (tab) items.push(await MenuItem.new({ text: "Remove from Tab", action: () => tag(tab, id, false) }));
  items.push(await MenuItem.new({ text: "Delete", action: () => remove(id) }));
  await (await Menu.new({ items })).popup();
}

async function tabMenu(id: string) {
  const items = await Promise.all([
    MenuItem.new({ text: "Edit Tab…", action: () => openEditor(id) }),
    MenuItem.new({ text: "Delete Tab", action: () => deleteTab(id) }),
  ]);
  await (await Menu.new({ items })).popup();
}

/** An icon element: the tab's image, its emoji, or its name's first letter. */
function tabIcon(image: string | null, emoji: string, name: string): HTMLElement {
  const el = document.createElement("span");
  el.className = "tab-icon";
  if (image) {
    const img = document.createElement("img");
    img.src = image;
    img.alt = "";
    img.draggable = false;
    el.append(img);
  } else {
    el.textContent = emoji || [...name][0]?.toUpperCase() || "?";
  }
  return el;
}

function tabButton(id: string | null, label: string, icon: HTMLElement, onClick: () => void): HTMLElement {
  const el = document.createElement("div");
  el.className = "tab";
  el.role = "tab";
  el.title = label;
  el.ariaLabel = label;
  el.ariaSelected = String(id === current);
  if (id) el.dataset.tab = id;
  el.append(icon);
  el.addEventListener("click", onClick);
  return el;
}

function renderTabs() {
  const all = tabButton(null, "All", tabIcon(null, "★", ""), () => openTab(null));
  const own = tabs.map((t) => {
    const el = tabButton(t.id, t.name, tabIcon(t.image && convertFileSrc(t.image), t.emoji, t.name), () => openTab(t.id));
    el.addEventListener("contextmenu", () => tabMenu(t.id));
    return el;
  });
  const add = tabButton(null, "New tab", tabIcon(null, "+", ""), () => openEditor(null));
  add.ariaSelected = "false";
  tabBar.replaceChildren(all, ...own, add);
  tabBar.querySelector("[aria-selected=true]")?.scrollIntoView({ inline: "nearest" });
}

async function refresh() {
  [all, tabs] = await Promise.all([invoke<Sticker[]>("list_stickers"), invoke<Tab[]>("list_tabs")]);
  render();
}

function render() {
  const tab = tabs.find((t) => t.id === current);
  if (!tab) current = null;
  stickers = visible(all, tab);
  renderTabs();
  grid.replaceChildren(cell(0, "Add stickers", document.createTextNode("+")), ...stickers.map(stickerCell));
  grid.firstElementChild!.classList.add("add");
  select(Math.min(selected, stickers.length));
  if (!stickers.length) say(tab ? "Drag stickers onto this tab, or add new ones here" : `Add stickers with ${PASTE} or +`);
}

function openTab(id: string | null) {
  current = id;
  selected = 1;
  say();
  render();
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

async function tag(tab: string, id: string, on: boolean) {
  if (await call("tag_sticker", { tab, id, on })) {
    say(on ? `Added to ${tabs.find((t) => t.id === tab)?.name}` : "");
  }
  await refresh();
}

async function reorder(id: string, index: number) {
  if (await call("reorder_sticker", { tab: current, id, index })) {
    selected = index + 1;
    say();
  }
  await refresh();
}

/** The grid cell index under a drag, or -1 when it isn't over one. */
function cellAt(e: DragEvent): number {
  const el = (e.target as Element).closest(".cell");
  return el ? [...grid.children].indexOf(el) : -1;
}

/** Headers that add raw-body imports to the open tab. */
function tabHeaders(): Record<string, string> {
  return current ? { tab: current } : {};
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
        await importFrom("import_bytes", bytes, { headers: tabHeaders() });
        continue;
      }
      const { width, height, delays, pixels } = frames;
      const headers = { width: String(width), height: String(height), delays: delays.join(","), ...tabHeaders() };
      await importFrom("import_frames", pixels, { headers });
    } catch (e) {
      say(`${file.name}: ${e}`);
    }
  }
}

/** Shows one of the popup's views: the sticker grid, settings, or the tab editor. */
function show(view: "grid" | "settings" | "editor") {
  grid.hidden = tabBar.hidden = view !== "grid";
  settings.hidden = view !== "settings";
  editor.hidden = view !== "editor";
  recording = false;
  say();
  if (view === "settings") invoke<Settings>("get_settings").then(showValues);
}

/** Opens the tab editor on tab `id`, or on a new tab when it's null. */
function openEditor(id: string | null) {
  const tab = tabs.find((t) => t.id === id);
  editing = id;
  pickIcon(null);
  tabName.value = tab?.name ?? "";
  tabEmoji.value = tab?.emoji ?? "";
  show("editor");
  showIcon();
  tabName.focus();
}

/** Previews the icon the editor would save: a picked image wins, then an emoji, then the saved image. */
function showIcon() {
  const saved = tabs.find((t) => t.id === editing)?.image;
  const image = iconFile ? iconUrl : saved && convertFileSrc(saved);
  const icon = tabIcon(tabEmoji.value.trim() ? null : image || null, tabEmoji.value.trim(), tabName.value);
  iconPreview.replaceChildren(...icon.childNodes);
}

/** Sets the picked icon image, replacing the old one's preview URL. */
function pickIcon(file: File | null) {
  URL.revokeObjectURL(iconUrl);
  iconFile = file;
  iconUrl = file ? URL.createObjectURL(file) : "";
}

async function saveTab() {
  if (saving) return;
  saving = true;
  try {
    const name = tabName.value;
    editing = await invoke<string>("save_tab", { id: editing, name, emoji: tabEmoji.value });
    if (iconFile) {
      const bytes = new Uint8Array(await iconFile.arrayBuffer());
      await invoke("set_tab_icon", bytes, { headers: { tab: editing } });
    }
    current = editing;
    selected = 1;
    show("grid");
  } catch (e) {
    say(String(e));
  }
  saving = false;
  await refresh();
}

async function deleteTab(id: string) {
  if (await call("delete_tab", { id })) say("Tab deleted. Its stickers are still in All");
  await refresh();
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
    if (e.key === "Escape") show("grid");
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
document.querySelector("#gear")!.addEventListener("click", () => show(settings.hidden ? "settings" : "grid"));

editor.addEventListener("submit", (e) => {
  e.preventDefault();
  saveTab();
});
document.querySelector("#cancel-tab")!.addEventListener("click", () => show("grid"));
tabName.addEventListener("input", showIcon);
tabEmoji.addEventListener("input", () => {
  pickIcon(null);
  showIcon();
});
document.querySelector("#emojis")!.append(
  ...EMOJIS.map((emoji) => {
    const b = document.createElement("button");
    b.type = "button";
    b.textContent = emoji;
    b.addEventListener("click", () => {
      tabEmoji.value = emoji;
      pickIcon(null);
      showIcon();
    });
    return b;
  }),
);
document.querySelector("#upload-icon")!.addEventListener("click", () => {
  picking = true;
  iconPicker.click();
});
iconPicker.addEventListener("cancel", () => (picking = false));
iconPicker.addEventListener("change", () => {
  picking = false;
  pickIcon(iconPicker.files?.[0] ?? null);
  if (iconFile) tabEmoji.value = "";
  iconPicker.value = "";
  showIcon();
});

window.addEventListener("keydown", (e) => {
  if (!settings.hidden) {
    settingsKey(e);
  } else if (!editor.hidden) {
    if (e.key === "Escape") show("grid");
  } else if (e.key === "Tab") {
    e.preventDefault();
    openTab(cycle(current, tabs, e.shiftKey ? -1 : 1));
  } else if ((e.metaKey || e.ctrlKey) && e.key === "v") {
    e.preventDefault();
    importFrom("import_clipboard", { tab: current });
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
    if (current) {
      // Inside a tab, Delete only takes the sticker out of it.
      tag(current, id, false);
    } else if (pendingDelete === id) {
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
  if (dragging === null) document.body.classList.add("dropping");
  else {
    const i = cellAt(e);
    if (i >= 0 && i !== selected) select(i);
  }
});
document.addEventListener("dragleave", (e) => {
  if (!e.relatedTarget) document.body.classList.remove("dropping");
});
document.addEventListener("drop", (e) => {
  e.preventDefault();
  document.body.classList.remove("dropping");
  const tab = (e.target as Element).closest<HTMLElement>(".tab")?.dataset.tab;
  if (dragging === null) {
    // Files dropped on a tab icon go into that tab.
    if (tab) openTab(tab);
    importFiles(e.dataTransfer?.files ?? []);
  } else if (tab) {
    tag(tab, dragging, true);
  } else if (cellAt(e) >= 0) {
    // Dropping on the add button moves the sticker to the front.
    reorder(dragging, Math.max(cellAt(e) - 1, 0));
  }
  dragging = null;
});

listen<boolean>("opened", ({ payload }) => {
  show("grid");
  // A drag that left the window may never have seen its dragend.
  dragging = null;
  sticky = payload;
  say(sticky ? "Drop images here to add them" : "");
});

getCurrentWindow().onFocusChanged(({ payload: focused }) => {
  if (!focused && !sticky && !picking) getCurrentWindow().hide();
});

document.addEventListener("contextmenu", (e) => e.preventDefault());

refresh();
