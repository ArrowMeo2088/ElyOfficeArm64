import { test } from "node:test";
import assert from "node:assert/strict";
import { order } from "../src/iso.ts";
import { ACCENT, model } from "../src/model.ts";

const all = model.flatMap((p) => p.blocks);
const [desk] = model[0].blocks;

test("the model draws without intersections or cycles", () => {
  assert.equal(order(all).length, all.length);
});

test("every block stands on the desk", () => {
  for (const b of all.slice(1)) {
    assert.ok(b.x >= 0 && b.y >= 0 && b.x + b.w <= desk.w && b.y + b.d <= desk.d, JSON.stringify(b));
    assert.ok(b.z >= desk.h);
  }
});

test("the accent covers at most a tenth of the visible tops", () => {
  const area = (list: typeof all) => list.reduce((s, b) => s + b.w * b.d, 0);
  assert.ok(area(all.filter((b) => b.tone === ACCENT)) / area([desk]) <= 0.1);
});

test("six apps between the desk and the roads", () => {
  assert.deepEqual(model.slice(1, -1).map((p) => p.name), ["ElyDoc", "ElyTable", "ElyShow", "ElyPDF", "ElyTree", "ElyMark"]);
});
