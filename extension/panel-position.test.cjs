const test = require('node:test');
const assert = require('node:assert/strict');
const vm = require('node:vm');
const fs = require('node:fs');
const path = require('node:path');

const context = { globalThis: {} };
vm.runInNewContext(fs.readFileSync(path.join(__dirname, 'panel-position.js'), 'utf8'), context);
const { placement } = context.globalThis.odmPanelPosition;

const viewport = { width: 1200, height: 800 };
const size = { width: 180, height: 27 };
const rect = (top, overrides = {}) => ({ top, bottom: top + 350, left: 0, right: 600, width: 600, height: 350, ...overrides });

const at = place => [place && place.top, place && place.left];

test('fully visible video near the viewport top keeps the legacy below-top placement', () => {
  assert.deepEqual(at(placement(rect(10), viewport, size, false)), [14, 412]);
  assert.deepEqual(at(placement(rect(32), viewport, size, false)), [36, 412], 'below the video top while there is no room above');
});

test('video with room above keeps the legacy above-top placement', () => {
  assert.equal(placement(rect(100), viewport, size, false).top, 100 - 27 - 4, 'panel sits above the video top');
  assert.deepEqual(at(placement(rect(50), viewport, size, false)), [19, 412]);
  assert.equal(placement(rect(33), viewport, size, false).top, 5, 'clamped to the 5px viewport margin exactly as the legacy code did');
});

test('partially scrolled-off video keeps the panel over its visible part', () => {
  const place = placement(rect(-200), viewport, size, false);
  assert.ok(place, 'panel stays visible');
  assert.equal(place.top, 5, 'panel is pinned inside the viewport top');
  assert.equal(place.left, 412);
});

test('video that has effectively left the viewport hides the panel', () => {
  assert.equal(placement(rect(-340), viewport, size, false), null, 'only a 10px sliver remains');
  assert.equal(placement(rect(-351), viewport, size, false), null, 'fully above');
  assert.equal(placement(rect(771), viewport, size, false), null, 'only a 29px strip below');
  assert.equal(placement(rect(800), viewport, size, false), null, 'fully below');
  assert.equal(placement(rect(50, { left: -620, right: -20 }), viewport, size, false), null, 'fully off the left');
});

test('boundary: a 30px visible strip still counts as in view', () => {
  assert.ok(placement(rect(-320), viewport, size, false));
  assert.ok(placement(rect(770), viewport, size, false));
});

test('small videos, dismissal and active scrolling hide the panel', () => {
  assert.equal(placement(rect(50, { right: 100, width: 100 }), viewport, size, false), null);
  assert.equal(placement(rect(50, { bottom: 110, height: 60 }), viewport, size, false), null);
  assert.equal(placement(rect(50), viewport, size, true), null);
});

test('panel stays inside the viewport near its right edge', () => {
  const place = placement(rect(50, { left: 1100, right: 1300 }), viewport, size, false);
  assert.equal(place.left, 1200 - 180 - 5);
});

test('preview or thumbnail video with inside: true places the panel inside the video frame', () => {
  const normal = placement(rect(100), viewport, size, false);
  const inside = placement(rect(100), viewport, size, false, { inside: true });
  assert.equal(normal.top, 69, 'normal placement sits above the video');
  assert.equal(inside.top, 104, 'inside placement sits inside the top edge of the video');
  assert.equal(inside.left, normal.left, 'horizontal alignment matches');
});
