const test = require('node:test');
const assert = require('node:assert/strict');
const vm = require('node:vm');
const fs = require('node:fs');
const path = require('node:path');

function load() {
  const context = { globalThis: {}, module: undefined };
  vm.runInNewContext(fs.readFileSync(path.join(__dirname, 'chooser-keyboard.js'), 'utf8'), context);
  return context.globalThis.odmChooserKeys;
}

function row(disabled = false) {
  return { disabled, focused: 0, scrolled: 0,
    focus() { this.focused += 1; },
    scrollIntoView() { this.scrolled += 1; } };
}
function chooserWith(list) {
  return { querySelectorAll(selector) { return selector === '.row' ? list : []; } };
}
const keyEvent = key => ({ key, prevented: 0, preventDefault() { this.prevented += 1; } });

test('arrow keys move focus between enabled rows and clamp at the edges', () => {
  const keys = load();
  const rows = [row(), row(), row()];
  const chooser = chooserWith(rows);
  let event = keyEvent('ArrowDown');
  assert.equal(keys.handleKey(event, chooser, null), true);
  assert.equal(rows[0].focused, 1);
  assert.equal(event.prevented, 1);
  keys.handleKey(keyEvent('ArrowDown'), chooser, rows[0]);
  assert.equal(rows[1].focused, 1);
  keys.handleKey(keyEvent('ArrowDown'), chooser, rows[1]);
  keys.handleKey(keyEvent('ArrowDown'), chooser, rows[2]);
  assert.equal(rows[2].focused, 2, 'focus clamps on the last row');
  keys.handleKey(keyEvent('ArrowUp'), chooser, rows[2]);
  assert.equal(rows[1].focused, 2);
  assert.equal(rows[1].scrolled > 0, true, 'focused row is scrolled into view');
});

test('ArrowUp with no focused row starts at the last row', () => {
  const keys = load();
  const rows = [row(), row()];
  keys.handleKey(keyEvent('ArrowUp'), chooserWith(rows), null);
  assert.equal(rows[1].focused, 1);
});

test('Home and End jump to the first and last enabled rows', () => {
  const keys = load();
  const rows = [row(), row(), row()];
  const chooser = chooserWith(rows);
  keys.handleKey(keyEvent('End'), chooser, null);
  assert.equal(rows[2].focused, 1);
  keys.handleKey(keyEvent('Home'), chooser, rows[2]);
  assert.equal(rows[0].focused, 1);
});

test('disabled rows are skipped', () => {
  const keys = load();
  const rows = [row(), row(true), row()];
  const chooser = chooserWith(rows);
  keys.handleKey(keyEvent('ArrowDown'), chooser, rows[0]);
  assert.equal(rows[1].focused, 0);
  assert.equal(rows[2].focused, 1);
  keys.handleKey(keyEvent('ArrowUp'), chooser, rows[2]);
  assert.equal(rows[0].focused, 1);
});

test('Escape closes the menu through the supplied callback', () => {
  const keys = load();
  let closed = 0;
  const event = keyEvent('Escape');
  assert.equal(keys.handleKey(event, chooserWith([row()]), null, { close: () => { closed += 1; } }), true);
  assert.equal(closed, 1);
  assert.equal(event.prevented, 1);
});

test('unrelated keys are ignored and focusFirst focuses the first enabled row', () => {
  const keys = load();
  const rows = [row(true), row()];
  assert.equal(keys.handleKey(keyEvent('Tab'), chooserWith(rows), null), false);
  assert.equal(keys.focusFirst(chooserWith(rows)), rows[1]);
  assert.equal(rows[1].focused, 1);
});
