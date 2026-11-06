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
