import assert from "node:assert/strict";
import test from "node:test";
import { move } from "./grid.ts";

test("arrows move within the grid and stop at its edges", () => {
  assert.equal(move(0, "ArrowRight", 5, 12), 1);
  assert.equal(move(0, "ArrowLeft", 5, 12), 0);
  assert.equal(move(0, "ArrowUp", 5, 12), 0);
  assert.equal(move(2, "ArrowDown", 5, 12), 7);
  assert.equal(move(11, "ArrowRight", 5, 12), 11);
  assert.equal(move(4, "Enter", 5, 12), 4);
});

test("down onto a shorter last row lands on the last cell", () => {
  assert.equal(move(8, "ArrowDown", 5, 12), 11);
  assert.equal(move(11, "ArrowDown", 5, 12), 11);
});
