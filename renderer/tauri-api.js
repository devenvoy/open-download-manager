// Created: 2026-11-05
// Reason: feat: scaffold Tauri settings shell
// Last updated: 2026-11-08
// Reason: feat(settings): expose category folders and temporary directory in options (Closes #32)
// Reason: feat(clipboard): add optional clipboard link monitoring (Closes #33)
// Reason: feat(media): add opt-in advanced browser media adapter (Closes #38)
// Reason: feat(mms): add MMS streaming support or explicit unsupported guidance (Closes #35)
// Reason: feat(proxy): import system and browser proxy configurations (Closes #39)
// Reason: feat(queues): add named download queues and recurring schedules (Closes #13)
// Description: Frontend API bridge exposing typed JavaScript wrappers for Tauri IPC commands and event subscriptions.

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
  checkDestinationConflict: ({ directory, filename }) => invoke('check_destination_conflict', { directory, filename }),
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
  resolveAsxPlaylist: urlOrContent => invoke('resolve_asx_playlist', { urlOrContent }),
  getVideoFormats: url => invoke('get_video_formats', { url }),
  addPlaylistDownloads: request => invoke('add_playlist_downloads', { request }),
  getMediaToolsStatus: () => invoke('get_media_tools_status'),
  installMediaTool: tool => invoke('install_media_tool', { tool }),
  testProxy: ({ proxy, testUrl }) => invoke('test_proxy_connection', { proxy, testUrl: testUrl || null }),
  detectProxy: () => invoke('detect_proxy'),
  applyDetectedProxy: ({ detected }) => invoke('apply_detected_proxy', { detected }),
  listCredentialRules: () => invoke('list_credential_rules'),
  saveCredentialRule: rule => invoke('save_credential_rule', { rule }),
  deleteCredentialRule: id => invoke('delete_credential_rule', { id }),
  listQueues: () => invoke('list_queues'),
  createQueue: ({ name, concurrencyLimit, speedLimitKbps, schedule }) => invoke('create_queue', { name, concurrencyLimit, speedLimitKbps, schedule }),
  updateQueue: ({ id, name, enabled, concurrencyLimit, speedLimitKbps, schedule }) => invoke('update_queue', { id, name, enabled, concurrencyLimit, speedLimitKbps, schedule }),
  deleteQueue: ({ id, disposition }) => invoke('delete_queue', { id, disposition }),
  startQueue: ({ id }) => invoke('start_queue', { id }),
  pauseQueue: ({ id }) => invoke('pause_queue', { id }),
  assignDownloadToQueue: ({ downloadId, queueId }) => invoke('assign_download_to_queue', { downloadId, queueId }),
  reorderQueueItems: ({ queueId, itemIds }) => invoke('reorder_queue_items', { queueId, itemIds }),
  updateDownloadCompletionActions: ({ id, actions }) => invoke('update_download_completion_actions', { id, actions }),
  retryDownloadActions: ({ id }) => invoke('retry_download_actions', { id }),
  validateDirectory: path => invoke('validate_directory', { path }),
  dismissClipboardCandidate: url => invoke('dismiss_clipboard_candidate', { url }),
  getAdvancedIntegration: () => invoke('get_advanced_integration_capability'),
  enableAdvancedIntegration: ({ consent }) => invoke('enable_advanced_integration', { consent }),
  disableAdvancedIntegration: () => invoke('disable_advanced_integration'),
  repairAdvancedIntegration: () => invoke('repair_advanced_integration')
});
