// Created: 2026-11-06
// Reason: extension: keyboard navigation for the quality chooser
// Last updated: 2026-11-06
// Reason: extension: keyboard navigation for the quality chooser
// Description: Keyboard navigation handler for the media panel quality selection popup menu.

(function (root) {
  // Roving-focus keyboard navigation for the quality chooser rows.
  const deltas = { ArrowDown: 1, ArrowUp: -1 };
  function rows(chooser) {
    return Array.from(chooser.querySelectorAll('.row')).filter(row => !row.disabled);
  }
  function targetIndex(list, active, key) {
    if (!list.length) return -1;
    const current = list.indexOf(active);
    if (key === 'Home') return 0;
    if (key === 'End') return list.length - 1;
    if (deltas[key]) {
      if (current < 0) return deltas[key] > 0 ? 0 : list.length - 1;
      return Math.min(list.length - 1, Math.max(0, current + deltas[key]));
    }
    return -1;
  }
  // Returns true when the key was handled. `active` is the shadow root's
  // activeElement; `close` dismisses the menu and restores trigger focus.
  function handleKey(event, chooser, active, { close } = {}) {
    const key = event.key;
    if (key === 'Escape') {
      event.preventDefault();
      if (close) close();
      return true;
    }
    const list = rows(chooser);
    const index = targetIndex(list, active, key);
    if (index < 0) return false;
    event.preventDefault();
    const row = list[index];
    row.focus();
    if (row.scrollIntoView) row.scrollIntoView({ block: 'nearest' });
    return true;
  }
  function focusFirst(chooser) {
    const first = rows(chooser)[0];
    if (first) first.focus();
    return first || null;
  }
  root.odmChooserKeys = { handleKey, focusFirst, targetIndex };
  if (typeof module !== 'undefined') module.exports = { handleKey, focusFirst, targetIndex };
})(globalThis);
