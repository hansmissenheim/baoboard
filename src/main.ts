import { convertFileSrc, invoke, type InvokeArgs } from "@tauri-apps/api/core";
import { Menu } from "@tauri-apps/api/menu";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { move } from "./grid.ts";

type Sticker = { id: string; path: string };

const COLS = 5; // matches grid-template-columns in styles.css
const PASTE = navigator.userAgent.includes("Mac") ? "⌘V" : "Ctrl+V";

const grid = document.querySelector<HTMLElement>("#grid")!;
const status = document.querySelector<HTMLElement>("#status")!;
const picker = document.querySelector<HTMLInputElement>("#picker")!;

let stickers: Sticker[] = [];
// Cell 0 is the add button, so sticker i sits in cell i + 1.
let selected = 0;
let pendingDelete: string | null = null;

function say(text = "") {
  status.textContent = text;
}

async function call(cmd: string, args?: InvokeArgs): Promise<boolean> {
  try {
    await invoke(cmd, args);
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
    const items = [{ id: "delete", text: "Delete", action: () => remove(sticker.id) }];
    await (await Menu.new({ items })).popup();
  });
  return el;
}

async function refresh() {
  stickers = await invoke<Sticker[]>("list_stickers");
  grid.replaceChildren(cell(0, "Add stickers", document.createTextNode("+")), ...stickers.map(stickerCell));
  grid.firstElementChild!.classList.add("add");
  select(Math.min(selected, stickers.length));
  if (!stickers.length) say(`Drop images here, paste with ${PASTE}, or click +`);
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
  if (i === 0) picker.click();
  else send(stickers[i - 1].id);
}

async function send(id: string) {
  if (await call("send_sticker", { id })) say(`Copied, paste with ${PASTE}`);
}

async function remove(id: string) {
  if (await call("delete_sticker", { id })) say();
  await refresh();
}

async function importFrom(cmd: string, args?: InvokeArgs) {
  if (await call(cmd, args)) {
    selected = 1;
    say();
  }
  // Refresh either way: a batch can partly succeed.
  await refresh();
}

window.addEventListener("keydown", (e) => {
  if ((e.metaKey || e.ctrlKey) && e.key === "v") {
    e.preventDefault();
    importFrom("import_clipboard");
  } else if (e.key.startsWith("Arrow")) {
    e.preventDefault();
    select(move(selected, e.key, COLS, stickers.length + 1));
    say();
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

picker.addEventListener("change", async () => {
  for (const file of picker.files ?? []) {
    await importFrom("import_bytes", new Uint8Array(await file.arrayBuffer()));
  }
  picker.value = "";
});

getCurrentWebview().onDragDropEvent(({ payload }) => {
  document.body.classList.toggle("dropping", payload.type === "enter" || payload.type === "over");
  if (payload.type === "drop") importFrom("import_files", { paths: payload.paths });
});

document.addEventListener("contextmenu", (e) => e.preventDefault());

refresh();
