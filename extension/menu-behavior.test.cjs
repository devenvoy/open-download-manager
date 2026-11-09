// Created: 2026-10-29
// Reason: Release Awesome Downloader 0.1.1
// Last updated: 2026-11-05
// Reason: rebrand: migrate project to Open Download Manager
// Description: Unit tests verifying quality menu open/close behavior, desktop handoff dispatch, and error display.

const test = require('node:test');
const assert = require('node:assert/strict');
const vm = require('node:vm');
const fs = require('node:fs');
const path = require('node:path');

function behavior() {
  const context = { globalThis: {} };
  vm.runInNewContext(fs.readFileSync(path.join(__dirname, 'menu-behavior.js'), 'utf8'), context);
  return context.globalThis.odmMenuResult;
}

test('successful desktop handoff closes the quality menu', () => {
  let closed = 0, error = '';
  const accepted = behavior()({ ok: true }, { accepted: () => { closed += 1; }, rejected: value => { error = value; } });
  assert.equal(accepted, true);
  assert.equal(closed, 1);
  assert.equal(error, '');
});

test('failed desktop handoff keeps the quality menu open and reports the error', () => {
  let closed = 0, error = '';
  const accepted = behavior()({ ok: false, error: 'Connector unavailable.' }, { accepted: () => { closed += 1; }, rejected: value => { error = value; } });
  assert.equal(accepted, false);
  assert.equal(closed, 0);
  assert.equal(error, 'Connector unavailable.');
});
