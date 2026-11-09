// Created: 2026-11-05
// Reason: feat: implement Phase A download context actions and filesystem parity
// Last updated: 2026-11-05
// Reason: feat: implement Phase A download context actions and filesystem parity
// Description: Frontend tests verifying context menu actions, confirmation prompts, and download operations.

'use strict';

const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const test = require('node:test');
const vm = require('node:vm');

const tauriApiSource = fs.readFileSync(path.join(__dirname, 'tauri-api.js'), 'utf8');

function createRendererEnvironment(options = {}) {
  const notifications = [];
  const apiCalls = [];
  let confirmResponse = options.confirmResponse ?? true;
  const confirmPrompts = [];

  const window = {
    downloader: null,
    confirm: prompt => {
      confirmPrompts.push(prompt);
      return typeof confirmResponse === 'function' ? confirmResponse(prompt) : confirmResponse;
    },
    dispatchEvent() {},
    __TAURI__: {
      core: {
        invoke: async (cmd, args) => {
          apiCalls.push({ cmd, args });
          if (options.invokeHandler) {
            return options.invokeHandler(cmd, args);
          }
          if (cmd === 'download_action') {
            if (options.actionError) {
              throw options.actionError;
            }
            return null;
          }
          if (cmd === 'remove_completed') {
            return options.removeCompletedResult || [];
          }
          if (cmd === 'snapshot') {
            return { downloads: options.downloads || [], settings: {} };
          }
          return null;
        }
      }
    }
  };

  vm.runInNewContext(tauriApiSource, {
    window,
    Error,
    CustomEvent: class CustomEvent {
      constructor(type, opts) {
        this.type = type;
        this.detail = opts.detail;
      }
    }
  });

  const api = window.downloader;

  // Simulate renderer helper functions (matching renderer/app.js)
  function notify(msg) {
    notifications.push(msg);
  }

  function messageOf(error) {
    return typeof error === 'string' ? error : error?.message || 'Unknown error';
  }

  async function performAction(d, action) {
    if (!d) return;
    if (
      action === 'remove' &&
      !window.confirm(`Delete “${d.filename || 'this download'}” from the download list?\n\nThe downloaded file will remain on your PC.`)
    ) {
      return;
    }
    try {
      await api.action({ id: d.id, action });
    } catch (error) {
      notify(messageOf(error));
    }
  }

  async function removeCompleted(downloads) {
    const completed = downloads.filter(d => d.status === 'complete' || d.status === 'completed');
    if (
      !completed.length ||
      !window.confirm(`Delete ${completed.length} completed download(s) from the list?\n\nThe downloaded files will remain on your PC.`)
    ) {
      return;
    }
    const results = await Promise.allSettled(
      completed.map(d => api.action({ id: d.id, action: 'remove' }))
    );
    const failures = results.filter(r => r.status === 'rejected');
    if (failures.length) notify(messageOf(failures[0].reason));
  }

  return {
    window,
    api,
    apiCalls,
    notifications,
    confirmPrompts,
    performAction,
    removeCompleted,
    setConfirmResponse: val => { confirmResponse = val; }
  };
}

test('User cancels remove confirmation prompt: no action is dispatched', async () => {
  const env = createRendererEnvironment({ confirmResponse: false });
  const download = { id: 'dl-1', filename: 'report.pdf', status: 'downloading' };

  await env.performAction(download, 'remove');

  assert.equal(env.confirmPrompts.length, 1);
  assert.match(env.confirmPrompts[0], /Delete “report.pdf”/);
  assert.equal(env.apiCalls.length, 0, 'api.action should not be called when user cancels');
});

test('User confirms remove prompt: api.action remove is dispatched', async () => {
  const env = createRendererEnvironment({ confirmResponse: true });
  const download = { id: 'dl-1', filename: 'report.pdf', status: 'paused' };

  await env.performAction(download, 'remove');

  assert.equal(env.confirmPrompts.length, 1);
  assert.equal(env.apiCalls.length, 1);
  assert.equal(env.apiCalls[0].cmd, 'download_action');
  assert.deepEqual(env.apiCalls[0].args, { id: 'dl-1', action: 'remove' });
});

test('User clicks Open on incomplete download: error notification is surfaced', async () => {
  const env = createRendererEnvironment({
    actionError: new Error('This download has not finished.')
  });
  const download = { id: 'dl-2', filename: 'video.mp4', status: 'downloading' };

  await env.performAction(download, 'open');

  assert.equal(env.apiCalls.length, 1);
  assert.deepEqual(env.apiCalls[0].args, { id: 'dl-2', action: 'open' });
  assert.equal(env.notifications.length, 1);
  assert.equal(env.notifications[0], 'This download has not finished.');
});

test('User clicks Open with on missing file: error notification is surfaced', async () => {
  const env = createRendererEnvironment({
    actionError: new Error('The downloaded file no longer exists.')
  });
  const download = { id: 'dl-3', filename: 'audio.mp3', status: 'completed' };

  await env.performAction(download, 'openWith');

  assert.equal(env.apiCalls.length, 1);
  assert.deepEqual(env.apiCalls[0].args, { id: 'dl-3', action: 'openWith' });
  assert.equal(env.notifications.length, 1);
  assert.equal(env.notifications[0], 'The downloaded file no longer exists.');
});

test('User clicks Reveal: dispatches reveal action without confirmation prompt', async () => {
  const env = createRendererEnvironment();
  const download = { id: 'dl-4', filename: 'archive.tar.gz', status: 'completed' };

  await env.performAction(download, 'reveal');

  assert.equal(env.confirmPrompts.length, 0, 'reveal should not ask for confirmation');
  assert.equal(env.apiCalls.length, 1);
  assert.deepEqual(env.apiCalls[0].args, { id: 'dl-4', action: 'reveal' });
});

test('removeCompleted with no completed downloads: exits early without prompt or calls', async () => {
  const env = createRendererEnvironment();
  const downloads = [
    { id: '1', status: 'downloading' },
    { id: '2', status: 'paused' }
  ];

  await env.removeCompleted(downloads);

  assert.equal(env.confirmPrompts.length, 0);
  assert.equal(env.apiCalls.length, 0);
});

test('removeCompleted with completed downloads: prompts user and removes all completed on confirm', async () => {
  const env = createRendererEnvironment({ confirmResponse: true });
  const downloads = [
    { id: '1', status: 'completed' },
    { id: '2', status: 'downloading' },
    { id: '3', status: 'completed' },
    { id: '4', status: 'paused' }
  ];

  await env.removeCompleted(downloads);

  assert.equal(env.confirmPrompts.length, 1);
  assert.match(env.confirmPrompts[0], /Delete 2 completed download\(s\)/);
  assert.equal(env.apiCalls.length, 2);
  assert.deepEqual(env.apiCalls.map(c => c.args), [
    { id: '1', action: 'remove' },
    { id: '3', action: 'remove' }
  ]);
});

test('removeCompleted when user rejects prompt: does not call remove', async () => {
  const env = createRendererEnvironment({ confirmResponse: false });
  const downloads = [{ id: '1', status: 'completed' }];

  await env.removeCompleted(downloads);

  assert.equal(env.confirmPrompts.length, 1);
  assert.equal(env.apiCalls.length, 0);
});

test('native removeCompleted command in downloader api invokes remove_completed', async () => {
  const env = createRendererEnvironment({
    removeCompletedResult: ['dl-10', 'dl-20']
  });

  const result = await env.api.removeCompleted();
  assert.deepEqual(result, ['dl-10', 'dl-20']);
  assert.equal(env.apiCalls[0].cmd, 'remove_completed');
});
