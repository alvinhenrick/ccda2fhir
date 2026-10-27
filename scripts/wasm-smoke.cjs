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

// The first place two JSON values differ, as a path with both sides (short).
function firstDifference(a, b, at = '$') {
  if (a === b) return null;
  if (typeof a !== typeof b || a === null || b === null || typeof a !== 'object') {
    return at + ': wasm=' + JSON.stringify(a)?.slice(0, 80) + ' native=' + JSON.stringify(b)?.slice(0, 80);
  }
  const keys = new Set([...Object.keys(a), ...Object.keys(b)]);
  for (const k of keys) {
    const d = firstDifference(a[k], b[k], at + '.' + k);
    if (d) return d;
  }
  return null;
}

let checked = 0;
const mismatches = [];
for (const name of fs.readdirSync(path.join(root, 'fixtures')).sort()) {
  if (!name.endsWith('.xml')) continue;
  const stem = name.slice(0, -4);
  const goldenPath = path.join(root, 'golden', stem + '.json');
  if (!fs.existsSync(goldenPath)) continue;
  const xml = fs.readFileSync(path.join(root, 'fixtures', name), 'utf8');
  // The goldens hold a placeholder where the converter's own version appears (software Device).
  const result = JSON.parse(wasm.convert_ccda(xml, undefined).split(wasm.version()).join('<crate-version>'));
  const golden = JSON.parse(fs.readFileSync(goldenPath, 'utf8'));
  const diff = firstDifference(result.bundle, golden.bundle);
  if (diff) { mismatches.push(stem + ' -> ' + diff); continue; }
  assert.ok(['complete', 'partial'].includes(result.status), stem);
  checked++;
}
assert.deepStrictEqual(mismatches, [], 'bundles differ from the native goldens');
assert.ok(checked >= 10, 'expected many fixtures, checked ' + checked);

// profiles=none declares no profile; a bad document throws without echoing it.
const doc = fs.readFileSync(path.join(root, 'fixtures', 'problems.xml'), 'utf8');
const none = JSON.parse(wasm.convert_ccda(doc, 'none'));
assert.strictEqual(none.bundle.entry[0].resource.meta, undefined);
assert.throws(() => wasm.convert_ccda('<Secret-Name/>', undefined), (e) => !String(e.message).includes('Secret'));

console.log('wasm smoke test passed: ' + checked + ' fixtures identical to the native goldens');
