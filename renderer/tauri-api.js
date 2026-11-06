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
  openProgress: ({ id }) => invoke('open_progress_window', { id }),
  closeProgress: () => invoke('close_auxiliary_window'),
  resizeProgress: ({ expanded }) => invoke('resize_progress_window', { expanded }),
  add: request => invoke('add_download', { request }),
  action: request => invoke('download_action', request),
  removeCompleted: () => invoke('remove_completed'),
  review: request => invoke('review_download', { request }),
  integration: () => invoke('get_integration_info'),
  browserSetup: ({ action, browser }) => invoke('browser_setup_action', { action, browser }),
  registerBrowser: ({ browser, extensionId }) => invoke('register_browser', { browser, extensionId }),
  openIntegration: () => invoke('open_integration_window'),
  openOptions: () => invoke('open_options_window'),
  openAdd: options => invoke('open_add_window', { options: options || null }),
  openScheduler: () => invoke('open_scheduler_window'),
  exportDownloads: () => invoke('export_downloads_to_file'),
  importDownloads: () => invoke('import_downloads_from_file'),
  showAbout: () => invoke('show_about_dialog'),
  getPlaylistInfo: url => invoke('get_playlist_info', { url }),
  addPlaylistDownloads: request => invoke('add_playlist_downloads', { request }),
  getMediaToolsStatus: () => invoke('get_media_tools_status'),
  installMediaTool: tool => invoke('install_media_tool', { tool })
});
