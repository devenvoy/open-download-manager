// Created: 2026-11-06
// Reason: extension: keep raw URLs out of quality-row tooltips and add #6 regression coverage
// Last updated: 2026-11-06
// Reason: extension: keep raw URLs out of quality-row tooltips and add #6 regression coverage
// Description: Sanitized tooltip builder preventing signed URL tokens or credentials from appearing in UI hover text.

(function (root) {
  // Tooltip text must never leak a raw media URL: signed URLs expire and can
  // carry credentials, and page-controlled producers supply the reason string.
  function safeTooltip(value) {
    return String(value || '')
      .replace(/https?:\/\/\S+/gi, '')
      .replace(/\s{2,}/g, ' ')
      .trim()
      .slice(0, 180);
  }
  root.odmSafeTooltip = safeTooltip;
  if (typeof module !== 'undefined') module.exports = { safeTooltip };
})(globalThis);
