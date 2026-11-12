// Created: 2026-11-12
// Reason: test(torrent): prove the Torrents window boots without a runtime error
// Description: Loads renderer/torrents.js against a small DOM built from
// renderer/torrents.html. A missing state member or a function that was never
// declared only shows up when the script runs, so the window is started here
// the same way the app starts it.

"use strict";

const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const test = require("node:test");
const vm = require("node:vm");

const RENDERER_DIR = __dirname;

// ---------------------------------------------------------------------------
// A small DOM
// ---------------------------------------------------------------------------

class Node {
	constructor(tag) {
		this.tagName = String(tag || "div").toUpperCase();
		this.attributes = {};
		this.children = [];
		this.parent = null;
		this.dataset = {};
		this.style = {};
		this.hidden = false;
		this.disabled = false;
		this.checked = false;
		this.readOnly = false;
		this.value = "";
		this.textContent = "";
		this.title = "";
		this.className = "";
		this.innerHTML = "";
		this.type = "";
		this.options = [];
		this.classList = {
			add: () => {},
			remove: () => {},
			toggle: () => {},
			contains: name => this.className.split(/\s+/).includes(name),
		};
	}

	append(...nodes) {
		for (const node of nodes) {
			node.parent = this;
			this.children.push(node);
		}
	}

	replaceChildren(...nodes) {
		this.children = [];
		this.append(...nodes);
	}

	addEventListener(type, handler) {
		(this.listeners ||= {})[type] ||= [];
		this.listeners[type].push(handler);
	}
	removeEventListener() {}
	dispatch(type, event = {}) {
		for (const handler of (this.listeners && this.listeners[type]) || []) handler(event);
	}
	setAttribute(name, value) {
		this.attributes[name] = value;
	}
	getAttribute(name) {
		return this.attributes[name] ?? null;
	}
	remove() {
		if (!this.parent) return;
		this.parent.children = this.parent.children.filter(child => child !== this);
	}
	focus() {}
	showModal() {}
	close() {}
	closest() {
		return null;
	}
	getBoundingClientRect() {
		return { top: 0, left: 0, width: 0, height: 0 };
	}

	descendants() {
		const all = [];
		for (const child of this.children) {
			all.push(child);
			all.push(...child.descendants());
		}
		return all;
	}

	querySelector(selector) {
		return this.querySelectorAll(selector)[0] || null;
	}

	querySelectorAll(selector) {
		return matchAll(this.descendants(), selector);
	}
}

// Matches `#id`, `.class`, `tag`, `[attr]`, `[attr=value]` and descendant
// chains of those.
function matchCompound(node, compound) {
	const parts = compound.match(/[#.]?[\w-]+|\[[^\]]+\]/g) || [];
	for (const part of parts) {
		if (part.startsWith("#")) {
			if (node.attributes.id !== part.slice(1)) return false;
		} else if (part.startsWith(".")) {
			if (!node.className.split(/\s+/).includes(part.slice(1))) return false;
		} else if (part.startsWith("[")) {
			const [name, raw] = part.slice(1, -1).split("=");
			const value = raw === undefined ? undefined : raw.replace(/^["']|["']$/g, "");
			if (!(name in node.attributes)) return false;
			if (value !== undefined && node.attributes[name] !== value) return false;
		} else if (node.tagName !== part.toUpperCase()) {
			return false;
		}
	}
	return true;
}

function matchAll(nodes, selector) {
	const chain = String(selector).trim().split(/\s+/).filter(Boolean);
	if (!chain.length) return [];
	const last = chain[chain.length - 1];
	const ancestors = chain.slice(0, -1);
	const found = [];
	for (const node of nodes) {
		if (!matchCompound(node, last)) continue;
		let scope = node.parent;
		let ok = true;
		for (const compound of ancestors.reverse()) {
			let found2 = false;
			while (scope) {
				if (matchCompound(scope, compound)) {
					found2 = true;
					scope = scope.parent;
					break;
				}
				scope = scope.parent;
			}
			if (!found2) {
				ok = false;
				break;
			}
		}
		if (ok) found.push(node);
	}
	return found;
}

// Builds the tree of renderer/torrents.html.
function parseHtml(html) {
	const root = new Node("html");
	const stack = [root];
	const token = /<!--[\s\S]*?-->|<(\/)?([a-zA-Z][\w-]*)((?:\s+[\w:-]+(?:\s*=\s*(?:"[^"]*"|'[^']*'|[^\s>]+))?)*)\s*(\/)?>|([^<]+)/g;
	let match;
	while ((match = token.exec(html))) {
		const [raw, closing, tag, attributeText, selfClosing, text] = match;
		if (raw.startsWith("<!--")) continue;
		if (text !== undefined) {
			const parent = stack[stack.length - 1];
			parent.textContent += text.trim();
			continue;
		}
		if (closing) {
			if (stack.length > 1) stack.pop();
			continue;
		}
		const element = new Node(tag);
		for (const attribute of attributeText.matchAll(/([\w:-]+)(?:\s*=\s*(?:"([^"]*)"|'([^']*)'|([^\s>]+)))?/g)) {
			const name = attribute[1];
			const value = attribute[2] ?? attribute[3] ?? attribute[4] ?? "";
			element.attributes[name] = value;
			if (name === "id") element.attributes.id = value;
			if (name === "class") element.className = value;
			if (name.startsWith("data-")) {
				element.dataset[name.slice(5).replace(/-([a-z])/g, (_, c) => c.toUpperCase())] = value;
			}
		}
		const parent = stack[stack.length - 1];
		parent.append(element);
		if (!selfClosing && !["meta", "link", "br", "img", "input", "hr", "source"].includes(tag.toLowerCase())) {
			stack.push(element);
		}
	}
	return root;
}

// ---------------------------------------------------------------------------
// The window
// ---------------------------------------------------------------------------

function createEnvironment(html) {
	const root = parseHtml(html);
	const document = {
		documentElement: root,
		body: root.querySelector("body") || new Node("body"),
		title: "Torrents",
		hidden: false,
		visibilityState: "visible",
		createElement: tag => new Node(tag),
		createTextNode: text => {
			const node = new Node("#text");
			node.textContent = text;
			return node;
		},
		querySelector: selector => root.querySelector(selector),
		querySelectorAll: selector => root.querySelectorAll(selector),
		getElementById: id => root.querySelector(`#${id}`),
		addEventListener() {},
		removeEventListener() {},
	};

	const calls = [];
	const api = {
		torrentStatus: async () => ({
			available: true,
			enabled: true,
			state: "ready",
			message: "",
			listenPort: 6881,
			dhtEnabled: true,
			restartRequired: false,
		}),
		torrentList: async () => [
			{
				id: 1,
				name: "debian.iso",
				infoHash: "abc123",
				state: "live",
				finished: false,
				progressBytes: 512,
				totalBytes: 1024,
				downloadSpeed: 2048,
				uploadSpeed: 128,
				seeds: 3,
				leechers: 2,
				peers: 5,
				ratio: 0.5,
				timeRemainingSecs: 30,
				files: [],
			},
		],
		torrentSettingsGet: async () => ({ enabled: true }),
		torrentSettingsSet: async settings => settings,
		torrentTrackerStatus: async () => [],
		torrentFiles: async () => [
			{ index: 0, name: "debian.iso", length: 1024, progressBytes: 512, included: true },
		],
		torrentSetFiles: async () => {},
		torrentAction: async () => {},
		torrentAddMagnet: async () => ({ id: 1, name: "test", infoHash: "abc" }),
		torrentAddFile: async () => ({ id: 1, name: "test", infoHash: "abc" }),
		torrentSetWatching: async () => {},
		torrentRestartEngine: async () => {},
		torrentTakeReview: async () => null,
		onTorrentProgress: () => () => {},
		onTorrentEngine: () => () => {},
		onTorrentReview: () => () => {},
		chooseDirectory: async () => null,
	};
	for (const name of Object.keys(api)) {
		const original = api[name];
		api[name] = (...args) => {
			calls.push({ name, args });
			return original(...args);
		};
	}

	const window = {
		downloader: api,
		TorrentView: require("./torrents-view.js"),
		document,
		location: { search: "", href: "tauri://localhost/torrents.html" },
		navigator: { clipboard: { writeText: async () => {} } },
		localStorage: { getItem: () => null, setItem: () => {} },
		setTimeout: () => 0,
		clearTimeout: () => {},
		setInterval: () => 0,
		clearInterval: () => {},
		addEventListener() {},
		removeEventListener() {},
		matchMedia: () => ({ matches: false, addEventListener() {} }),
		MutationObserver: class {
			observe() {}
			disconnect() {}
		},
	};
	window.window = window;
	return { window, document, api, calls, root };
}

test("the Torrents window starts without a runtime error", async () => {
	const html = fs.readFileSync(path.join(RENDERER_DIR, "torrents.html"), "utf8");
	const source = fs.readFileSync(path.join(RENDERER_DIR, "torrents.js"), "utf8");
	const { window, document, calls, root } = createEnvironment(html);

	vm.runInNewContext(source, {
		window,
		document,
		location: window.location,
		navigator: window.navigator,
		localStorage: window.localStorage,
		MutationObserver: window.MutationObserver,
		matchMedia: window.matchMedia,
		URL,
		URLSearchParams,
		Number,
		Date,
		Math,
		JSON,
		Promise,
		Set,
		Map,
		Array,
		Object,
		String,
		Boolean,
		Infinity,
		console: { warn() {}, error() {}, log() {} },
		setTimeout: window.setTimeout,
		clearTimeout: window.clearTimeout,
		setInterval: window.setInterval,
		clearInterval: window.clearInterval,
	});

	// The script starts the window on load. Give the first round of promises a
	// chance to settle, so an error in the first refresh is caught too.
	await new Promise(resolve => setImmediate(resolve));
	await new Promise(resolve => setImmediate(resolve));

	assert.ok(
		calls.some(call => call.name === "torrentStatus"),
		"the window asks the engine for its status",
	);

	// Open the file list. The shipped crash happened here: the window kept
	// the info hash of every cached file list in a map that was never created.
	const filesTab = root.querySelector('[data-tab="files"]');
	assert.ok(filesTab, "the document has a files tab");
	filesTab.dispatch("click");
	await new Promise(resolve => setImmediate(resolve));
	await new Promise(resolve => setImmediate(resolve));

	const rows = root.querySelectorAll(".torrent-file-row");
	assert.equal(rows.length, 1, "the file list is drawn");
	assert.ok(
		calls.some(call => call.name === "torrentFiles"),
		"the window asks the engine for the files of the torrent",
	);
});

test("every element the Torrents window looks up exists in the document", () => {
	const html = fs.readFileSync(path.join(RENDERER_DIR, "torrents.html"), "utf8");
	const source = fs.readFileSync(path.join(RENDERER_DIR, "torrents.js"), "utf8");
	const { root } = createEnvironment(html);
	const missing = new Set();
	for (const match of source.matchAll(/\$\("#([A-Za-z0-9_-]+)"\)/g)) {
		if (!root.querySelector(`#${match[1]}`)) missing.add(match[1]);
	}
	assert.deepEqual([...missing], [], `missing elements: ${[...missing].join(", ")}`);
});

test("the Torrents window keeps its file list cache in a declared map", () => {
	// The shipped crash: the map that holds the info hash of every cached file
	// list was read and written but never created.
	const source = fs.readFileSync(path.join(RENDERER_DIR, "torrents.js"), "utf8");
	const state = source.slice(source.indexOf("const state = {"));
	const literal = state.slice(0, state.indexOf("\n};"));
	assert.match(literal, /filesHash:\s*new Map\(\)/);
});
