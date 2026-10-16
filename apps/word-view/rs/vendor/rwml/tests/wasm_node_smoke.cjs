const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");

if (process.argv.length !== 4) {
  throw new Error("usage: node tests/wasm_node_smoke.cjs <binding-dir> <fixture.docx>");
}

const bindingDir = path.resolve(process.argv[2]);
const fixture = path.resolve(process.argv[3]);
const rwml = require(path.join(bindingDir, "rwml.js"));
const bytes = new Uint8Array(fs.readFileSync(fixture));

assert.equal(rwml.extractText(bytes), "Alpha\nBeta");
assert.equal(rwml.markdown(bytes), "Alpha\n\nBeta");
assert.equal(rwml.html(bytes), "<p>Alpha</p><p>Beta</p>");

const report = JSON.parse(rwml.reportJson(bytes));
assert.equal(report.format, "docx");
assert.equal(report.stats.paragraphs, 2);
assert.equal(report.features.comments, 2);
assert.throws(() => rwml.reportJson(Uint8Array.of(1, 2, 3)));

console.log("wasm_node_smoke exports=4 fixture=comments.docx status=pass");
