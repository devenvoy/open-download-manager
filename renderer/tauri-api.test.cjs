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

test('unmigrated functionality rejects explicitly', async () => {
  const api = loadApi();

  await assert.rejects(api.add(), error => {
    assert.equal(error.code, 'feature_not_migrated');
    assert.match(error.message, /Downloads/);
    return true;
  });
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
