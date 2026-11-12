// Created: 2026-11-11
// Reason: test(torrent): verify torrent view-model formatting and action guards
// Description: Unit tests for the pure helpers backing the dedicated Torrents window.

"use strict";

const test = require("node:test");
const assert = require("node:assert");
const {
	stateKey,
	stateLabel,
	stateShortLabel,
	progressPercent,
	formatRatio,
	formatEta,
	availableActions,
	summarize,
	includedIndices,
	fileProgressPercent,
	numberValue,
	reviewPlan,
	friendlyError,
	torrentKey,
	staleTorrentIds,
	validateSettingsInput,
} = require("./torrents-view.js");

function torrent(overrides = {}) {
	return {
		id: 1,
		name: "debian.iso",
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
		...overrides,
	};
}

test("stateKey derives torrent states from the engine payload", () => {
	assert.equal(stateKey(null), "unknown");
	assert.equal(
		stateKey({ state: "live", finished: false, seeds: 2, downloadSpeed: 1024 }),
		"downloading",
	);
	assert.equal(
		stateKey({ state: "live", finished: true, progressBytes: 512 }),
		"seeding",
	);
	assert.equal(stateKey({ state: "paused", finished: false }), "paused");
	assert.equal(stateKey({ state: "initializing" }), "metadata");
	assert.equal(stateKey({ state: "error" }), "error");
	assert.equal(stateKey({ state: "other" }), "unknown");
});

test("stateKey reports a running torrent without a peer that has the data", () => {
	// The engine can be live and still have nobody to download from. Saying
	// "downloading" there is a lie, and it looks like a broken app.
	assert.equal(
		stateKey({ state: "live", finished: false, seeds: 0, downloadSpeed: 0 }),
		"waiting",
	);
	assert.equal(
		stateKey({
			state: "live",
			finished: false,
			seeds: 0,
			leechers: 4,
			downloadSpeed: 0,
		}),
		"waiting",
	);
	// A single seed means the transfer is about to start.
	assert.equal(
		stateKey({ state: "live", finished: false, seeds: 1, downloadSpeed: 0 }),
		"downloading",
	);
	// Bytes that arrived mean it is transferring.
	assert.equal(
		stateKey({
			state: "live",
			finished: false,
			seeds: 0,
			downloadSpeed: 0,
			progressBytes: 4096,
		}),
		"downloading",
	);
});

test("stateLabel uses readable labels", () => {
	assert.equal(
		stateLabel({ state: "live", finished: false, seeds: 1, downloadSpeed: 1024 }),
		"Downloading",
	);
	assert.equal(
		stateLabel({ state: "live", finished: false, seeds: 0, downloadSpeed: 0 }),
		"Waiting for peers",
	);
	assert.equal(
		stateLabel({ state: "live", finished: true, progressBytes: 512 }),
		"Seeding",
	);
	assert.equal(
		stateLabel({ state: "live", finished: true, progressBytes: 0 }),
		"Nothing selected",
	);
	assert.equal(stateLabel({ state: "paused" }), "Paused");
	assert.equal(stateLabel({ state: "initializing" }), "Fetching metadata");
	assert.equal(stateLabel({ state: "error" }), "Error");
	assert.equal(stateLabel({ state: "other" }), "Unknown");
	assert.equal(stateShortLabel({ state: "initializing" }), "Metadata");
	assert.equal(stateShortLabel({ state: "live", finished: true }), "Nothing selected");
	assert.equal(
		stateShortLabel({ state: "live", finished: true, progressBytes: 512 }),
		"Seeding",
	);
	assert.equal(
		stateShortLabel({ state: "live", finished: false, seeds: 0, downloadSpeed: 0 }),
		"Waiting",
	);
});

test("progressPercent clamps and handles missing totals", () => {
	assert.equal(progressPercent(null), 0);
	assert.equal(progressPercent(torrent({ progressBytes: 512, totalBytes: 1024 })), 50);
	assert.equal(progressPercent(torrent({ progressBytes: 0, totalBytes: 0 })), 0);
	assert.equal(
		progressPercent(torrent({ progressBytes: 2048, totalBytes: 1024 })),
		100,
	);
});

test("progressPercent does not claim a finished torrent that downloaded nothing", () => {
	// The engine reports a torrent without selected files as finished. That
	// means "nothing is wanted", not "everything is downloaded".
	assert.equal(
		progressPercent(torrent({ finished: true, progressBytes: 0, totalBytes: 1024 })),
		0,
	);
	assert.equal(
		progressPercent(torrent({ finished: true, progressBytes: 1024, totalBytes: 1024 })),
		100,
	);
});

test("stateKey does not report seeding before anything was downloaded", () => {
	assert.equal(
		stateKey({ state: "live", finished: true, progressBytes: 0 }),
		"idle",
	);
	assert.equal(
		stateKey({ state: "live", finished: true, progressBytes: 1024 }),
		"seeding",
	);
	assert.equal(stateKey({ state: "paused", finished: true }), "paused");
});

test("formatRatio keeps two decimals and never shows negatives", () => {
	assert.equal(formatRatio(0.1234), "0.12");
	assert.equal(formatRatio(1), "1.00");
	assert.equal(formatRatio(0), "0.00");
	assert.equal(formatRatio(-2), "0.00");
	assert.equal(formatRatio("not a number"), "0.00");
});

test("formatEta renders compact durations", () => {
	assert.equal(formatEta(0), "—");
	assert.equal(formatEta(null), "—");
	assert.equal(formatEta(-10), "—");
	assert.equal(formatEta(30), "30s");
	assert.equal(formatEta(90), "1m 30s");
	assert.equal(formatEta(3700), "1h 1m");
	assert.equal(formatEta(90_000), "1d 1h");
});

test("availableActions guards actions by torrent state", () => {
	assert.deepEqual(availableActions(null), {
		pause: false,
		resume: false,
		recheck: false,
		forget: false,
		delete: false,
	});
	assert.deepEqual(availableActions(torrent({ state: "live" })), {
		pause: true,
		resume: false,
		recheck: true,
		forget: true,
		delete: true,
	});
	assert.deepEqual(availableActions(torrent({ state: "paused" })), {
		pause: false,
		resume: true,
		recheck: true,
		forget: true,
		delete: true,
	});
	assert.deepEqual(availableActions(torrent({ state: "initializing" })), {
		pause: true,
		resume: false,
		recheck: false,
		forget: true,
		delete: true,
	});
	assert.deepEqual(availableActions(torrent({ state: "error" })), {
		pause: false,
		resume: true,
		recheck: true,
		forget: true,
		delete: true,
	});
});

test("torrentKey identifies a download by its info hash, not by its id", () => {
	// The engine hands the same id to the next torrent once the list is
	// empty, so an id alone cannot tell two downloads apart.
	const first = { id: 0, infoHash: "aaaa" };
	const second = { id: 0, infoHash: "bbbb" };
	assert.equal(torrentKey(first), "aaaa");
	assert.equal(torrentKey(second), "bbbb");
	assert.notEqual(torrentKey(first), torrentKey(second));
	assert.equal(torrentKey({ id: 0, infoHash: "aaaa" }), torrentKey(first));
	// Without a hash the id is the best that is available.
	assert.equal(torrentKey({ id: 7 }), "id:7");
	assert.equal(torrentKey(null), "");
});

test("staleTorrentIds finds cache entries that belong to another download", () => {
	const known = new Map([
		[0, "aaaa"],
		[1, "bbbb"],
	]);
	// The torrent with id 0 is gone and id 1 now holds a different download.
	const torrents = [{ id: 1, infoHash: "cccc" }];
	const stale = staleTorrentIds(known, torrents);
	assert.deepEqual([...stale].sort(), [0, 1]);
	// Nothing changed: the cache stays.
	assert.equal(staleTorrentIds(known, [{ id: 0, infoHash: "aaaa" }, { id: 1, infoHash: "bbbb" }]).size, 0);
	// A torrent that is new has no cache to invalidate.
	assert.equal(staleTorrentIds(new Map(), torrents).size, 0);
});

test("summarize aggregates counts and speeds", () => {
	const summary = summarize([
		torrent({ id: 1, state: "live", finished: false, downloadSpeed: 100, uploadSpeed: 10 }),
		torrent({ id: 2, state: "live", finished: true, downloadSpeed: 0, uploadSpeed: 20 }),
		torrent({ id: 3, state: "paused", downloadSpeed: 0, uploadSpeed: 0 }),
		torrent({ id: 4, state: "initializing", downloadSpeed: 50, uploadSpeed: 0 }),
	]);
	assert.deepEqual(summary, {
		count: 4,
		downloading: 2,
		waiting: 0,
		seeding: 1,
		paused: 1,
		downloadSpeed: 150,
		uploadSpeed: 30,
	});
	assert.deepEqual(summarize(null), {
		count: 0,
		downloading: 0,
		waiting: 0,
		seeding: 0,
		paused: 0,
		downloadSpeed: 0,
		uploadSpeed: 0,
	});
});

test("summarize counts a torrent that waits for peers separately", () => {
	const summary = summarize([
		torrent({
			id: 1,
			state: "live",
			finished: false,
			seeds: 0,
			progressBytes: 0,
			downloadSpeed: 0,
		}),
	]);
	assert.equal(summary.waiting, 1);
	assert.equal(summary.downloading, 0);
});

test("includedIndices returns sorted included file indexes", () => {
	assert.deepEqual(includedIndices(null), []);
	assert.deepEqual(
		includedIndices([
			{ index: 3, included: true },
			{ index: 1, included: false },
			{ index: 2, included: true },
			{ index: -1, included: true },
		]),
		[2, 3],
	);
});

test("reviewPlan uses the files marked as included when nothing was picked", () => {
	const plan = reviewPlan(
		[
			{ index: 0, included: true },
			{ index: 1, included: false },
			{ index: 2, included: true },
		],
		null,
		false,
	);
	assert.deepEqual(plan.files, [0, 2]);
	assert.equal(plan.action, "resume");
	assert.equal(plan.error, null);
});

test("reviewPlan honours the explicit file selection", () => {
	const plan = reviewPlan(
		[
			{ index: 0, included: true },
			{ index: 1, included: false },
		],
		new Set([1]),
		false,
	);
	assert.deepEqual(plan.files, [1]);
	assert.equal(plan.action, "resume");
	assert.equal(plan.error, null);
});

test("reviewPlan sorts and drops invalid indices", () => {
	const plan = reviewPlan([{ index: 0, included: true }], new Set([3, 1, -1, "x"]), false);
	assert.deepEqual(plan.files, [1, 3]);
});

test("reviewPlan reports when the file list is not ready", () => {
	const plan = reviewPlan([], null, false);
	assert.deepEqual(plan.files, []);
	assert.equal(plan.action, "resume");
	assert.match(plan.error, /file list is not ready/i);
});

test("reviewPlan reports an empty selection", () => {
	const plan = reviewPlan([{ index: 0, included: false }], new Set(), false);
	assert.deepEqual(plan.files, []);
	assert.match(plan.error, /at least one file/i);
});

test("reviewPlan keeps the torrent paused when the setting asks for it", () => {
	const plan = reviewPlan([{ index: 0, included: true }], null, true);
	assert.deepEqual(plan.files, [0]);
	assert.equal(plan.action, "pause");
	assert.equal(plan.error, null);
});

test("friendlyError turns engine wording into readable text", () => {
	assert.equal(
		friendlyError(new Error("torrent is already live")),
		"This torrent is already running.",
	);
	assert.equal(
		friendlyError(new Error("torrent is not resolved")),
		"The torrent metadata is not ready yet. Wait a moment and try again.",
	);
	assert.equal(
		friendlyError(new Error("No space left on device")),
		"There is not enough space left on the disk.",
	);
	assert.equal(friendlyError(new Error("")), "The action could not be completed.");
	assert.equal(
		friendlyError(new Error("something else went wrong")),
		"something else went wrong",
	);
});

test("fileProgressPercent handles partial and missing sizes", () => {
	assert.equal(fileProgressPercent(null), 0);
	assert.equal(fileProgressPercent({ length: 0, progressBytes: 0 }), 0);
	assert.equal(fileProgressPercent({ length: 200, progressBytes: 50 }), 25);
	assert.equal(fileProgressPercent({ length: 200, progressBytes: 400 }), 100);
});

test("numberValue falls back for empty or invalid input", () => {
	assert.equal(numberValue("", 7), 7);
	assert.equal(numberValue(null, 7), 7);
	assert.equal(numberValue(undefined, 7), 7);
	assert.equal(numberValue("12.5", 7), 12.5);
	assert.equal(numberValue("nope", 7), 7);
});

test("validateSettingsInput rejects unsafe values", () => {
	const base = {
		listenPort: 0,
		maxPeersPerTorrent: 50,
		downloadLimitKbps: 0,
		uploadLimitKbps: 0,
		seedRatioLimit: 0,
		seedTimeLimitMinutes: 0,
		downloadDirectory: "",
	};
	assert.equal(validateSettingsInput(base), null);
	assert.equal(
		validateSettingsInput({ ...base, downloadDirectory: "/Users/me/Torrents" }),
		null,
	);
	assert.equal(
		validateSettingsInput({ ...base, downloadDirectory: "C:\\Torrents" }),
		null,
	);
	assert.match(
		validateSettingsInput({ ...base, listenPort: 70_000 }),
		/Listen port/,
	);
	assert.match(
		validateSettingsInput({ ...base, maxPeersPerTorrent: 0 }),
		/Max peers/,
	);
	assert.match(
		validateSettingsInput({ ...base, downloadLimitKbps: -1 }),
		/download limit/,
	);
	assert.match(
		validateSettingsInput({ ...base, seedRatioLimit: 2_000 }),
		/Seed ratio/,
	);
	assert.match(
		validateSettingsInput({ ...base, seedTimeLimitMinutes: 600_000 }),
		/Seed time/,
	);
	assert.match(
		validateSettingsInput({ ...base, downloadDirectory: "relative/path" }),
		/absolute/,
	);
});
