import assert from "node:assert/strict";

import {
  featureEntries,
  warningEntries,
} from "../examples/wasm-demo/report-format.mjs";

const entries = featureEntries({
  comments: 2,
  fields: 0,
  field_kinds: [{ kind: "PAGE", count: 3 }],
  unsupported_field_reasons: [{ reason: "UnsupportedSwitch", count: 1 }],
  metafiles: [
    {
      path: "word/media/image1.emf",
      format: "EMF",
      bytes: 64,
      compressed: false,
      width_px: 12,
      height_px: 8,
    },
  ],
});

assert.deepEqual(entries, [
  "comments: 2",
  "field_kinds: PAGE=3",
  "unsupported_field_reasons: UnsupportedSwitch=1",
  "metafiles: EMF word/media/image1.emf (64 bytes)",
]);
assert.ok(entries.every((entry) => !entry.includes("undefined")));
assert.deepEqual(
  warningEntries([{ kind: "UnsupportedMetafileImages", count: 1 }]),
  ['UnsupportedMetafileImages: {"kind":"UnsupportedMetafileImages","count":1}'],
);
