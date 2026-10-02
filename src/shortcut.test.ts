import assert from "node:assert/strict";
import test from "node:test";
import { accelerator } from "./shortcut.ts";

const none = { ctrlKey: false, altKey: false, shiftKey: false, metaKey: false };

test("modifiers come out in a fixed order before the key", () => {
  assert.equal(accelerator({ ...none, metaKey: true, ctrlKey: true, code: "KeyB" }), "Control+Super+KeyB");
  assert.equal(accelerator({ ...none, shiftKey: true, altKey: true, code: "Digit1" }), "Alt+Shift+Digit1");
});

test("needs a real key and Ctrl, Alt or Cmd", () => {
  assert.equal(accelerator({ ...none, ctrlKey: true, code: "ControlLeft" }), null);
  assert.equal(accelerator({ ...none, shiftKey: true, code: "KeyB" }), null);
  assert.equal(accelerator({ ...none, code: "KeyB" }), null);
});
