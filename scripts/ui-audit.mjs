// DOM and accessibility contract for the dependency-free Tauri settings UI.
// Runs in CI without WebKit; native visual QA is still required on macOS.
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';

const read = name => readFileSync(new URL(`../${name}`, import.meta.url), 'utf8');
const html = read('frontend/settings.html');
const style = read('frontend/settings.css');
const settings = read('frontend/settings.js');
const extensions = read('frontend/traffic-settings.js');
const floating = read('frontend/floating.css');

const idMatches = [...html.matchAll(/\bid="([^"]+)"/g)].map(match => match[1]);
const ids = new Set(idMatches);
assert.equal(ids.size, idMatches.length, 'HTML IDs must remain unique');

for (const link of html.matchAll(/href="#([^"]+)"/g)) {
  assert.ok(ids.has(link[1]), `Navigation destination #${link[1]} is missing`);
}
for (const match of settings.matchAll(/\$\('([a-zA-Z][\w-]*)'\)/g)) {
  assert.ok(ids.has(match[1]), `Settings control #${match[1]} is missing`);
}
for (const match of extensions.matchAll(/getElementById\('([a-zA-Z][\w-]*)'\)/g)) {
  assert.ok(ids.has(match[1]), `Extension control #${match[1]} is missing`);
}
for (const id of [
  'historyChart', 'targetRows', 'testResult', 'catAssetResult',
  'floatingShowTraffic', 'trafficInterface', 'networkCatSettings',
  'networkCatTheme', 'networkCatFile', 'save', 'message',
]) {
  assert.ok(ids.has(id), `Required control #${id} is missing`);
}

assert.match(html, /<nav class="section-nav" aria-label=/, 'Named section navigation required');
assert.match(html, /id="message" role="status" aria-live="polite"/, 'Save feedback must be announced');
assert.match(html, /id="historyChart" role="img" aria-label=/, 'Chart requires an accessible name');
assert.match(settings, /class="target-select"[^>]*aria-label=/, 'Target selector requires a button label');
assert.match(settings, /aria-pressed=/, 'Active target selector must expose state');
assert.match(settings, /targetRows'\)\.addEventListener\('click'/, 'Target selection should delegate events');
assert.match(settings, /generation !== chartGeneration/, 'Stale history responses must be rejected');
assert.doesNotMatch(extensions, /insertAdjacentHTML/, 'Extension controls belong in the HTML document');
assert.match(style, /prefers-color-scheme: dark/, 'Dark mode support required');
assert.match(style, /prefers-reduced-motion: reduce/, 'Reduced motion support required');
assert.match(style, /prefers-reduced-transparency: reduce/, 'Reduced transparency support required');
assert.match(floating, /prefers-reduced-motion: reduce/, 'HUD needs reduced motion support');

console.log(`UI contract passed: ${ids.size} unique controls and all section links resolved.`);
