'use strict';

const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const test = require('node:test');
const vm = require('node:vm');

const source = fs.readFileSync(path.join(__dirname, 'tauri-api.js'), 'utf8');

function loadApi(tauri = {}) {
  const window = {
    __TAURI__: tauri,
    dispatchEvent() {}
  };
  vm.runInNewContext(source, {
    window,
    Error,
    CustomEvent: class CustomEvent {
      constructor(type, options) {
        this.type = type;
        this.detail = options.detail;
      }
    }
  });
  return window.downloader;
}

test('snapshot invokes the Rust snapshot command', async () => {
  const calls = [];
  const api = loadApi({
    core: {
      invoke: async (...args) => {
        calls.push(args);
        return { downloads: [] };
      }
    }
  });

  const snapshot = await api.snapshot();

  assert.deepEqual(snapshot, { downloads: [] });
  assert.equal(calls[0][0], 'snapshot');
});

test('settings passes a typed patch to the Rust command', async () => {
  const calls = [];
  const api = loadApi({
    core: {
      invoke: async (...args) => {
        calls.push(args);
        return { maxConcurrent: 4 };
      }
    }
  });
  const patch = { maxConcurrent: 4 };

  const updated = await api.settings(patch);

  assert.deepEqual(updated, { maxConcurrent: 4 });
  assert.equal(calls[0][0], 'update_settings');
  assert.equal(JSON.stringify(calls[0][1]), JSON.stringify({ patch }));
});

test('download details use the native progress window commands', async () => {
  const calls = [];
  const api = loadApi({
    core: {
      invoke: async (...args) => {
        calls.push(args);
        return null;
      }
    }
  });

  await api.openProgress({ id: 'download-123' });
  await api.resizeProgress({ expanded: true });
  await api.closeProgress();

  assert.equal(JSON.stringify(calls), JSON.stringify([
    ['open_progress_window', { id: 'download-123' }],
    ['resize_progress_window', { expanded: true }],
    ['close_auxiliary_window', undefined]
  ]));
});

test('Rust errors preserve their message and stable code', async () => {
  const api = loadApi({
    core: {
      invoke: async () => {
        throw { code: 'invalid_settings', message: 'Choose an absolute path.' };
      }
    }
  });

  await assert.rejects(api.snapshot(), error => {
    assert.equal(error.code, 'invalid_settings');
    assert.equal(error.message, 'Choose an absolute path.');
    return true;
  });
});

test('adding a download invokes the Rust HTTP download command', async () => {
  const calls = [];
  const api = loadApi({
    core: {
      invoke: async (...args) => {
        calls.push(args);
        return ['download-123'];
      }
    }
  });
  const request = { urls: ['https://example.test/file.bin'], queueOnly: false };

  assert.deepEqual(await api.add(request), ['download-123']);
  assert.equal(calls[0][0], 'add_download');
  assert.equal(JSON.stringify(calls[0][1]), JSON.stringify({ request }));
});

test('download actions invoke the typed Rust command', async () => {
  const calls = [];
  const api = loadApi({
    core: {
      invoke: async (...args) => {
        calls.push(args);
        return null;
      }
    }
  });
  const request = { id: 'download-123', action: 'resume' };

  await api.action(request);

  assert.equal(calls[0][0], 'download_action');
  assert.equal(JSON.stringify(calls[0][1]), JSON.stringify(request));
});

test('folder picker uses the restricted native directory dialog', async () => {
  const calls = [];
  const api = loadApi({
    dialog: {
      open: async options => {
        calls.push(options);
        return '/downloads';
      }
    }
  });

  assert.equal(await api.chooseDirectory(), '/downloads');
  assert.equal(
    JSON.stringify(calls[0]),
    JSON.stringify({ directory: true, multiple: false })
  );
});
