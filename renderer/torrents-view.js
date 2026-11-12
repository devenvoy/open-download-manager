// Created: 2026-11-11
// Reason: feat(torrent): dedicated Torrents window UI
// Description: Pure torrent view-model helpers shared by the torrent window and its
// tests. No DOM access so the same module runs in WebKit and under node:test.

"use strict";

const ACTIVE_STATES = new Set(["downloading", "metadata", "waiting"]);

function stateKey(torrent) {
	if (!torrent) return "unknown";
	const raw = String(torrent.state || "").toLowerCase();
	if (raw === "error") return "error";
	if (raw === "paused") return "paused";
	if (raw === "initializing") return "metadata";
	if (raw === "live") {
		// The engine reports a torrent without selected files as finished, but
		// that means "nothing is wanted", not "everything is downloaded".
		const done = Number(torrent.progressBytes) || 0;
		if (!torrent.finished) {
			// A running torrent that has not received a byte and cannot see a
			// single peer with the data is waiting for the swarm. It is not
			// broken, and saying "downloading" would be a lie.
			const seeds = Number(torrent.seeds) || 0;
			const speed = Number(torrent.downloadSpeed) || 0;
			if (done === 0 && seeds === 0 && speed === 0) return "waiting";
			return "downloading";
		}
		return done > 0 ? "seeding" : "idle";
	}
	return "unknown";
}

function stateLabel(torrent) {
	return (
		{
			downloading: "Downloading",
			waiting: "Waiting for peers",
			seeding: "Seeding",
			idle: "Nothing selected",
			paused: "Paused",
			metadata: "Fetching metadata",
			error: "Error",
			unknown: "Unknown",
		}[stateKey(torrent)] || "Unknown"
	);
}

function stateShortLabel(torrent) {
	const key = stateKey(torrent);
	if (key === "metadata") return "Metadata";
	if (key === "waiting") return "Waiting";
	return stateLabel(torrent);
}

function progressPercent(torrent) {
	if (!torrent) return 0;
	const total = Number(torrent.totalBytes) || 0;
	const done = Number(torrent.progressBytes) || 0;
	if (!total) return 0;
	if (done >= total) return 100;
	return Math.min(100, Math.max(0, (done / total) * 100));
}

function formatRatio(value) {
	const ratio = Number(value);
	return Number.isFinite(ratio) && ratio > 0 ? ratio.toFixed(2) : "0.00";
}

function formatEta(seconds) {
	const value = Number(seconds);
	if (!Number.isFinite(value) || value <= 0) return "—";
	if (value < 60) return `${Math.ceil(value)}s`;
	if (value < 3600) {
		return `${Math.floor(value / 60)}m ${Math.ceil(value % 60)}s`;
	}
	if (value < 86_400) {
		return `${Math.floor(value / 3600)}h ${Math.floor((value % 3600) / 60)}m`;
	}
	return `${Math.floor(value / 86_400)}d ${Math.floor((value % 86_400) / 3600)}h`;
}

function availableActions(torrent) {
	const key = stateKey(torrent);
	const exists = Boolean(torrent);
	return {
		pause: exists && ACTIVE_STATES.has(key),
		resume: exists && (key === "paused" || key === "error"),
		recheck: exists && key !== "metadata",
		forget: exists,
		delete: exists,
	};
}

function summarize(torrents) {
	const list = Array.isArray(torrents) ? torrents : [];
	let downloading = 0;
	let waiting = 0;
	let seeding = 0;
	let paused = 0;
	let downloadSpeed = 0;
	let uploadSpeed = 0;
	for (const torrent of list) {
		const key = stateKey(torrent);
		if (key === "downloading" || key === "metadata") downloading += 1;
		else if (key === "waiting") waiting += 1;
		else if (key === "seeding") seeding += 1;
		else if (key === "paused") paused += 1;
		downloadSpeed += Number(torrent.downloadSpeed) || 0;
		uploadSpeed += Number(torrent.uploadSpeed) || 0;
	}
	return {
		count: list.length,
		downloading,
		waiting,
		seeding,
		paused,
		downloadSpeed,
		uploadSpeed,
	};
}

function includedIndices(files) {
	if (!Array.isArray(files)) return [];
	return files
		.filter(file => file && file.included)
		.map(file => Number(file.index))
		.filter(index => Number.isInteger(index) && index >= 0)
		.sort((left, right) => left - right);
}

/**
 * Builds the plan for confirming a torrent review.
 *
 * The torrent was added with an empty file selection, so nothing is
 * transferred until the user confirms. This returns the file indices to
 * enable and the engine action to run afterwards.
 *
 * @param {Array} files the file list of the torrent under review
 * @param {Set|Array|null} selection the indices the user picked, or null to
 *   use the files that are already marked as included
 * @param {boolean} startPaused whether the torrent should stay paused
 * @returns {{files: number[], action: string, error: string|null}}
 */
function reviewPlan(files, selection, startPaused) {
	if (!Array.isArray(files) || !files.length) {
		return {
			files: [],
			action: startPaused ? "pause" : "resume",
			error: "The file list is not ready yet. Wait for metadata, then try again.",
		};
	}

	const picked =
		selection === null || selection === undefined
			? includedIndices(files)
			: [...selection];
	const selected = picked
		.map(index => Number(index))
		.filter(index => Number.isInteger(index) && index >= 0)
		.sort((left, right) => left - right);

	if (!selected.length) {
		return {
			files: [],
			action: startPaused ? "pause" : "resume",
			error: "Select at least one file to download.",
		};
	}

	return {
		files: selected,
		action: startPaused ? "pause" : "resume",
		error: null,
	};
}

function fileProgressPercent(file) {
	if (!file) return 0;
	const total = Number(file.length) || 0;
	const done = Number(file.progressBytes) || 0;
	if (!total) return 0;
	return Math.min(100, Math.max(0, (done / total) * 100));
}

function numberValue(value, fallback = 0) {
	if (value === "" || value === null || value === undefined) return fallback;
	const parsed = Number(value);
	return Number.isFinite(parsed) ? parsed : fallback;
}

function validateSettingsInput(settings) {
	const port = numberValue(settings.listenPort, 0);
	if (!Number.isInteger(port) || port < 0 || port > 65_535) {
		return "Listen port must be a whole number between 0 and 65535.";
	}
	const peers = numberValue(settings.maxPeersPerTorrent, 0);
	if (!Number.isInteger(peers) || peers < 1 || peers > 2_000) {
		return "Max peers per torrent must be between 1 and 2000.";
	}
	const downloadLimit = numberValue(settings.downloadLimitKbps, 0);
	const uploadLimit = numberValue(settings.uploadLimitKbps, 0);
	if (!Number.isInteger(downloadLimit) || downloadLimit < 0) {
		return "The download limit must be 0 or a positive whole number.";
	}
	if (!Number.isInteger(uploadLimit) || uploadLimit < 0) {
		return "The upload limit must be 0 or a positive whole number.";
	}
	const ratio = numberValue(settings.seedRatioLimit, 0);
	if (!Number.isFinite(ratio) || ratio < 0 || ratio > 1_000) {
		return "Seed ratio limit must be between 0 and 1000.";
	}
	const seedTime = numberValue(settings.seedTimeLimitMinutes, 0);
	if (!Number.isInteger(seedTime) || seedTime < 0 || seedTime > 525_600) {
		return "Seed time limit must be between 0 and 525600 minutes.";
	}
	const directory = String(settings.downloadDirectory || "").trim();
	if (directory && !directory.startsWith("/") && !/^[a-zA-Z]:[\\/]/.test(directory)) {
		return "Choose an absolute torrent download folder.";
	}
	return null;
}

/**
 * The identity of a torrent.
 *
 * The engine hands the same id to the next torrent once the list is empty,
 * so an id alone cannot tell two downloads apart. The info hash can.
 *
 * @param {{id: number, infoHash?: string}|null} torrent the torrent or review
 * @returns {string} a key that stays the same for one download
 */
function torrentKey(torrent) {
	if (!torrent) return "";
	const hash = torrent.infoHash || torrent.info_hash;
	return hash || `id:${torrent.id}`;
}

/**
 * Finds the torrent ids whose cached state belongs to another download.
 *
 * The window caches the file list per torrent id. The engine reuses an id
 * after a torrent is deleted, so a cache entry can belong to a torrent that
 * no longer exists or to a different download that took its place. Such an
 * entry must be dropped instead of being shown for the wrong download.
 *
 * @param {Map<number, string>} knownHashes the info hash each id stands for
 * @param {Array<{id: number, infoHash?: string}>} torrents the current list
 * @returns {Set<number>} the ids whose cache is stale
 */
function staleTorrentIds(knownHashes, torrents) {
	const list = Array.isArray(torrents) ? torrents : [];
	const stale = new Set();
	for (const [id, hash] of knownHashes) {
		const torrent = list.find(item => item.id === id);
		if (!torrent || torrent.infoHash !== hash) {
			stale.add(id);
		}
	}
	return stale;
}

/**
 * Turns an engine or IPC error into a message a user can act on.
 *
 * The engine reports internal wording such as "torrent is already live",
 * which means nothing to someone who just pressed a button.
 *
 * @param {unknown} error the rejected value of the failed call
 * @returns {string} a short, readable explanation
 */
function friendlyError(error) {
	const raw =
		typeof error?.message === "string"
			? error.message
			: error === undefined || error === null
				? ""
				: String(error);
	const text = raw.toLowerCase();
	if (!text) return "The action could not be completed.";
	if (text.includes("already live")) {
		return "This torrent is already running.";
	}
	if (text.includes("not paused") || text.includes("cannot pause")) {
		return "This torrent is not running.";
	}
	if (text.includes("torrent is not resolved") || text.includes("metadata")) {
		return "The torrent metadata is not ready yet. Wait a moment and try again.";
	}
	if (text.includes("initializing") || text.includes("starting")) {
		return "The torrent is still starting up. Try again in a moment.";
	}
	if (text.includes("file exists") || text.includes("already exists")) {
		return "A file with that name already exists in the download folder.";
	}
	if (text.includes("no space") || text.includes("disk")) {
		return "There is not enough space left on the disk.";
	}
	if (text.includes("not registered") || text.includes("unsupported")) {
		return "That magnet link or torrent file is not supported.";
	}
	if (text.includes("permission") || text.includes("access denied")) {
		return "The app is not allowed to write to that folder.";
	}
	if (text.includes("engine") && text.includes("not")) {
		return "The torrent engine is not running. Restart it in the settings.";
	}
	return raw;
}

const TorrentView = Object.freeze({
	stateKey,
	stateLabel,
	stateShortLabel,
	progressPercent,
	formatRatio,
	formatEta,
	availableActions,
	summarize,
	includedIndices,
	reviewPlan,
	friendlyError,
	fileProgressPercent,
	numberValue,
	torrentKey,
	staleTorrentIds,
	validateSettingsInput,
});

if (typeof window !== "undefined") {
	window.TorrentView = TorrentView;
}

if (typeof module !== "undefined" && module.exports) {
	module.exports = TorrentView;
}
