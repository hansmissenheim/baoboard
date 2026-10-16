import assert from "node:assert/strict";
import test from "node:test";
import { kind } from "./media.ts";

const ftyp = (brand: string) => new Uint8Array([0, 0, 0, 0x20, ...new TextEncoder().encode(`ftyp${brand}`)]);

test("ftyp brands tell AVIF from MP4", () => {
  assert.equal(kind(ftyp("avif")), "avif");
  assert.equal(kind(ftyp("avis")), "avif");
  assert.equal(kind(ftyp("isom")), "mp4");
  assert.equal(kind(ftyp("mp42")), "mp4");
});

test("anything else is left to the backend", () => {
  assert.equal(kind(new TextEncoder().encode("GIF89a......")), undefined);
  assert.equal(kind(new Uint8Array(3)), undefined);
});
