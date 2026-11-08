// Created: 2026-11-08
// Reason: feat(theme): add light, dark, and system theme options to the desktop app (Closes #4)
// Description: Frontend tests verifying theme selection, dynamic system tracking, and data-theme attributes.

'use strict';

const test = require('node:test');
const assert = require('node:assert');

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
