// Created: 2026-11-11
// Reason: feat(torrent): dedicated Torrents window UI
// Description: Controller for the standalone Torrents window. Talks only to the
// torrent IPC commands and never touches download history or queue state.

"use strict";

const $ = selector => document.querySelector(selector);
const $$ = selector => [...document.querySelectorAll(selector)];
const view = window.TorrentView;
const api = window.downloader;

if (typeof window.initTheme === "function") window.initTheme();

const state = {
	status: null,
	settings: null,
	torrents: [],
	selectedId: null,
	tab: "general",
	fileSelection: null,
	filesTorrentId: null,
	// The info hash the file selection belongs to. The engine reuses
	// torrent ids, so the id alone cannot identify a download.
	filesTorrentHash: null,
	filesDirty: false,
	files: new Map(),
	filesLoading: new Map(),
	filesFetchedAt: new Map(),
	// What the trackers report about the swarm of a torrent.
	swarm: new Map(),
	swarmLoading: new Map(),
	swarmFetchedAt: new Map(),
	swarmHash: new Map(),
	// The info hash each torrent id currently stands for. The engine reuses
	// ids after a torrent is deleted, so the id alone cannot identify a
	// download.
	torrentHash: new Map(),
	review: null,
	// Info hashes of the torrents whose review the user already answered. A
	// late review for one of them must not bring the confirmation back.
	answered: new Set(),
	refreshing: false,
	toastTimer: null,
};

// How often the per-file list is read from the engine. The progress event
// arrives every second, and every read costs a round trip to the engine.
const FILES_REFRESH_INTERVAL = 2000;

// The peer limit a torrent runs with. Only connected peers can send pieces,
// so a small limit slows a healthy swarm down.
const DEFAULT_MAX_PEERS = 200;

// How often the tracker numbers are read. They change with every announce.
const SWARM_REFRESH_INTERVAL = 5000;

function element(tag, className, text) {
	const node = document.createElement(tag);
	if (className) node.className = className;
	if (text !== undefined) node.textContent = text;
	return node;
}

function formatBytes(value) {
	return typeof window.bytes === "function"
		? window.bytes(value)
		: `${Number(value) || 0} B`;
}

function formatSpeed(value) {
	return `${formatBytes(value)}/s`;
}

function formatAdded(value) {
	const seconds = Number(value);
	if (!Number.isFinite(seconds) || seconds <= 0) return "—";
	const timestamp = new Date(seconds * 1000);
	return typeof window.date === "function"
		? window.date(timestamp)
		: timestamp.toLocaleString();
}

function notify(message, isError = false) {
	const toast = $("#torrent-toast");
	toast.textContent = message;
	toast.dataset.error = isError ? "true" : "false";
	toast.hidden = false;
	window.clearTimeout(state.toastTimer);
	if (isError) {
		// An error stays on screen until it is dismissed, so a failed action
		// cannot pass unnoticed.
		toast.title = "Click to dismiss";
		return;
	}
	state.toastTimer = window.setTimeout(() => {
		toast.hidden = true;
	}, 6000);
}

function showDialog(selector) {
	const dialog = $(selector);
	if (dialog && !dialog.open) dialog.showModal();
}

function closeDialog(selector) {
	const dialog = $(selector);
	if (dialog?.open) dialog.close();
}

function selectedTorrent() {
	return state.torrents.find(torrent => torrent.id === state.selectedId) || null;
}

// ---------------------------------------------------------------------------
// Engine state
// ---------------------------------------------------------------------------

function renderBanner() {
	const banner = $("#torrent-banner");
	const text = $("#torrent-banner-text");
	const action = $("#torrent-banner-action");
	const status = state.status;

	action.hidden = true;
	action.onclick = null;
	banner.dataset.kind = "info";

	if (!status) {
		banner.hidden = true;
		return;
	}
	if (!status.available) {
		banner.hidden = false;
		banner.dataset.kind = "error";
		text.textContent =
			"This build does not include BitTorrent support. Rebuild with --features torrent.";
		return;
	}
	if (!status.enabled) {
		banner.hidden = false;
		banner.dataset.kind = "warning";
		text.textContent = "Torrent support is disabled.";
		action.hidden = false;
		action.textContent = "Enable torrents";
		action.onclick = enableTorrents;
		return;
	}
	if (status.state === "starting") {
		banner.hidden = false;
		text.textContent = "Starting the torrent engine…";
		return;
	}
	if (status.state === "failed") {
		banner.hidden = false;
		banner.dataset.kind = "error";
		text.textContent = status.message || "The torrent engine failed to start.";
		action.hidden = false;
		action.textContent = "Retry";
		action.onclick = restartEngine;
		return;
	}
	if (status.state === "stopped") {
		banner.hidden = false;
		banner.dataset.kind = "warning";
		text.textContent = "The torrent engine is stopped.";
		action.hidden = false;
		action.textContent = "Start engine";
		action.onclick = restartEngine;
		return;
	}
	if (status.state === "ready" && status.restartRequired) {
		banner.hidden = false;
		banner.dataset.kind = "warning";
		text.textContent =
			"Settings changed. Restart the engine to apply them.";
		action.hidden = false;
		action.textContent = "Restart";
		action.onclick = restartEngine;
		return;
	}
	banner.hidden = true;
}

function renderToolbar() {
	const status = state.status;
	const engineReady = Boolean(
		status?.available && status?.enabled && status?.state === "ready",
	);
	const actions = view.availableActions(selectedTorrent());

	$("#torrent-add-magnet").disabled = !engineReady;
	$("#torrent-add-file").disabled = !engineReady;
	$("#torrent-settings").disabled = !status?.available;
	$("#torrent-pause").disabled = !engineReady || !actions.pause;
	$("#torrent-resume").disabled = !engineReady || !actions.resume;
	$("#torrent-recheck").disabled = !engineReady || !actions.recheck;
	$("#torrent-remove").disabled = !engineReady || !actions.forget;
}

function renderStatus() {
	renderBanner();
	renderToolbar();
	renderEmptyState();
	renderEngineInfo();
	renderSummary();
}

function renderEngineInfo() {
	const status = state.status;
	const info = $("#torrent-engine-info");
	const details = $("#torrent-settings-engine-info");
	const parts = [];
	if (status) {
		if (!status.available) {
			parts.push("BitTorrent support is not available in this build.");
		} else if (!status.enabled) {
			parts.push("BitTorrent support is switched off.");
		} else if (status.state === "starting") {
			parts.push("The torrent engine is starting.");
		} else if (status.state === "failed") {
			parts.push(status.message || "The torrent engine failed to start.");
		} else {
			parts.push(
				`Listening on port ${status.listenPort ?? "?"} · DHT ${
					status.dhtEnabled ? "on" : "off"
				}`,
			);
		}
	}
	// The listening port is technical detail: it only belongs in the
	// settings, not in the toolbar.
	if (info) info.textContent = "";
	if (details) details.textContent = parts.join(" ");
}

function renderSummary() {
	const summary = view.summarize(state.torrents);
	const parts = [
		`↓ ${formatSpeed(summary.downloadSpeed)}`,
		`↑ ${formatSpeed(summary.uploadSpeed)}`,
	];
	if (!summary.count) {
		parts.push("No torrents");
	} else {
		parts.push(`${summary.count} torrent${summary.count === 1 ? "" : "s"}`);
		if (summary.downloading) parts.push(`${summary.downloading} downloading`);
		if (summary.waiting) parts.push(`${summary.waiting} waiting for peers`);
		if (summary.seeding) parts.push(`${summary.seeding} seeding`);
		if (summary.paused) parts.push(`${summary.paused} paused`);
	}
	$("#torrent-summary").textContent = parts.join(" · ");
}

function renderEmptyState() {
	const empty = $("#torrent-empty");
	empty.hidden = state.torrents.length > 0;
	const title = empty.querySelector("strong");
	const description = empty.querySelector("span");
	const status = state.status;
	if (status && !status.available) {
		title.textContent = "BitTorrent support is unavailable";
		description.textContent =
			"This build does not include the torrent feature.";
	} else if (status && !status.enabled) {
		title.textContent = "Torrents are disabled";
		description.textContent =
			"Enable torrent support to add magnet links and .torrent files.";
	} else {
		title.textContent = "No torrents yet";
		description.textContent =
			"Add a magnet link or open a .torrent file to start downloading.";
	}
}

// ---------------------------------------------------------------------------
// Data refresh
// ---------------------------------------------------------------------------

async function refreshStatus() {
	try {
		const status = await api.torrentStatus();
		const wasReady = state.status?.state === "ready";
		state.status = status;
		renderStatus();
		if (status.state === "ready" && !wasReady) {
			await refreshList({ silent: true });
		}
	} catch (error) {
		state.status = {
			available: false,
			enabled: false,
			state: "failed",
			message: view.friendlyError(error),
			listenPort: null,
			dhtEnabled: false,
			restartRequired: false,
		};
		renderStatus();
	}
}

async function refreshSettings() {
	try {
		state.settings = await api.torrentSettingsGet();
	} catch {
		state.settings = null;
	}
}

async function refreshList({ silent = false } = {}) {
	if (state.refreshing) return;
	state.refreshing = true;
	try {
		const list = await api.torrentList();
		applyList(list);
	} catch (error) {
		if (!silent) notify(view.friendlyError(error), true);
	} finally {
		state.refreshing = false;
	}
}

// `torrent_list` omits per-file details, so the Files tab and the browser
// review flow read them from `torrent_files` and cache the result per torrent.
//
// The engine reuses a torrent id once a torrent is deleted, so the id alone
// is not an identity. Every cache entry therefore remembers the info hash it
// belongs to, and an entry that belongs to another torrent is thrown away
// instead of being shown for the wrong download.
function torrentFiles(torrent) {
	if (!torrent) return [];
	const cached = state.files.get(torrent.id);
	if (!cached) return [];
	if (state.filesHash.get(torrent.id) !== torrent.infoHash) {
		dropTorrentCache(torrent.id);
		return [];
	}
	return cached;
}

// Throws away everything the window remembers about a torrent. The engine
// reuses a torrent id, so a cache entry can belong to another download.
function dropTorrentCache(id) {
	state.files.delete(id);
	state.filesHash.delete(id);
	state.filesFetchedAt.delete(id);
	state.filesLoading.delete(id);
	state.swarm.delete(id);
	state.swarmFetchedAt.delete(id);
	state.swarmLoading.delete(id);
}

// Forgets the files the user ticked. Used whenever the selected download
// changes, because a tick only makes sense for one torrent.
function resetFileSelection() {
	state.fileSelection = null;
	state.filesTorrentId = null;
	state.filesTorrentHash = null;
	state.filesDirty = false;
}

function refreshFiles(torrent, { force = false } = {}) {
	if (!torrent || !Number.isInteger(torrent.id)) return Promise.resolve([]);
	const id = torrent.id;
	const inFlight = state.filesLoading.get(id);
	if (inFlight) return inFlight;
	// Every refresh of the file list costs a round trip to the engine, which
	// reads the torrent details and the stats. The progress event arrives
	// every second, so the list is refreshed on a timer instead of on every
	// tick.
	const fetchedAt = state.filesFetchedAt.get(id) || 0;
	if (!force && Date.now() - fetchedAt < FILES_REFRESH_INTERVAL) {
		return Promise.resolve(torrentFiles(torrent));
	}
	const request = api
		.torrentFiles(id)
		.then(files => {
			const list = Array.isArray(files) ? files : [];
			state.files.set(id, list);
			state.filesHash.set(id, torrent.infoHash);
			state.filesFetchedAt.set(id, Date.now());
			return list;
		})
		.finally(() => {
			state.filesLoading.delete(id);
		});
	state.filesLoading.set(id, request);
	return request;
}

function syncVisibleFiles() {
	const torrent = selectedTorrent();
	if (!torrent) return;
	const reviewing =
		state.review?.id === torrent.id &&
		view.torrentKey(state.review) === view.torrentKey(torrent);
	if (state.tab !== "files" && !reviewing) return;
	refreshFiles(torrent)
		.then(() => {
			if (state.selectedId !== torrent.id) return;
			renderDetail();
			renderReview();
		})
		.catch(() => {
			// The next poll retries; the tab keeps showing its loading note.
		});
}

function applyList(list) {
	state.torrents = Array.isArray(list) ? list : [];
	// The engine reuses the id of a deleted torrent. A torrent that comes
	// back under an id that now belongs to another download must not inherit
	// its file list, its file selection or its review state.
	for (const id of view.staleTorrentIds(state.torrentHash, state.torrents)) {
		dropTorrentCache(id);
		state.torrentHash.delete(id);
		if (state.selectedId === id) {
			state.selectedId = null;
			resetFileSelection();
		}
		if (state.review?.id === id) {
			state.review = null;
		}
	}
	for (const torrent of state.torrents) {
		state.torrentHash.set(torrent.id, torrent.infoHash);
	}
	if (
		state.selectedId !== null &&
		!state.torrents.some(torrent => torrent.id === state.selectedId)
	) {
		state.selectedId = null;
		resetFileSelection();
	}
	if (
		state.selectedId === null &&
		state.review &&
		state.torrents.some(torrent => torrent.id === state.review.id)
	) {
		state.selectedId = state.review.id;
		state.tab = "files";
	}
	if (state.selectedId === null && state.torrents.length) {
		state.selectedId = state.torrents[0].id;
		state.tab = "general";
	}
	renderList();
	renderDetail();
	renderSummary();
	renderToolbar();
	renderEmptyState();
	renderReview();
	syncVisibleFiles();
}

// ---------------------------------------------------------------------------
// List
// ---------------------------------------------------------------------------

function createRow(torrent) {
	const row = element("div", "torrent-row");
	row.dataset.id = String(torrent.id);
	row.setAttribute("role", "option");
	row.tabIndex = 0;

	const main = element("div", "torrent-row-main");
	const name = element("span", "torrent-name");
	const meta = element("span", "torrent-row-sub");
	const progress = document.createElement("progress");
	progress.className = "torrent-row-progress";
	progress.max = 100;
	progress.value = 0;
	main.append(name, meta, progress);

	row.append(
		main,
		element("span", "torrent-size"),
		element("span", "torrent-row-state"),
	);
	return row;
}

function rowSubtitle(torrent) {
	const percent = view.progressPercent(torrent);
	const key = view.stateKey(torrent);
	if (key === "downloading") {
		const eta = view.formatEta(torrent.timeRemainingSecs);
		return `${percent.toFixed(1)}% · ↓ ${formatSpeed(torrent.downloadSpeed)}${
			eta === "—" ? "" : ` · ${eta} left`
		}`;
	}
	if (key === "seeding") {
		return `Ratio ${view.formatRatio(torrent.ratio)} · ↑ ${formatSpeed(
			torrent.uploadSpeed,
		)}`;
	}
	if (key === "waiting") {
		return "No peer with the files yet — still looking";
	}
	if (key === "paused") return `Paused at ${percent.toFixed(1)}%`;
	if (key === "metadata") return "Fetching metadata…";
	if (key === "error") return torrent.error || "Transfer failed";
	return "—";
}

function updateRow(row, torrent) {
	const selected = torrent.id === state.selectedId;
	row.classList.toggle("selected", selected);
	row.setAttribute("aria-selected", selected ? "true" : "false");
	row.dataset.state = view.stateKey(torrent);
	row.querySelector(".torrent-name").textContent =
		torrent.name || torrent.infoHash;
	row.querySelector(".torrent-size").textContent = formatBytes(
		torrent.totalBytes,
	);
	row.querySelector(".torrent-row-state").textContent =
		view.stateShortLabel(torrent);
	row.querySelector(".torrent-row-sub").textContent = rowSubtitle(torrent);
	row.querySelector(".torrent-row-progress").value =
		view.progressPercent(torrent);
}

function renderList() {
	const container = $("#torrent-list");
	const existing = new Map(
		[...container.children].map(row => [Number(row.dataset.id), row]),
	);
	const seen = new Set();
	for (const torrent of state.torrents) {
		let row = existing.get(torrent.id);
		if (!row) {
			row = createRow(torrent);
			container.append(row);
		}
		updateRow(row, torrent);
		seen.add(torrent.id);
	}
	for (const [id, row] of existing) {
		if (!seen.has(id)) row.remove();
	}
}

function selectTorrent(id, { focus = false } = {}) {
	if (state.selectedId === id) {
		if (focus) $("#torrent-list")?.focus();
		return;
	}
	state.selectedId = id;
	resetFileSelection();
	state.tab = "general";
	renderList();
	renderDetail();
	renderToolbar();
	if (focus) {
		const row = [...$("#torrent-list").children].find(
			candidate => Number(candidate.dataset.id) === id,
		);
		row?.focus();
	}
}

function moveSelection(offset) {
	if (!state.torrents.length) return;
	const index = state.torrents.findIndex(
		torrent => torrent.id === state.selectedId,
	);
	const next = Math.min(
		state.torrents.length - 1,
		Math.max(0, index === -1 ? 0 : index + offset),
	);
	selectTorrent(state.torrents[next].id, { focus: true });
}

// ---------------------------------------------------------------------------
// Detail
// ---------------------------------------------------------------------------

function setFact(id, value) {
	$(id).textContent = value;
}

function renderDetail() {
	const torrent = selectedTorrent();
	$("#torrent-detail-empty").hidden = Boolean(torrent);
	$("#torrent-detail").hidden = !torrent;
	if (!torrent) return;

	$("#torrent-detail-name").textContent = torrent.name || torrent.infoHash;
	$("#torrent-detail-hash").textContent = torrent.infoHash || "";

	const errorCard = $("#torrent-detail-error");
	errorCard.hidden = !torrent.error;
	errorCard.textContent = torrent.error ? view.friendlyError(torrent.error) : "";

	const percent = view.progressPercent(torrent);
	$("#torrent-detail-progress").value = percent;

	setFact("#torrent-fact-status", view.stateLabel(torrent));
	setFact("#torrent-fact-size", formatBytes(torrent.totalBytes));
	setFact(
		"#torrent-fact-downloaded",
		`${formatBytes(torrent.progressBytes)} (${percent.toFixed(1)}%)`,
	);
	setFact("#torrent-fact-uploaded", formatBytes(torrent.uploadedBytes));
	setFact("#torrent-fact-ratio", view.formatRatio(torrent.ratio));
	setFact("#torrent-fact-down-speed", formatSpeed(torrent.downloadSpeed));
	setFact("#torrent-fact-up-speed", formatSpeed(torrent.uploadSpeed));
	setFact(
		"#torrent-fact-peers",
		`${torrent.peers || 0} (seeds ${torrent.seeds || 0}, leechers ${
			torrent.leechers || 0
		})`,
	);
	setFact("#torrent-fact-swarm", swarmText(torrent));
	setFact("#torrent-fact-eta", view.formatEta(torrent.timeRemainingSecs));
	setFact("#torrent-fact-added", formatAdded(torrent.addedOn));
	setFact("#torrent-fact-folder", torrent.outputFolder || "—");

	renderTabs();
	if (state.tab === "files") {
		renderFilesTab(torrent);
	} else if (state.tab === "trackers") {
		renderTrackers(torrent);
	}
	syncSwarm(torrent);
}

// The trackers know the size of the swarm, the window only knows the peers
// that are connected right now. Showing both makes it clear whether a
// torrent is waiting for the network or has nobody to download from.
function swarmText(torrent) {
	const swarm = torrent ? state.swarm.get(torrent.id) : null;
	if (!swarm || !swarm.length) return "—";
	let seeders = 0;
	let leechers = 0;
	let working = 0;
	for (const tracker of swarm) {
		if (tracker.state !== "working") continue;
		working += 1;
		seeders += Number(tracker.seeders) || 0;
		leechers += Number(tracker.leechers) || 0;
	}
	if (!working) return "—";
	return `${seeders} seeders, ${leechers} leechers reported by ${working} tracker${
		working === 1 ? "" : "s"
	}`;
}

// Reads the tracker numbers of the selected torrent. They change slowly, so
// the read is throttled like the file list.
function syncSwarm(torrent) {
	const id = torrent.id;
	if (state.swarmHash.get(id) && state.swarmHash.get(id) !== torrent.infoHash) {
		// The id now belongs to another download.
		dropTorrentCache(id);
	}
	state.swarmHash.set(id, torrent.infoHash);
	const fetchedAt = state.swarmFetchedAt.get(id) || 0;
	if (!state.swarmLoading.has(id) && Date.now() - fetchedAt < SWARM_REFRESH_INTERVAL) {
		return;
	}
	const request = api
		.torrentTrackerStatus(id)
		.then(trackers => {
			state.swarm.set(id, Array.isArray(trackers) ? trackers : []);
			state.swarmFetchedAt.set(id, Date.now());
			if (state.selectedId === id) renderDetail();
		})
		.catch(() => {
			// The next poll retries.
		})
		.finally(() => {
			state.swarmLoading.delete(id);
		});
	state.swarmLoading.set(id, request);
}

function renderTabs() {
	for (const button of $$(".torrent-tab")) {
		const active = button.dataset.tab === state.tab;
		button.classList.toggle("active-tab", active);
		button.setAttribute("aria-selected", active ? "true" : "false");
	}
	$("#torrent-tab-general").hidden = state.tab !== "general";
	$("#torrent-tab-files").hidden = state.tab !== "files";
	$("#torrent-tab-trackers").hidden = state.tab !== "trackers";
}

function renderFilesTab(torrent) {
	const files = torrentFiles(torrent);
	if (!files.length) {
		renderFilesPending(torrent);
		refreshFiles(torrent)
			.then(() => {
				if (state.selectedId !== torrent.id) return;
				renderDetail();
				renderReview();
			})
			.catch(error => {
				renderFilesPending(torrent, view.friendlyError(error));
			});
		return;
	}
	const rendered = $("#torrent-file-list").querySelectorAll(
		".torrent-file-row",
	).length;
	if (state.filesTorrentId !== torrent.id || files.length !== rendered) {
		renderFiles(torrent, files);
	} else {
		updateFileProgress(torrent);
	}
	renderReview();
}

function renderFilesPending(torrent, error) {
	const list = $("#torrent-file-list");
	list.replaceChildren(
		element(
			"p",
			"field-help",
			error ||
				(view.stateKey(torrent) === "metadata"
					? "Fetching metadata. The file list appears as soon as it arrives."
					: "Loading files…"),
		),
	);
	$("#torrent-files-toolbar").hidden = true;
	state.filesTorrentId = null;
	state.filesTorrentHash = null;
}

function defaultFileSelection(files, torrent) {
	const reported = view.includedIndices(files);
	if (reported.length) return reported;
	if (state.review?.id === torrent.id) {
		return files
			.map(file => Number(file.index))
			.filter(index => Number.isInteger(index) && index >= 0);
	}
	return [];
}

function renderFiles(torrent, files = torrentFiles(torrent)) {
	const list = $("#torrent-file-list");
	// Keep the selection the user made when the list is re-rendered, so a
	// background refresh does not throw the pending choice away. The
	// selection only belongs to this torrent when the info hash still
	// matches: the engine hands the same id to the next torrent after a
	// deletion.
	const sameTorrent =
		state.filesTorrentId === torrent.id &&
		state.filesTorrentHash === torrent.infoHash;
	const kept = sameTorrent ? state.fileSelection : null;
	list.replaceChildren();
	const selection = kept
		? [...kept]
				.filter(index =>
					files.some(file => Number(file.index) === Number(index)),
				)
				.sort((left, right) => left - right)
		: defaultFileSelection(files, torrent);
	state.fileSelection = new Set(selection);
	state.filesTorrentId = torrent.id;
	state.filesTorrentHash = torrent.infoHash;
	if (!kept) {
		state.filesDirty = false;
	}

	const toolbar = $("#torrent-files-toolbar");
	toolbar.hidden = files.length === 0;
	if (!files.length) {
		list.append(
			element(
				"p",
				"field-help",
				view.stateKey(torrent) === "metadata"
					? "Fetching metadata. The file list appears as soon as it arrives."
					: "This torrent does not report any files.",
			),
		);
		updateFilesApply();
		return;
	}

	for (const file of files) {
		const row = element("label", "torrent-file-row");
		row.dataset.index = String(file.index);

		const checkbox = document.createElement("input");
		checkbox.type = "checkbox";
		checkbox.checked = selection.includes(Number(file.index));
		checkbox.dataset.index = String(file.index);

		const main = element("span", "torrent-file-main");
		const name = element("span", "torrent-file-name", file.name);
		const progress = document.createElement("progress");
		progress.className = "torrent-file-progress";
		progress.max = 100;
		progress.value = view.fileProgressPercent(file);
		main.append(name, progress);

		row.append(
			checkbox,
			main,
			element(
				"span",
				"torrent-file-meta",
				`${formatBytes(file.progressBytes)} / ${formatBytes(file.length)}`,
			),
		);
		list.append(row);
	}
	updateFilesApply();
}

function updateFileProgress(torrent) {
	const list = $("#torrent-file-list");
	const files = torrentFiles(torrent);
	if (files.length !== list.querySelectorAll(".torrent-file-row").length) {
		renderFiles(torrent, files);
		return;
	}
	const byIndex = new Map(files.map(file => [file.index, file]));
	for (const row of list.querySelectorAll(".torrent-file-row")) {
		const file = byIndex.get(Number(row.dataset.index));
		if (!file) continue;
		row.querySelector(".torrent-file-progress").value =
			view.fileProgressPercent(file);
		row.querySelector(".torrent-file-meta").textContent =
			`${formatBytes(file.progressBytes)} / ${formatBytes(file.length)}`;
	}
}

function updateFilesApply() {
	const button = $("#torrent-files-apply");
	const torrent = selectedTorrent();
	const files = torrent ? torrentFiles(torrent) : [];
	// The button stays available whenever there is a file list, so the
	// selection can always be pushed to the engine.
	button.disabled = files.length === 0;
	renderFilesHint();
}

function renderFilesHint() {
	const hint = $("#torrent-files-hint");
	if (!hint) return;
	const torrent = selectedTorrent();
	const files = torrent ? torrentFiles(torrent) : [];
	if (!files.length) {
		hint.textContent =
			"Tick the files you want. Only the ticked files are downloaded and shared.";
		return;
	}
	const selected = state.fileSelection ? state.fileSelection.size : 0;
	hint.textContent = `${selected} of ${files.length} file${
		files.length === 1 ? "" : "s"
	} ticked. Only the ticked files are downloaded and shared.`;
}

function renderTrackers(torrent) {
	const list = $("#torrent-tracker-list");
	list.replaceChildren();
	const trackers = Array.isArray(torrent.trackers) ? torrent.trackers : [];
	if (!trackers.length) {
		const item = element("li", "field-help", "No trackers reported.");
		list.append(item);
		return;
	}
	for (const tracker of trackers) {
		const item = element("li", "", tracker);
		item.title = tracker;
		list.append(item);
	}
}

// ---------------------------------------------------------------------------
// Actions
// ---------------------------------------------------------------------------

async function performAction(action, message) {
	const torrent = selectedTorrent();
	if (!torrent) return;
	renderToolbar();
	try {
		await api.torrentAction(torrent.id, action);
		notify(message);
		await refreshList({ silent: true });
	} catch (error) {
		notify(view.friendlyError(error), true);
	}
}

function openRemoveDialog() {
	const torrent = selectedTorrent();
	if (!torrent) return;
	$("#torrent-remove-name").textContent =
		torrent.name || torrent.infoHash;
	showDialog("#torrent-remove-dialog");
}

async function removeSelected(action) {
	const torrent = selectedTorrent();
	if (!torrent) return;
	closeDialog("#torrent-remove-dialog");
	try {
		await api.torrentAction(torrent.id, action);
		// The torrent is gone. Its file list must go with it: the engine
		// hands the same id to the next torrent, and the window would show
		// the deleted download's files for the new one.
		dropTorrentCache(torrent.id);
		state.torrentHash.delete(torrent.id);
		state.selectedId = null;
		resetFileSelection();
		notify(
			action === "delete"
				? "Torrent and its files were deleted."
				: "Torrent removed from the list.",
		);
		await refreshList({ silent: true });
	} catch (error) {
		notify(view.friendlyError(error), true);
	}
}

async function enableTorrents() {
	try {
		const settings = state.settings || (await api.torrentSettingsGet());
		const saved = await api.torrentSettingsSet({
			...settings,
			enabled: true,
		});
		state.settings = saved;
		notify("Torrent support enabled.");
		await refreshStatus();
		window.setTimeout(() => {
			refreshStatus();
			refreshList({ silent: true });
		}, 600);
	} catch (error) {
		notify(view.friendlyError(error), true);
	}
}

async function restartEngine() {
	try {
		notify("Restarting the torrent engine…");
		await api.torrentRestartEngine();
		await refreshStatus();
		await refreshList({ silent: true });
	} catch (error) {
		notify(view.friendlyError(error), true);
	}
}

// ---------------------------------------------------------------------------
// Add torrent
// ---------------------------------------------------------------------------

let addMode = "magnet";

function setAddMode(mode) {
	addMode = mode === "file" ? "file" : "magnet";
	const magnetButton = $("#torrent-source-magnet");
	const fileButton = $("#torrent-source-file");
	const magnetActive = addMode === "magnet";
	magnetButton.classList.toggle("active-tab", magnetActive);
	fileButton.classList.toggle("active-tab", !magnetActive);
	magnetButton.setAttribute("aria-pressed", magnetActive ? "true" : "false");
	fileButton.setAttribute("aria-pressed", magnetActive ? "false" : "true");
	$("#torrent-magnet-panel").hidden = !magnetActive;
	$("#torrent-file-panel").hidden = magnetActive;
}

function openAddDialog(mode = "magnet") {
	$("#torrent-add-error").hidden = true;
	$("#torrent-magnet-input").value = "";
	$("#torrent-file-path").value = "";
	$("#torrent-output-folder").value =
		state.settings?.downloadDirectory || "";
	setAddMode(mode);
	showDialog("#torrent-add-dialog");
	window.setTimeout(() => {
		if (addMode === "magnet") $("#torrent-magnet-input").focus();
		else $("#torrent-file-browse").focus();
	}, 0);
}

async function browseTorrentFile() {
	const dialog = window.__TAURI__?.dialog;
	if (!dialog?.open) {
		notify("The file picker is unavailable.", true);
		return;
	}
	try {
		const path = await dialog.open({
			multiple: false,
			directory: false,
			filters: [{ name: "Torrent files", extensions: ["torrent"] }],
		});
		if (path) $("#torrent-file-path").value = String(path);
	} catch (error) {
		notify(view.friendlyError(error), true);
	}
}

async function browseFolder(inputSelector) {
	try {
		const directory = await api.chooseDirectory();
		if (directory) $(inputSelector).value = String(directory);
	} catch (error) {
		notify(view.friendlyError(error), true);
	}
}

async function submitAdd() {
	const errorElement = $("#torrent-add-error");
	errorElement.hidden = true;
	const button = $("#torrent-add-confirm");
	button.disabled = true;
	// Review mode fetches metadata without downloading payload so the window can
	// show the file list before anything is transferred.
	const options = {
		outputFolder: $("#torrent-output-folder").value.trim() || null,
		review: true,
	};
	try {
		let result;
		if (addMode === "magnet") {
			const magnet = $("#torrent-magnet-input").value.trim();
			if (!/^magnet:\?/i.test(magnet)) {
				throw new Error("Enter a magnet link that starts with magnet:?.");
			}
			result = await api.torrentAddMagnet(magnet, options);
		} else {
			const path = $("#torrent-file-path").value.trim();
			if (!path) throw new Error("Choose a .torrent file.");
			result = await api.torrentAddFile(path, options);
		}
		closeDialog("#torrent-add-dialog");
		const name = result?.name || "torrent";
		if (result?.alreadyManaged || !Number.isInteger(result?.id)) {
			notify(`Already added: ${name}`);
			await refreshList({ silent: true });
			if (Number.isInteger(result?.id)) selectTorrent(result.id);
			return;
		}
		notify(`Added ${name}. Choose the files to download.`);
		await beginReview({
			id: result.id,
			name,
			infoHash: result.infoHash || "",
		});
	} catch (error) {
		errorElement.textContent = view.friendlyError(error);
		errorElement.hidden = false;
	} finally {
		button.disabled = false;
	}
}

// ---------------------------------------------------------------------------
// Files
// ---------------------------------------------------------------------------

function applyFileSelection() {
	const torrent = selectedTorrent();
	const files = torrent ? torrentFiles(torrent) : [];
	if (!torrent || !state.fileSelection || !files.length) return;
	const included = [...state.fileSelection].sort((left, right) => left - right);
	api
		.torrentSetFiles(torrent.id, included)
		.then(async () => {
			dropTorrentCache(torrent.id);
			state.filesTorrentId = torrent.id;
			state.filesTorrentHash = torrent.infoHash;
			state.filesDirty = false;
			updateFilesApply();
			notify(
				included.length
					? `Saved. ${included.length} file${
							included.length === 1 ? "" : "s"
						} will be downloaded.`
					: "Saved. No file is ticked, so nothing is downloaded.",
			);
			await refreshFiles(torrent, { force: true });
			await refreshList({ silent: true });
		})
		.catch(error => notify(view.friendlyError(error), true));
}

function updateSelectionFromCheckboxes() {
	state.fileSelection = new Set(
		$$("#torrent-file-list input[type=checkbox]")
			.filter(checkbox => checkbox.checked)
			.map(checkbox => Number(checkbox.dataset.index)),
	);
	state.filesDirty = true;
	updateFilesApply();
	renderReview();
}

function setAllFiles(checked) {
	for (const checkbox of $$("#torrent-file-list input[type=checkbox]")) {
		checkbox.checked = checked;
	}
	updateSelectionFromCheckboxes();
}

// ---------------------------------------------------------------------------
// Browser handoff review
// ---------------------------------------------------------------------------

async function takePendingReview() {
	if (typeof api.torrentTakeReview !== "function") return;
	try {
		const pending = await api.torrentTakeReview();
		if (pending && pending.id !== undefined) await beginReview(pending);
	} catch {
		// No pending review.
	}
}

function renderReview() {
	const panel = $("#torrent-review-panel");
	const review = state.review;
	if (!review) {
		panel.hidden = true;
		return;
	}
	const torrent = state.torrents.find(item => item.id === review.id);
	const files = torrentFiles(torrent);
	panel.hidden = false;
	$("#torrent-review-name").textContent =
		torrent?.name || review.name || review.infoHash || "Torrent";
	let hint;
	if (!torrent) {
		hint = "Getting the file list…";
	} else if (!files.length) {
		hint =
			view.stateKey(torrent) === "metadata"
				? "Waiting for the torrent information. The file list appears as soon as it arrives."
				: "This torrent does not contain any files.";
	} else {
		const selected = state.fileSelection ? state.fileSelection.size : 0;
		hint = `${selected} of ${files.length} file${
			files.length === 1 ? "" : "s"
		} ticked. Nothing is downloaded until you start it.`;
	}
	$("#torrent-review-hint").textContent = hint;
}

async function beginReview(review) {
	if (!review || !Number.isInteger(review.id)) {
		state.review = null;
		renderReview();
		return;
	}
	if (state.answered.has(view.torrentKey(review))) {
		// The user already answered this review.
		return;
	}
	if (
		state.review?.id === review.id &&
		view.torrentKey(state.review) === view.torrentKey(review)
	) {
		// The same review arrived a second time (the event and the poll can
		// both hand it out). Keep the files the user already ticked.
		renderReview();
		return;
	}
	state.review = review;
	state.selectedId = review.id;
	state.tab = "files";
	resetFileSelection();
	renderReview();
	await refreshList({ silent: true });
	// A torrent that is already running has nothing to confirm. That happens
	// when the same magnet is handed over twice: the engine keeps the
	// torrent it already has, and the review would ask about files of a
	// download that is under way.
	const torrent = state.torrents.find(item => item.id === review.id);
	const key = view.stateKey(torrent);
	if (key === "downloading" || key === "seeding" || key === "waiting") {
		state.review = null;
		renderReview();
		return;
	}
	refreshFiles(torrent, { force: true })
		.then(() => {
			if (state.review?.id !== review.id) return;
			renderDetail();
			renderReview();
		})
		.catch(() => {});
	renderDetail();
	renderReview();
}

async function confirmReview() {
	const review = state.review;
	if (!review) return;
	const button = $("#torrent-review-start");
	button.disabled = true;
	// The files are applied before the torrent is started. If starting fails
	// the selection is still in place, so the review is closed either way and
	// the user is not stuck on a banner that does nothing.
	let applied = false;
	try {
		const torrent = state.torrents.find(item => item.id === review.id);
		let files = torrentFiles(torrent);
		if (!files.length) files = await refreshFiles(torrent, { force: true });
		// The ticks live in the file list, which may not have been drawn yet
		// when the user is quick. Without a choice the review takes the files
		// it would have shown as ticked anyway.
		const selection = state.fileSelection ?? defaultFileSelection(files, torrent);
		const plan = view.reviewPlan(
			files,
			selection,
			Boolean(state.settings?.startPaused),
		);
		if (plan.error) {
			throw new Error(plan.error);
		}
		await api.torrentSetFiles(review.id, plan.files);
		applied = true;
		await api.torrentAction(review.id, plan.action);
		notify(
			state.settings?.startPaused
				? `Queued: ${review.name || "torrent"} will start later.`
				: `Downloading ${review.name || "torrent"} (${
						plan.files.length === 1
							? "1 file"
							: `${plan.files.length} files`
					}).`,
		);
	} catch (error) {
		notify(view.friendlyError(error), true);
	} finally {
		if (applied) {
			state.answered.add(view.torrentKey(review));
			dropTorrentCache(review.id);
			resetFileSelection();
			state.review = null;
			renderReview();
		}
		button.disabled = false;
	}
	await refreshList({ silent: true });
}

async function cancelReview() {
	const review = state.review;
	if (!review) return;
	const button = $("#torrent-review-cancel");
	button.disabled = true;
	try {
		await api.torrentAction(review.id, "forget");
		notify("The torrent was removed. Nothing was downloaded.");
	} catch (error) {
		notify(view.friendlyError(error), true);
	} finally {
		state.answered.add(view.torrentKey(review));
		state.review = null;
		dropTorrentCache(review.id);
		resetFileSelection();
		renderReview();
		button.disabled = false;
		await refreshList({ silent: true });
	}
}

// ---------------------------------------------------------------------------
// Settings
// ---------------------------------------------------------------------------

function fillSettings(settings) {
	$("#torrent-setting-enabled").checked = Boolean(settings.enabled);
	$("#torrent-setting-directory").value = settings.downloadDirectory || "";
	$("#torrent-setting-port").value = String(settings.listenPort ?? 0);
	$("#torrent-setting-dht").checked = Boolean(settings.enableDht);
	$("#torrent-setting-upnp").checked = Boolean(settings.enableUpnp);
	$("#torrent-setting-utp").checked = Boolean(settings.enableUtp);
	$("#torrent-setting-peers").value = String(
		settings.maxPeersPerTorrent ?? DEFAULT_MAX_PEERS,
	);
	$("#torrent-setting-download-limit").value = String(
		settings.downloadLimitKbps ?? 0,
	);
	$("#torrent-setting-upload-limit").value = String(
		settings.uploadLimitKbps ?? 0,
	);
	$("#torrent-setting-ratio").value = String(settings.seedRatioLimit ?? 0);
	$("#torrent-setting-seed-time").value = String(
		settings.seedTimeLimitMinutes ?? 0,
	);
	$("#torrent-setting-start-paused").checked = Boolean(settings.startPaused);
	$("#torrent-setting-notify").checked = Boolean(settings.notifyOnComplete);
}

function collectSettings() {
	const base = state.settings || {};
	return {
		version: base.version ?? 1,
		enabled: $("#torrent-setting-enabled").checked,
		downloadDirectory:
			$("#torrent-setting-directory").value.trim() || null,
		listenPort: view.numberValue($("#torrent-setting-port").value, 0),
		enableDht: $("#torrent-setting-dht").checked,
		enableUpnp: $("#torrent-setting-upnp").checked,
		enableUtp: $("#torrent-setting-utp").checked,
		downloadLimitKbps: view.numberValue(
			$("#torrent-setting-download-limit").value,
			0,
		),
		uploadLimitKbps: view.numberValue(
			$("#torrent-setting-upload-limit").value,
			0,
		),
		maxPeersPerTorrent: view.numberValue(
			$("#torrent-setting-peers").value,
			DEFAULT_MAX_PEERS,
		),
		seedRatioLimit: view.numberValue(
			$("#torrent-setting-ratio").value,
			0,
		),
		seedTimeLimitMinutes: view.numberValue(
			$("#torrent-setting-seed-time").value,
			0,
		),
		startPaused: $("#torrent-setting-start-paused").checked,
		notifyOnComplete: $("#torrent-setting-notify").checked,
	};
}

async function openSettings() {
	if (!state.status?.available) {
		notify("This build does not include BitTorrent support.", true);
		return;
	}
	try {
		state.settings = await api.torrentSettingsGet();
		fillSettings(state.settings);
		$("#torrent-settings-error").hidden = true;
		showDialog("#torrent-settings-dialog");
	} catch (error) {
		notify(view.friendlyError(error), true);
	}
}

async function saveSettings() {
	const errorElement = $("#torrent-settings-error");
	errorElement.hidden = true;
	const next = collectSettings();
	const validation = view.validateSettingsInput(next);
	if (validation) {
		errorElement.textContent = validation;
		errorElement.hidden = false;
		return;
	}
	const button = $("#torrent-settings-save");
	button.disabled = true;
	try {
		state.settings = await api.torrentSettingsSet(next);
		fillSettings(state.settings);
		closeDialog("#torrent-settings-dialog");
		await refreshStatus();
		notify(
			state.status?.restartRequired
				? "Settings saved. Restart the engine to apply them."
				: "Torrent settings saved.",
		);
		await refreshList({ silent: true });
	} catch (error) {
		errorElement.textContent = view.friendlyError(error);
		errorElement.hidden = false;
	} finally {
		button.disabled = false;
	}
}

// ---------------------------------------------------------------------------
// Wiring
// ---------------------------------------------------------------------------

function wireEvents() {
	$("#torrent-add-magnet").addEventListener("click", () =>
		openAddDialog("magnet"),
	);
	$("#torrent-add-file").addEventListener("click", () => openAddDialog("file"));
	$("#torrent-pause").addEventListener("click", () =>
		performAction("pause", "Torrent paused."),
	);
	$("#torrent-resume").addEventListener("click", () =>
		performAction("resume", "Torrent resumed."),
	);
	$("#torrent-recheck").addEventListener("click", () =>
		performAction("recheck", "Recheck started."),
	);
	$("#torrent-remove").addEventListener("click", openRemoveDialog);
	$("#torrent-settings").addEventListener("click", openSettings);

	$("#torrent-remove-forget").addEventListener("click", () =>
		removeSelected("forget"),
	);
	$("#torrent-remove-delete").addEventListener("click", () =>
		removeSelected("delete"),
	);

	for (const button of $$("[data-close]")) {
		button.addEventListener("click", () =>
			closeDialog(`#${button.dataset.close}`),
		);
	}

	$("#torrent-source-magnet").addEventListener("click", () =>
		setAddMode("magnet"),
	);
	$("#torrent-source-file").addEventListener("click", () => setAddMode("file"));
	$("#torrent-file-browse").addEventListener("click", browseTorrentFile);
	$("#torrent-output-browse").addEventListener("click", () =>
		browseFolder("#torrent-output-folder"),
	);
	$("#torrent-add-confirm").addEventListener("click", submitAdd);

	$("#torrent-setting-directory-browse").addEventListener("click", () =>
		browseFolder("#torrent-setting-directory"),
	);
	$("#torrent-settings-save").addEventListener("click", saveSettings);
	$("#torrent-settings-restart").addEventListener("click", restartEngine);

	$("#torrent-list").addEventListener("click", event => {
		const row = event.target.closest(".torrent-row");
		if (row) selectTorrent(Number(row.dataset.id));
	});
	$("#torrent-list").addEventListener("keydown", event => {
		if (event.key === "ArrowDown" || event.key === "ArrowUp") {
			event.preventDefault();
			moveSelection(event.key === "ArrowDown" ? 1 : -1);
		} else if (event.key === "Enter" || event.key === " ") {
			event.preventDefault();
			const row = event.target.closest(".torrent-row");
			if (row) selectTorrent(Number(row.dataset.id));
		} else if (event.key === "Delete" || event.key === "Backspace") {
			event.preventDefault();
			openRemoveDialog();
		}
	});

	for (const button of $$(".torrent-tab")) {
		button.addEventListener("click", () => {
			state.tab = button.dataset.tab;
			renderDetail();
		});
	}

	$("#torrent-file-list").addEventListener("change", event => {
		if (!event.target.matches("input[type=checkbox]")) return;
		updateSelectionFromCheckboxes();
	});
	$("#torrent-files-all").addEventListener("click", () => setAllFiles(true));
	$("#torrent-files-none").addEventListener("click", () => setAllFiles(false));
	$("#torrent-files-apply").addEventListener("click", applyFileSelection);

	$("#torrent-review-start").addEventListener("click", confirmReview);
	$("#torrent-review-cancel").addEventListener("click", cancelReview);
	$("#torrent-toast").addEventListener("click", () => {
		$("#torrent-toast").hidden = true;
	});

	document.addEventListener("visibilitychange", () => {
		// The window keeps watching the engine even when it is covered, so
		// the progress stays up to date.
		api.torrentSetWatching(true).catch(() => {});
		refreshStatus();
		refreshList({ silent: true });
	});
	window.addEventListener("beforeunload", () => {
		api.torrentSetWatching(false).catch(() => {});
	});
}

function startLiveUpdates() {
	try {
		api.onTorrentProgress(list => {
			applyList(list);
		});
		api.onTorrentEngine(status => {
			state.status = status;
			renderStatus();
			if (status.state === "ready") refreshList({ silent: true });
		});
		api.onTorrentReview(async review => {
			// The backend keeps the review as a fallback for a window that was
			// not open yet. Drop that copy now: otherwise the polling below
			// hands the same review out again and the review reappears after
			// the user confirmed it.
			api.torrentTakeReview().catch(() => {});
			await beginReview(review);
		});
	} catch {
		// Polling below keeps the window functional without the event bridge.
	}
}

async function init() {
	wireEvents();
	startLiveUpdates();
	renderStatus();
	await refreshStatus();
	if (state.status?.available) {
		await Promise.allSettled([
			refreshSettings(),
			refreshList({ silent: true }),
		]);
		// The browser may have handed off a review before this window loaded, so
		// pick up the pending request even if the event was already emitted.
		await takePendingReview();
	}
	api.torrentSetWatching(true).catch(() => {});
	// A magnet link resolves metadata before the review exists, so keep
	// polling for a pending review while no review is active.
	//
	// The poll must not depend on `document.visibilityState`: on macOS the
	// page reports "hidden" as soon as another window covers the Torrents
	// window, and the review would then never be picked up.
	window.setInterval(() => {
		if (state.review) return;
		takePendingReview();
	}, 1000);
	window.setInterval(() => {
		refreshStatus();
	}, 3000);
	window.setInterval(() => {
		refreshList({ silent: true });
	}, 3000);
}

init();
