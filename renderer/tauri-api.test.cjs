'use strict';

const assert = require('node:assert');
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
  const patch = {
    maxConcurrent: 4,
    capturePolicy: {
      enabled: true,
      confirmationMode: 'askForUnknown',
      fileTypes: ['zip', 'exe'],
      excludedHosts: ['example.com'],
      excludedAddresses: ['192.168.1.1']
    }
  };

  const updated = await api.settings(patch);

  assert.deepEqual(updated, { maxConcurrent: 4 });
  assert.equal(calls[0][0], 'update_settings');
  assert.equal(JSON.stringify(calls[0][1]), JSON.stringify({ patch }));
});

test('testProxy invokes test_proxy_connection with proxy configuration', async () => {
  const calls = [];
  const api = loadApi({
    core: {
      invoke: async (...args) => {
        calls.push(args);
        return { success: true, message: 'Proxy connection successful (HTTP 204)' };
      }
    }
  });
  const proxy = {
    enabled: true,
    protocol: 'http',
    host: '127.0.0.1',
    port: 8080,
    username: 'user',
    password: 'password'
  };

  const result = await api.testProxy({ proxy, testUrl: 'http://127.0.0.1:9090' });

  assert.equal(calls[0][0], 'test_proxy_connection');
  assert.deepEqual(calls[0][1], { proxy, testUrl: 'http://127.0.0.1:9090' });
  assert.equal(result.success, true);
  assert.equal(result.message, 'Proxy connection successful (HTTP 204)');
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

  for (const action of ['resume', 'pause', 'retry', 'cancel', 'remove', 'open', 'reveal', 'openWith']) {
    const request = { id: 'download-123', action };
    await api.action(request);
  }

  assert.equal(calls.length, 8);
  assert.equal(calls[0][0], 'download_action');
  assert.equal(calls[0][1].action, 'resume');
  assert.equal(calls[4][1].action, 'remove');
  assert.equal(calls[5][1].action, 'open');
  assert.equal(calls[6][1].action, 'reveal');
  assert.equal(calls[7][1].action, 'openWith');
});

test('removeCompleted invokes the remove_completed Rust command', async () => {
  const calls = [];
  const api = loadApi({
    core: {
      invoke: async (...args) => {
        calls.push(args);
        return ['completed-1', 'completed-2'];
      }
    }
  });

  const result = await api.removeCompleted();
  assert.deepEqual(result, ['completed-1', 'completed-2']);
  assert.equal(calls[0][0], 'remove_completed');
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

test('review invokes review_download Rust command with request', async () => {
  const calls = [];
  const api = loadApi({
    core: {
      invoke: async (...args) => {
        calls.push(args);
        return ['download-123'];
      }
    }
  });

  const request = { id: 'download-123', filename: 'confirmed.iso', queueOnly: false };
  const result = await api.review(request);

  assert.deepEqual(result, ['download-123']);
  assert.equal(calls[0][0], 'review_download');
  assert.equal(JSON.stringify(calls[0][1]), JSON.stringify({ request }));
});

test('checkDestinationConflict invokes check_destination_conflict Rust command', async () => {
  const calls = [];
  const api = loadApi({
    core: {
      invoke: async (...args) => {
        calls.push(args);
        return { type: 'resolved', path: '/downloads/test.zip', policy: 'overwrite' };
      }
    }
  });

  const result = await api.checkDestinationConflict({ directory: '/downloads', filename: 'test.zip' });
  assert.deepEqual(result, { type: 'resolved', path: '/downloads/test.zip', policy: 'overwrite' });
  assert.equal(calls[0][0], 'check_destination_conflict');
  assert.equal(JSON.stringify(calls[0][1]), JSON.stringify({ directory: '/downloads', filename: 'test.zip' }));
});

test('integration invokes get_integration_info Rust command', async () => {
  const calls = [];
  const api = loadApi({
    core: {
      invoke: async (...args) => {
        calls.push(args);
        return { port: 12345, browsers: [] };
      }
    }
  });

  const result = await api.integration();
  assert.deepEqual(result, { port: 12345, browsers: [] });
  assert.equal(calls[0][0], 'get_integration_info');
});

test('browserSetup invokes browser_setup_action Rust command', async () => {
  const calls = [];
  const api = loadApi({
    core: {
      invoke: async (...args) => {
        calls.push(args);
        return { browser: 'Chrome' };
      }
    }
  });

  const result = await api.browserSetup({ action: 'copy', browser: 'Chrome' });
  assert.deepEqual(result, { browser: 'Chrome' });
  assert.equal(calls[0][0], 'browser_setup_action');
  assert.equal(JSON.stringify(calls[0][1]), JSON.stringify({ action: 'copy', browser: 'Chrome' }));
});

test('registerBrowser invokes register_browser Rust command', async () => {
  const calls = [];
  const api = loadApi({
    core: {
      invoke: async (...args) => {
        calls.push(args);
        return null;
      }
    }
  });

  await api.registerBrowser({ browser: 'Chrome', extensionId: 'abcdefghijklmnopabcdefghijklmnop' });
  assert.equal(calls[0][0], 'register_browser');
  assert.equal(JSON.stringify(calls[0][1]), JSON.stringify({ browser: 'Chrome', extensionId: 'abcdefghijklmnopabcdefghijklmnop' }));
});

test('auxiliary window and dialog methods invoke corresponding Rust commands', async () => {
  const calls = [];
  const api = loadApi({
    core: {
      invoke: async (...args) => {
        calls.push(args);
        return null;
      }
    }
  });

  await api.openIntegration();
  await api.openOptions();
  await api.openAdd({ paste: true, scheduled: '2026-12-01T00:00:00Z' });
  await api.openScheduler();
  await api.exportDownloads();
  await api.importDownloads();
  await api.showAbout();

  assert.equal(calls.length, 7);
  assert.equal(calls[0][0], 'open_integration_window');
  assert.equal(calls[1][0], 'open_options_window');
  assert.equal(calls[2][0], 'open_add_window');
  assert.equal(JSON.stringify(calls[2][1]), JSON.stringify({ options: { paste: true, scheduled: '2026-12-01T00:00:00Z' } }));
  assert.equal(calls[3][0], 'open_scheduler_window');
  assert.equal(calls[4][0], 'export_downloads_to_file');
  assert.equal(calls[5][0], 'import_downloads_from_file');
  assert.equal(calls[6][0], 'show_about_dialog');
});

test('playlist methods invoke get_playlist_info and add_playlist_downloads', async () => {
  const calls = [];
  const api = loadApi({
    core: {
      invoke: async (...args) => {
        calls.push(args);
        if (args[0] === 'get_playlist_info') {
          return { id: 'PL12345', title: 'Test Playlist', entries: [] };
        }
        if (args[0] === 'add_playlist_downloads') {
          return 5;
        }
        return null;
      }
    }
  });

  const info = await api.getPlaylistInfo('https://www.youtube.com/playlist?list=PL12345');
  assert.equal(info.id, 'PL12345');
  assert.equal(calls[0][0], 'get_playlist_info');
  assert.deepEqual(calls[0][1], { url: 'https://www.youtube.com/playlist?list=PL12345' });

  const added = await api.addPlaylistDownloads({
    playlistId: 'PL12345',
    playlistTitle: 'Test Playlist',
    items: [{ id: 'abc', url: 'https://youtube.com/watch?v=abc', title: 'Video', selected: true }]
  });
  assert.equal(added, 5);
  assert.equal(calls[1][0], 'add_playlist_downloads');
  assert.deepEqual(calls[1][1], {
    request: {
      playlistId: 'PL12345',
      playlistTitle: 'Test Playlist',
      items: [{ id: 'abc', url: 'https://youtube.com/watch?v=abc', title: 'Video', selected: true }]
    }
  });
});

test('media tools methods invoke get_media_tools_status and install_media_tool', async () => {
  const calls = [];
  const api = loadApi({
    core: {
      invoke: async (...args) => {
        calls.push(args);
        if (args[0] === 'get_media_tools_status') {
          return [
            { toolType: 'ffmpeg', isAvailable: true, path: '/usr/local/bin/ffmpeg', source: 'system-path' },
            { toolType: 'yt-dlp', isAvailable: false, path: null, source: null }
          ];
        }
        if (args[0] === 'install_media_tool') {
          return { toolType: args[1].tool, isAvailable: true, path: '/home/user/.open-download-manager/tools/' + args[1].tool, source: 'managed-user-data' };
        }
        return null;
      }
    }
  });

  const statuses = await api.getMediaToolsStatus();
  assert.equal(statuses.length, 2);
  assert.equal(statuses[0].toolType, 'ffmpeg');
  assert.equal(statuses[0].isAvailable, true);
  assert.equal(calls[0][0], 'get_media_tools_status');

  const installed = await api.installMediaTool('yt-dlp');
  assert.equal(installed.toolType, 'yt-dlp');
  assert.equal(installed.isAvailable, true);
  assert.equal(calls[1][0], 'install_media_tool');
  assert.deepEqual(calls[1][1], { tool: 'yt-dlp' });
});

test('credential rules methods invoke corresponding Rust commands', async () => {
  const calls = [];
  const api = loadApi({
    core: {
      invoke: async (...args) => {
        calls.push(args);
        if (args[0] === 'list_credential_rules') {
          return [
            { id: 'rule-1', scheme: 'https', host: 'site.test', pathPrefix: '/team/', username: 'alice', secret: '******' }
          ];
        }
        if (args[0] === 'save_credential_rule') {
          return { id: 'rule-2', ...args[1].rule, secret: '******' };
        }
        return null;
      }
    }
  });

  const rules = await api.listCredentialRules();
  assert.equal(rules.length, 1);
  assert.equal(rules[0].host, 'site.test');
  assert.equal(calls[0][0], 'list_credential_rules');

  const saved = await api.saveCredentialRule({
    scheme: 'https',
    host: 'site2.test',
    pathPrefix: '/',
    username: 'bob',
    secret: 'password123'
  });
  assert.equal(saved.username, 'bob');
  assert.equal(calls[1][0], 'save_credential_rule');
  assert.equal(calls[1][1].rule.username, 'bob');

  await api.deleteCredentialRule('rule-1');
  assert.equal(calls[2][0], 'delete_credential_rule');
  assert.deepEqual(calls[2][1], { id: 'rule-1' });
});
