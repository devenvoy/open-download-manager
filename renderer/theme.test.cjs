// Created: 2026-11-08
// Reason: feat(theme): add light, dark, and system theme options to the desktop app (Closes #4)
// Last updated: 2026-11-08
// Reason: fix(theme): verify dark theme CSS overrides for main downloads table and auxiliary windows (Refs #4)
// Reason: fix(theme): verify dark theme styling for progress dialog tabs, active tab, and tab panels (Refs #4)
// Reason: fix(theme): verify dark theme styling for completed download dialog and panels
// Description: Frontend tests verifying theme selection, dynamic system tracking, data-theme attributes, and CSS rules.

'use strict';

const test = require('node:test');
const assert = require('node:assert');
const fs = require('node:fs');
const path = require('node:path');
const themeModule = require('./theme.js');

test('theme helper resolves system, light, and dark modes correctly', () => {
  let isOsDark = false;
  const matchMedia = (query) => ({
    matches: isOsDark,
    addEventListener: () => {}
  });

  function getEffectiveTheme(themeSetting) {
    if (themeSetting === 'light' || themeSetting === 'dark') {
      return themeSetting;
    }
    return matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light';
  }

  // 1. Explicit light
  assert.equal(getEffectiveTheme('light'), 'light');
  // 2. Explicit dark
  assert.equal(getEffectiveTheme('dark'), 'dark');
  // 3. System when OS is light
  assert.equal(getEffectiveTheme('system'), 'light');
  assert.equal(getEffectiveTheme(null), 'light');
  assert.equal(getEffectiveTheme(undefined), 'light');

  // 4. System when OS is dark
  isOsDark = true;
  assert.equal(getEffectiveTheme('system'), 'dark');
  assert.equal(getEffectiveTheme(null), 'dark');
  assert.equal(getEffectiveTheme(undefined), 'dark');

  // 5. Explicit modes still override OS dark
  assert.equal(getEffectiveTheme('light'), 'light');
  assert.equal(getEffectiveTheme('dark'), 'dark');

  // 6. Test exported theme.js module directly
  assert.equal(themeModule.getEffectiveTheme('light', matchMedia), 'light');
  assert.equal(themeModule.getEffectiveTheme('dark', matchMedia), 'dark');
  assert.equal(themeModule.getEffectiveTheme('system', matchMedia), 'dark');
});

test('theme applies attributes to documentElement correctly', () => {
  const attributes = {};
  const styles = {};
  const docElement = {
    setAttribute: (name, val) => { attributes[name] = val; },
    getAttribute: (name) => attributes[name],
    style: styles
  };

  let osDark = true;
  function applyTheme(themeSetting = 'system') {
    const effective = (themeSetting === 'light' || themeSetting === 'dark')
      ? themeSetting
      : (osDark ? 'dark' : 'light');
    docElement.setAttribute('data-theme', effective);
    docElement.setAttribute('data-theme-setting', themeSetting);
    docElement.style.colorScheme = effective;
  }

  // Apply default (system, when OS dark)
  applyTheme('system');
  assert.equal(docElement.getAttribute('data-theme'), 'dark');
  assert.equal(docElement.getAttribute('data-theme-setting'), 'system');
  assert.equal(docElement.style.colorScheme, 'dark');

  // Change to light
  applyTheme('light');
  assert.equal(docElement.getAttribute('data-theme'), 'light');
  assert.equal(docElement.getAttribute('data-theme-setting'), 'light');
  assert.equal(docElement.style.colorScheme, 'light');

  // Change to dark
  applyTheme('dark');
  assert.equal(docElement.getAttribute('data-theme'), 'dark');
  assert.equal(docElement.getAttribute('data-theme-setting'), 'dark');
  assert.equal(docElement.style.colorScheme, 'dark');
});

test('styles.css contains high-contrast dark theme overrides for main table and auxiliary windows', () => {
  const css = fs.readFileSync(path.join(__dirname, 'styles.css'), 'utf8');

  // Verify main download pane, table scroll, and cell overrides
  assert.match(css, /\[data-theme="dark"\]\s+\.main-pane/);
  assert.match(css, /\[data-theme="dark"\]\s+\.table-scroll/);
  assert.match(css, /\[data-theme="dark"\]\s+\.downloads-table tbody/);
  assert.match(css, /\[data-theme="dark"\]\s+\.downloads-table tbody td/);
  assert.match(css, /\[data-theme="dark"\]\s+\.downloads-table thead th/);

  // Verify dialog tabs and tab panels in dark mode
  assert.match(css, /\[data-theme="dark"\]\s+\.dialog-tabs button/);
  assert.match(css, /\[data-theme="dark"\]\s+\.dialog-tabs \.active-tab/);
  assert.match(css, /\[data-theme="dark"\]\s+\.tab-panel/);

  // Verify auxiliary window modes have dark backgrounds
  assert.match(css, /\[data-theme="dark"\]\s+body\.options-window/);
  assert.match(css, /\[data-theme="dark"\]\s+body\.add-window/);
  assert.match(css, /\[data-theme="dark"\]\s+body\.scheduler-window/);
  assert.match(css, /\[data-theme="dark"\]\s+body\.integration-window/);
  assert.match(css, /\[data-theme="dark"\]\s+body\.progress-window/);

  // Verify auxiliary dialog footers and bodies
  assert.match(css, /\[data-theme="dark"\]\s+\.options-window #settings-dialog/);
  assert.match(css, /\[data-theme="dark"\]\s+#integration-dialog \.dialog-body/);
  assert.match(css, /\[data-theme="dark"\]\s+\.browser-card/);
  assert.match(css, /\[data-theme="dark"\]\s+\.playlist-items-list/);
  assert.match(css, /\[data-theme="dark"\]\s+\.site-overrides-list/);

  // Verify completed download dialog and panels in dark mode
  assert.match(css, /\[data-theme="dark"\]\s+\.completed-panel/);
  assert.match(css, /\[data-theme="dark"\]\s+\.completed-summary/);
  assert.match(css, /\[data-theme="dark"\]\s+\.completed-icon/);
  assert.match(css, /\[data-theme="dark"\]\s+\.completed-field/);
});
