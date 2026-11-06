(function (root) {
  // Geometry for the floating download panel: the panel follows the part of
  // its video that is actually visible instead of disappearing as soon as the
  // video's top edge leaves the viewport.
  function visiblePart(rect, viewport) {
    const top = Math.max(0, rect.top), left = Math.max(0, rect.left);
    const bottom = Math.min(viewport.height, rect.bottom), right = Math.min(viewport.width, rect.right);
    if (bottom - top <= 0 || right - left <= 0) return null;
    return { top, left, bottom, right, width: right - left, height: bottom - top };
  }
  // Where the panel goes for a video rect, or null when it should stay hidden:
  // video too small, effectively scrolled out of view, or temporarily hidden
  // (dismissed by the user, or while a scroll is in flight).
  function placement(rect, viewport, size, hidden) {
    if (hidden) return null;
    const part = visiblePart(rect, viewport);
    if (!part || rect.width <= 100 || rect.height <= 60 || part.height < 30) return null;
    const above = rect.top >= size.height + 6;
    const top = Math.max(5, Math.min(viewport.height - size.height - 5, above ? rect.top - size.height - 4 : Math.max(part.top + 4, rect.top + 4)));
    const left = Math.max(5, Math.min(viewport.width - size.width - 5, rect.right - size.width - 8));
    return { top, left };
  }
  root.odmPanelPosition = { visiblePart, placement };
  if (typeof module !== 'undefined') module.exports = { visiblePart, placement };
})(globalThis);
