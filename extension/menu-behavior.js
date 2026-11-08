// Created: 2026-10-29
// Reason: Release Awesome Downloader 0.1.1
// Last updated: 2026-11-05
// Reason: rebrand: migrate project to Open Download Manager
// Description: Quality selection popup menu open, close, positioning, and desktop handoff handlers.

globalThis.odmMenuResult = (response, { accepted, rejected }) => {
  if (response?.ok) {
    accepted();
    return true;
  }
  rejected(response?.error || 'The desktop app did not accept this download.');
  return false;
};
