import assert from "node:assert/strict";
import { test } from "node:test";
import { formatMs } from "./format.ts";

test("formatMs keeps sub-millisecond p99 values in microseconds", () => {
  assert.equal(formatMs(0.086), "86 µs");
  assert.equal(formatMs(0.074), "74 µs");
  assert.equal(formatMs(0.094), "94 µs");
  assert.equal(formatMs(0.068), "68 µs");
  assert.equal(formatMs(0.5), "500 µs");
});

test("formatMs still uses milliseconds and seconds above 1 ms", () => {
  assert.equal(formatMs(0), "0 ms");
  assert.equal(formatMs(1), "1 ms");
  assert.equal(formatMs(42), "42 ms");
  assert.equal(formatMs(380), "380 ms");
  assert.equal(formatMs(1100), "1.10 s");
  assert.equal(formatMs(12_000), "12.0 s");
});
