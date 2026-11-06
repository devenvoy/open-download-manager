const test = require('node:test');
const assert = require('node:assert/strict');
const vm = require('node:vm');
const fs = require('node:fs');
const path = require('node:path');

const context = { globalThis: {} };
vm.runInNewContext(fs.readFileSync(path.join(__dirname, 'safe-tooltip.js'), 'utf8'), context);
const safeTooltip = context.globalThis.odmSafeTooltip;

test('signed URLs are stripped from tooltip text', () => {
  const signed = 'Expires soon: https://cdn.example.com/vid.mp4?sig=abc123&expires=999 try again';
  assert.equal(safeTooltip(signed), 'Expires soon: try again');
  assert.ok(!safeTooltip(signed).includes('sig=abc123'));
});

test('a reason that is only a URL leaves no tooltip', () => {
  assert.equal(safeTooltip('https://cdn.example.com/vid.mp4?sig=abc'), '');
  assert.equal(safeTooltip(''), '');
  assert.equal(safeTooltip(null), '');
  assert.equal(safeTooltip(undefined), '');
});

test('ordinary reasons pass through unchanged and stay bounded', () => {
  assert.equal(safeTooltip('DASH streams are unsupported.'), 'DASH streams are unsupported.');
  assert.equal(safeTooltip('x'.repeat(300)).length, 180);
});
