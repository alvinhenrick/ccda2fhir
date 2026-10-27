// Runs the built Node module on fixtures and checks it gives exactly what the native converter's
// golden files hold. Usage: node scripts/wasm-smoke.cjs <dir with the nodejs build>
'use strict';
const fs = require('fs');
const path = require('path');
const assert = require('assert');

const dir = path.resolve(process.argv[2] || 'target/wasm-nodejs');
const wasm = require(path.join(dir, 'ccda2fhir_wasm.js'));
const root = path.resolve(__dirname, '..', 'crates', 'ccda2fhir', 'tests');

assert.match(wasm.version(), /^\d+\.\d+\.\d+/);

let checked = 0;
for (const name of fs.readdirSync(path.join(root, 'fixtures')).sort()) {
  if (!name.endsWith('.xml')) continue;
  const stem = name.slice(0, -4);
  const goldenPath = path.join(root, 'golden', stem + '.json');
  if (!fs.existsSync(goldenPath)) continue;
  const xml = fs.readFileSync(path.join(root, 'fixtures', name), 'utf8');
  const result = JSON.parse(wasm.convert_ccda(xml, undefined));
  const golden = JSON.parse(fs.readFileSync(goldenPath, 'utf8'));
  assert.deepStrictEqual(result.bundle, golden.bundle, stem + ': bundle differs from the native golden');
  assert.ok(['complete', 'partial'].includes(result.status), stem);
  checked++;
}
assert.ok(checked >= 10, 'expected many fixtures, checked ' + checked);

// profiles=none declares no profile; a bad document throws without echoing it.
const doc = fs.readFileSync(path.join(root, 'fixtures', 'problems.xml'), 'utf8');
const none = JSON.parse(wasm.convert_ccda(doc, 'none'));
assert.strictEqual(none.bundle.entry[0].resource.meta, undefined);
assert.throws(() => wasm.convert_ccda('<Secret-Name/>', undefined), (e) => !String(e.message).includes('Secret'));

console.log('wasm smoke test passed: ' + checked + ' fixtures identical to the native goldens');
