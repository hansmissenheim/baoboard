import assert from "node:assert/strict";
import test from "node:test";
import { cycle, visible, type Tab } from "./tabs.ts";

const sticker = (id: string) => ({ id, path: `/${id}.png` });
const tab = (id: string, stickers: string[]): Tab => ({ id, name: id, emoji: "", image: null, stickers });

test("a tab shows its own stickers in its order, skipping deleted ones", () => {
  const all = ["a", "b", "c"].map(sticker);
  assert.deepEqual(visible(all, undefined), all);
  assert.deepEqual(visible(all, tab("t", ["c", "gone", "a"])).map((s) => s.id), ["c", "a"]);
});

test("cycling wraps around through the All tab", () => {
  const tabs = [tab("x", []), tab("y", [])];
  assert.equal(cycle(null, tabs, 1), "x");
  assert.equal(cycle("y", tabs, 1), null);
  assert.equal(cycle(null, tabs, -1), "y");
  assert.equal(cycle("deleted", tabs, 1), "x");
  assert.equal(cycle(null, [], 1), null);
});
