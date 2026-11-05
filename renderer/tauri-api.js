'use strict';

const tauri = window.__TAURI__;

function commandError(error) {
  const message = typeof error === 'string'
    ? error
    : error?.message || 'The Tauri command failed.';
  const normalized = new Error(message);
  if (typeof error?.code === 'string') normalized.code = error.code;
  return normalized;
}

function invoke(command, args) {
  if (typeof tauri?.core?.invoke !== 'function') {
    return Promise.reject(new Error('The Tauri command bridge is unavailable.'));
  }
  return tauri.core.invoke(command, args).catch(error => {
    throw commandError(error);
  });
}

function unavailable(feature) {
  const error = new Error(`${feature} is not available in the Tauri migration yet.`);
  error.code = 'feature_not_migrated';
  return Promise.reject(error);
}

function subscribe(eventName, callback) {
  let unlisten;
  let cancelled = false;
  if (typeof tauri?.event?.listen !== 'function') {
    throw new Error('The Tauri event bridge is unavailable.');
  }

  tauri.event.listen(eventName, event => callback(event.payload)).then(dispose => {
    if (cancelled) dispose();
    else unlisten = dispose;
  }).catch(error => {
    window.dispatchEvent(new CustomEvent('downloader:api-error', {
      detail: commandError(error)
    }));
  });

  return () => {
    cancelled = true;
    unlisten?.();
  };
}

async function chooseDirectory() {
  if (typeof tauri?.dialog?.open !== 'function') {
    throw new Error('The native folder picker is unavailable.');
  }
  return tauri.dialog.open({ directory: true, multiple: false });
}

async function readClipboard() {
  if (typeof tauri?.clipboardManager?.readText !== 'function') {
    throw new Error('Clipboard access is unavailable.');
  }
  return tauri.clipboardManager.readText();
}

window.downloader = Object.freeze({
  snapshot: () => invoke('snapshot'),
  settings: patch => invoke('update_settings', { patch }),
  chooseDirectory,
  clipboard: readClipboard,
  subscribe: callback => subscribe('downloader:change', callback),
  onCommand: callback => subscribe('downloader:command', callback),
  add: () => unavailable('Downloads'),
  action: () => unavailable('Download actions'),
  review: () => unavailable('Browser download review'),
  integration: () => unavailable('Browser integration'),
  browserSetup: () => unavailable('Browser setup'),
  openProgress: () => unavailable('Download details'),
  openIntegration: () => unavailable('Browser integration'),
  openOptions: () => Promise.resolve(),
  openAdd: () => Promise.resolve(),
  openScheduler: () => Promise.resolve(),
  closeProgress: () => Promise.resolve(),
  resizeProgress: () => Promise.resolve()
});
