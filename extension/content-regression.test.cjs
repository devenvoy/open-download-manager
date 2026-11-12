// Created: 2026-11-06
// Reason: extension: keep raw URLs out of quality-row tooltips and add #6 regression coverage
// Last updated: 2026-11-07
// Reason: feat(media): modularize download executors, enable native multi-track streaming, and fix progress UX
// Description: Regression tests verifying multi-video panels, SPA navigations, shadow DOM discovery, and keyboard focus.

// Regression coverage for issue #6: multi-video pages, nested scrolling, SPA
// video changes, chooser keyboard interaction and tooltip sanitization,
// driven through the real content.js with a stubbed extension boundary.
const test = require('node:test');
const assert = require('node:assert/strict');
const vm = require('node:vm');
const fs = require('node:fs');
const path = require('node:path');

const LIBS = ['media-choices.js', 'menu-behavior.js', 'safe-tooltip.js', 'panel-position.js', 'chooser-keyboard.js', 'format-presentation.js'];

function makeEl(tag) {
  const el = {
    tagName: tag, className: '', textContent: '', title: '', type: '', disabled: false,
    style: {}, children: [], listeners: {}, removed: false, focused: false,
    parentNode: null, offsetWidth: 180, offsetHeight: 27,
    appendChild(child) { this.children.push(child); child.parentNode = this; },
    append(child) { this.appendChild(child); },
    remove() {
      this.removed = true;
      if (this.parentNode) {
        const index = this.parentNode.children.indexOf(this);
        if (index >= 0) this.parentNode.children.splice(index, 1);
        this.parentNode = null;
      }
    },
    addEventListener(name, fn) { (this.listeners[name] ||= []).push(fn); },
    dispatch(name, event = {}) {
      event.target ??= this;
      event.preventDefault ??= () => { event.defaultPrevented = true; };
      event.stopPropagation ??= () => {};
      for (const fn of this.listeners[name] || []) fn(event);
      if (!event.stopped && this.parentNode && this.parentNode.dispatch) this.parentNode.dispatch(name, event);
    },
    focus() { this.focused = true; el.ownerHarness.activeElement = this; },
    click() { if (this.onclick) this.onclick({ preventDefault() {}, stopPropagation() {} }); },
    scrollIntoView() {},
    getBoundingClientRect() { return { top: 50, bottom: 80, left: 0, right: 180, width: 180, height: 30 }; },
    querySelector(selector) {
      if (selector === '.chooser') return this.children.find(child => child.className === 'chooser') || null;
      return null;
    },
    querySelectorAll(selector) {
      if (selector !== '.row') return [];
      const found = [];
      const walk = node => { for (const child of node.children) { if (child.className === 'row') found.push(child); walk(child); } };
      walk(this);
      return found;
    },
  };
  if (tag === 'div') {
    el.attachShadow = () => {
      el.isHost = true;
      el.ownerHarness.hosts.push(el);
      const shadow = makeEl('shadow');
      shadow.download = makeEl('button');
      shadow.dismiss = makeEl('button');
      shadow.querySelector = selector => {
        if (selector === '.download') return shadow.download;
        if (selector === '.dismiss') return shadow.dismiss;
        if (selector === '.chooser') return shadow.children.find(child => child.className === 'chooser') || null;
        return null;
      };
      Object.defineProperty(shadow, 'activeElement', { get: () => el.ownerHarness.activeElement });
      shadow.ownerHarness = el.ownerHarness;
      shadow.download.ownerHarness = el.ownerHarness;
      shadow.dismiss.ownerHarness = el.ownerHarness;
      el.shadow = shadow;
      return shadow;
    };
  }
  return el;
}

function makeVideo(rect, harness) {
  return {
    currentSrc: 'https://media.example.com/video.mp4', src: '', poster: '',
    isConnected: true,
    querySelectorAll() { return []; },
    addEventListener() {},
    getBoundingClientRect() { return this.rect; },
    rect,
  };
}

function harness(videos, listResponse) {
  const h = { activeElement: null, hosts: [], videos, shadowHosts: [], rafQueue: [] };
  const runtime = {
    id: 'test-extension',
    sendMessage(message) {
      h.sent = h.sent || [];
      h.sent.push(message.action);
      h.messages = h.messages || [];
      h.messages.push(message);
      if (message.action === 'listForFrame') return Promise.resolve(listResponse);
      return Promise.resolve({ ok: true });
    },
    onMessage: { addListener() {}, removeListener() {} },
  };
  class Observer {
    constructor(fn) { this.fn = fn; h.observers = h.observers || []; h.observers.push(this); }
    observe(target, options) { this.target = target; this.options = options; }
    unobserve() {}
    disconnect() { this.disconnected = true; }
  }
  class ResizeObserverStub extends Observer {}
  class MutationObserverStub extends Observer {}
  class IntersectionObserverStub extends Observer {}
  const documentElement = makeEl('html');
  const documentListeners = {};
  const windowListeners = {};
  const document = {
    title: 'Videos',
    documentElement,
    fullscreenElement: null,
    querySelectorAll(selector) {
      if (selector === 'video') return h.videos;
      if (selector === '*') return h.shadowHosts;
      return [];
    },
    createElement(tag) { const el = makeEl(tag); el.ownerHarness = h; return el; },
    addEventListener(name, fn) { (documentListeners[name] ||= []).push(fn); },
  };
  const context = {
    chrome: { runtime }, document,
    location: { href: 'https://videos.example.com/page', hostname: 'videos.example.com' },
    URL, AbortController, ResizeObserver: ResizeObserverStub, MutationObserver: MutationObserverStub, IntersectionObserver: IntersectionObserverStub,
    addEventListener(name, fn) { (windowListeners[name] ||= []).push(fn); },
    removeEventListener() {},
    requestAnimationFrame(fn) { h.rafQueue.push(fn); return h.rafQueue.length; },
    cancelAnimationFrame() {},
    setTimeout, clearTimeout,
    innerHeight: 800, innerWidth: 1200,
  };
  vm.createContext(context);
  for (const lib of LIBS) vm.runInContext(fs.readFileSync(path.join(__dirname, lib), 'utf8'), context);
  vm.runInContext(fs.readFileSync(path.join(__dirname, 'content.js'), 'utf8'), context);
  h.context = context;
  h.documentListeners = documentListeners;
  h.windowListeners = windowListeners;
  // The document-level mutation observer watches documentElement with subtree.
  h.mutationObserver = h.observers.find(observer => observer instanceof MutationObserverStub && observer.options && observer.options.subtree);
  h.flushFrame = () => { for (const fn of h.rafQueue.splice(0)) fn(); };
  h.triggerMutation = () => { h.mutationObserver.fn(); h.flushFrame(); };
  h.scroll = event => { for (const fn of windowListeners.scroll || []) fn(event); };
  h.resize = () => { for (const fn of windowListeners.resize || []) fn(); };
  return h;
}

const RECT = { top: 50, bottom: 400, left: 0, right: 600, width: 600, height: 350 };
const ITEMS = { ok: true, items: [
  { url: 'https://media.example.com/video-1080.mp4', title: 'Clip', quality: '1080p', bitrate: 4500000, format: 'MP4', codecs: 'avc1.640028, mp4a.40.2', mime: 'video/mp4', reason: '' },
  { url: 'https://media.example.com/video-720.mp4', title: 'Clip', quality: '720p', bitrate: 2500000, format: 'MP4', codecs: 'avc1.4d401f, mp4a.40.2', mime: 'video/mp4', reason: '' },
] };

test('each video on a multi-video page gets its own panel and dismissal', async () => {
  const h = harness([makeVideo({ ...RECT }, ), makeVideo({ ...RECT, top: 500, bottom: 850 })], ITEMS);
  assert.equal(h.hosts.length, 2);
  const [first, second] = h.hosts;
  first.shadow.dismiss.dispatch('click');
  assert.equal(first.style.display, 'none');
  h.resize();
  assert.equal(first.style.display, 'none', 'dismissed panel stays hidden');
  assert.equal(second.style.display, 'block', 'other video keeps its panel');
});

test('a panel follows a video scrolled partly out of view instead of vanishing', () => {
  const video = makeVideo({ ...RECT });
  const h = harness([video], ITEMS);
  const host = h.hosts[0];
  assert.equal(host.style.display, 'block');
  video.rect = { ...RECT, top: -200, bottom: 150 };
  h.resize();
  assert.equal(host.style.display, 'block', 'still partly visible');
  assert.equal(host.style.top, '5px', 'pinned inside the viewport top');
  video.rect = { ...RECT, top: -340, bottom: 10 };
  h.resize();
  assert.equal(host.style.display, 'none', 'hidden once effectively out of view');
});

test('SPA navigation removes the panel of a detached video and panels new videos', () => {
  const first = makeVideo({ ...RECT });
  const h = harness([first], ITEMS);
  assert.equal(h.hosts.length, 1);
  first.isConnected = false;
  h.resize();
  assert.equal(h.hosts[0].removed, true, 'detached video panel is removed');
  const second = makeVideo({ ...RECT });
  h.videos.push(second);
  h.triggerMutation();
  assert.equal(h.hosts.length, 2, 'scan picks up the new video');
  assert.equal(h.hosts[1].style.display, 'block');
});

test('videos in open shadow roots are discovered, including after dynamic insertion', () => {
  const h = harness([], ITEMS);
  const shadowRoot = {
    videos: [],
    querySelectorAll(selector) {
      if (selector === 'video') return this.videos;
      return [];
    },
  };
  h.shadowHosts.push({ shadowRoot });
  h.triggerMutation();
  assert.equal(h.hosts.length, 0, 'empty shadow root has no player panel');

  shadowRoot.videos.push(makeVideo({ ...RECT }));
  const observer = h.observers.find(value => value.target === shadowRoot);
  assert.ok(observer, 'shadow root is observed for later changes');
  observer.fn();
  h.flushFrame();
  assert.equal(h.hosts.length, 1, 'new shadow-root video gets a player panel');
});

test('unverified media links trigger extractor discovery and remain available on fallback', async () => {
  const video = makeVideo({ ...RECT });
  const h = harness([video], {
    ok: true,
    items: [{ url: video.currentSrc, title: 'Clip', media: true }],
  });
  const host = h.hosts[0];
  await host.shadow.download.listeners.click[0]({ preventDefault() {}, stopPropagation() {} });
  await new Promise(resolve => setImmediate(resolve));
  assert.ok(h.sent.includes('extractFormats'), 'generic extractor is queried for unverified media');
  assert.equal(host.shadow.querySelector('.chooser').querySelectorAll('.row').length, 1);
  assert.match(host.shadow.querySelector('.chooser').children[0].textContent, /showing detected media links/i);
});

test('scrolling inside the chooser keeps it open; page scrolling closes it', async () => {
  const video = makeVideo({ ...RECT });
  const h = harness([video], ITEMS);
  const host = h.hosts[0];
  await host.shadow.download.listeners.click[0]({ preventDefault() {}, stopPropagation() {} });
  await new Promise(resolve => setImmediate(resolve));
  const chooser = () => host.shadow.querySelector('.chooser');
  assert.ok(chooser(), 'chooser opened');
  h.scroll({ composedPath: () => [host] });
  assert.ok(chooser(), 'scroll inside the panel does not close the chooser');
  h.scroll({ composedPath: () => [] });
  assert.equal(chooser(), null, 'page scroll dismisses the chooser');
});

test('quality menu supports full keyboard navigation through the real content script', async () => {
  const video = makeVideo({ ...RECT });
  const h = harness([video], ITEMS);
  const host = h.hosts[0];
  await host.shadow.download.listeners.click[0]({ preventDefault() {}, stopPropagation() {} });
  await new Promise(resolve => setImmediate(resolve));
  const chooser = host.shadow.querySelector('.chooser');
  const rows = chooser.querySelectorAll('.row');
  assert.ok(rows.length >= 2, 'menu rows rendered');
  assert.equal(rows[0].focused, true, 'first row focused on open');
  rows[0].dispatch('keydown', { key: 'ArrowDown' });
  assert.equal(rows[1].focused, true, 'ArrowDown moves focus');
  rows[1].dispatch('keydown', { key: 'ArrowUp' });
  assert.equal(h.activeElement, rows[0]);
  rows[0].dispatch('keydown', { key: 'End' });
  assert.equal(h.activeElement, rows[rows.length - 1]);
  rows[rows.length - 1].dispatch('keydown', { key: 'Escape' });
  assert.equal(host.shadow.querySelector('.chooser'), null, 'Escape closes the menu');
  assert.equal(h.activeElement, host.shadow.download, 'focus returns to the download button');
});

test('signed URLs never reach row tooltips', async () => {
  const video = makeVideo({ ...RECT });
  const signed = { ...ITEMS.items[0], reason: 'May expire: https://media.example.com/video-1080.mp4?sig=secret&expires=1' };
  const h = harness([video], { ok: true, items: [signed, ITEMS.items[1]] });
  const host = h.hosts[0];
  await host.shadow.download.listeners.click[0]({ preventDefault() {}, stopPropagation() {} });
  await new Promise(resolve => setImmediate(resolve));
  for (const row of host.shadow.querySelector('.chooser').querySelectorAll('.row')) {
    assert.ok(!row.title.includes('sig=secret'), `tooltip leaked a signed URL: ${row.title}`);
    assert.ok(!/https?:\/\//.test(row.title), `tooltip contains a raw URL: ${row.title}`);
  }
});

test('magnet link clicks ask before opening the torrent window', async () => {
  const h = harness([], ITEMS);
  const magnet = 'magnet:?xt=urn:btih:cab507494d02ebb1178b38f2e9d7be299c86b862&dn=ubuntu.iso';
  const anchor = { closest: () => anchor, getAttribute: () => magnet };
  let confirmations = 0, prevented = 0;
  h.context.confirm = () => { confirmations += 1; return true; };
  const click = { target: anchor, button: 0, preventDefault() { prevented += 1; }, stopPropagation() {} };
  for (const fn of h.documentListeners.click || []) fn(click);
  await new Promise(resolve => setImmediate(resolve));
  assert.equal(confirmations, 1, 'magnet clicks are confirmed once');
  assert.equal(prevented, 1, 'the browser navigation is blocked after confirmation');
  assert.equal(h.messages.at(-1).action, 'torrent');
  assert.equal(h.messages.at(-1).magnet, magnet);
});

test('declining the magnet confirmation leaves the browser alone', async () => {
  const h = harness([], ITEMS);
  const anchor = { closest: () => anchor, getAttribute: () => 'magnet:?xt=urn:btih:cab507494d02ebb1178b38f2e9d7be299c86b862' };
  let prevented = 0;
  h.context.confirm = () => false;
  const before = (h.sent || []).length;
  for (const fn of h.documentListeners.click || []) fn({ target: anchor, button: 0, preventDefault() { prevented += 1; }, stopPropagation() {} });
  await new Promise(resolve => setImmediate(resolve));
  assert.equal(prevented, 1, 'navigation is blocked while the user decides');
  assert.equal((h.sent || []).length, before, 'nothing is sent without confirmation');
});
