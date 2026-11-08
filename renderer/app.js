// Created: 2026-10-24
// Reason: Initial public release
// Last updated: 2026-11-08
// Reason: feat(settings): expose category folders and temporary directory in options (Closes #32)
// Reason: feat(clipboard): add optional clipboard link monitoring (Closes #33)
// Reason: feat(proxy): add SOCKS5 proxy support and protocol-specific proxy settings (Closes #34)
// Reason: feat(limits): add per-site connection overrides and rolling download limits (Closes #36)
// Reason: feat(media): add opt-in advanced browser media adapter (Closes #38)
// Reason: feat(transfer): recover interrupted transfers and refresh expired links (Closes #3)
// Reason: feat(proxy): import system and browser proxy configurations (Closes #39)
// Reason: feat(queues): add named download queues and recurring schedules (Closes #13)
// Reason: feat(theme): add light, dark, and system theme options to the desktop app (Closes #4)
// Description: Main frontend application controller managing DOM interactions, dialogs, state updates, and Tauri IPC event subscriptions.

"use strict";
const $ = (selector, root = document) => root.querySelector(selector);
const $$ = (selector, root = document) => [...root.querySelectorAll(selector)];
const api = window.downloader;
const progressMode = new URLSearchParams(location.search).get("progress");
const integrationMode =
	new URLSearchParams(location.search).get("integration") === "1";
const optionsMode = new URLSearchParams(location.search).get("options") === "1";
const addMode = new URLSearchParams(location.search).get("add") === "1";
const schedulerMode =
	new URLSearchParams(location.search).get("scheduler") === "1";
if (progressMode) document.body.classList.add("progress-window");
if (integrationMode) document.body.classList.add("integration-window");
if (optionsMode) document.body.classList.add("options-window");
if (addMode) document.body.classList.add("add-window");
if (schedulerMode) document.body.classList.add("scheduler-window");
if (integrationMode) document.title = "Browser integration";
if (optionsMode) document.title = "Options";
if (addMode) document.title = "Download File Info";
if (schedulerMode) document.title = "Scheduler";

let currentThemeSetting = "system";

function getEffectiveTheme(themeSetting) {
	if (themeSetting === "light" || themeSetting === "dark") {
		return themeSetting;
	}
	return typeof window !== "undefined" &&
		window.matchMedia &&
		window.matchMedia("(prefers-color-scheme: dark)").matches
		? "dark"
		: "light";
}

function applyTheme(themeSetting = "system") {
	currentThemeSetting = themeSetting || "system";
	const effective = getEffectiveTheme(currentThemeSetting);
	document.documentElement.setAttribute("data-theme", effective);
	document.documentElement.setAttribute("data-theme-setting", currentThemeSetting);
	document.documentElement.style.colorScheme = effective;
}

if (typeof window !== "undefined" && window.matchMedia) {
	window.matchMedia("(prefers-color-scheme: dark)").addEventListener("change", () => {
		if (currentThemeSetting === "system") {
			applyTheme("system");
		}
	});
}

applyTheme("system");
let state = { downloads: [], settings: {}, integration: {}, fileIcons: {} },
	selectedId = null,
	selectionAnchorId = null,
	detailId = null,
	filter = "all",
	category = "",
	searchTerm = "",
	sort = "createdAt",
	ascending = false,
	visibleDownloads = [],
	toastTimer,
	unsubscribe,
	unsubscribeCommands;
const selectedIds = new Set();
const pending = new Set();
let pendingReviewId = null;
let selectionDrag = null,
	suppressRowClick = false;
const dismissedReviews = new Set();
const iconPaths = {
	add: ["M5 9h8l2-3h6l2 3h6v20H5z", "M17 13v12M11 19h12"],
	resume: [
		"M16 2a14 14 0 1 0 0 28a14 14 0 0 0 0-28",
		"M16 8v16M10 18l6 6 6-6",
	],
	stop: [
		"M9 17V8a2 2 0 0 1 4 0v8V4a2 2 0 0 1 4 0v12V6a2 2 0 0 1 4 0v11V10a2 2 0 0 1 4 0v12q0 8-9 8h-1q-4 0-7-5L3 18q-2-4 2-4z",
	],
	stopall: [
		"M9 17V8a2 2 0 0 1 4 0v8V4a2 2 0 0 1 4 0v12V6a2 2 0 0 1 4 0v11V10a2 2 0 0 1 4 0v12q0 8-9 8h-1q-4 0-7-5L3 18q-2-4 2-4z",
	],
	delete: ["M5 7h22M12 7V3h8v4M7 10l2 20h14l2-20M13 12v14M19 12v14"],
	clear: ["M5 6h22l-2 24H7zM12 6V3h8v3", "M10 17l5 5 8-11"],
	options: [
		"M12.22 2h-.44a2 2 0 0 0-2 2v.18a2 2 0 0 1-1 1.73l-.43.25a2 2 0 0 1-2 0l-.15-.08a2 2 0 0 0-2.73.73l-.22.38a2 2 0 0 0 .73 2.73l.15.1a2 2 0 0 1 1 1.72v.51a2 2 0 0 1-1 1.74l-.15.09a2 2 0 0 0-.73 2.73l.22.38a2 2 0 0 0 2.73.73l.15-.08a2 2 0 0 1 2 0l.43.25a2 2 0 0 1 1 1.73V20a2 2 0 0 0 2 2h.44a2 2 0 0 0 2-2v-.18a2 2 0 0 1 1-1.73l.43-.25a2 2 0 0 1 2 0l.15.08a2 2 0 0 0 2.73-.73l.22-.38a2 2 0 0 0-.73-2.73l-.15-.09a2 2 0 0 1-1-1.74v-.51a2 2 0 0 1 1-1.74l.15-.09a2 2 0 0 0 .73-2.73l-.22-.38a2 2 0 0 0-2.73-.73l-.15.08a2 2 0 0 1-2 0l-.43-.25a2 2 0 0 1-1-1.73V4a2 2 0 0 0-2-2z",
		"M12 9a3 3 0 1 0 0 6a3 3 0 0 0 0-6",
	],
	clock: [
		"M16 4a13 13 0 1 0 0 26a13 13 0 0 0 0-26",
		"M6 5 3 2M26 5l3-3M16 9v8l6-6",
	],
	startqueue: ["M2 2h23v24H2zM7 7h23v23H7z", "M22 12v18M16 24l6 6 6-6"],
	stopqueue: ["M2 2h23v24H2zM7 7h23v23H7z", "M19 18h12v13H19z"],
	browser: [
		"M16 3a13 13 0 1 0 0 26a13 13 0 0 0 0-26",
		"M3 16h26M16 3c-8 8-8 18 0 26M16 3c8 8 8 18 0 26M6 8h20M6 24h20",
	],
};
for (const button of $$("[data-icon]")) {
	const svg = document.createElementNS("http://www.w3.org/2000/svg", "svg");
	svg.setAttribute(
		"viewBox",
		button.dataset.icon === "options" ? "0 0 24 24" : "0 0 34 34",
	);
	svg.setAttribute("aria-hidden", "true");
	for (const data of iconPaths[button.dataset.icon] || []) {
		const path = document.createElementNS(
			"http://www.w3.org/2000/svg",
			"path",
		);
		path.setAttribute("d", data);
		svg.append(path);
	}
	button.prepend(svg);
}
function element(tag, className, text) {
	const node = document.createElement(tag);
	if (className) node.className = className;
	if (text !== undefined) node.textContent = text;
	return node;
}
const rememberLabel = element("label", "checkbox-label");
const rememberInput = element("input");
rememberInput.type = "checkbox";
rememberInput.id = "remember-category-path";
rememberInput.disabled = true;
rememberLabel.append(
	rememberInput,
	document.createTextNode("Remember this folder for the selected category"),
);
$("#add-directory").closest(".form-line").after(rememberLabel);
let integrationBrowsers = [],
	selectedBrowserName = null,
	setupPolling = false;
const browserStatus = (browser) =>
	browser.connected
		? ["Connected", "connected"]
		: !browser.installed
			? ["Not installed", "missing"]
			: browser.setupState === "approval"
				? ["Enable extension", "setup"]
				: browser.setupState === "disabled"
					? ["Disabled", "setup"]
					: browser.setupState === "removed"
						? ["Reinstall needed", "setup"]
						: ["Setup needed", "setup"];
const storeFallback = element("button", "button", "Install from Web Store");
storeFallback.type = "button";
storeFallback.id = "browser-store-action";
$("#browser-primary-action").after(storeFallback);
storeFallback.addEventListener("click", () => runBrowserSetup("store"));
function browserIcon(browser, className = "") {
	const frame = element("span", `browser-icon-frame ${className}`.trim());
	if (browser.icon) {
		const image = element("img");
		image.src = browser.icon;
		image.alt = "";
		frame.append(image);
	} else
		frame.append(
			element("span", "browser-monogram", browser.name.slice(0, 1)),
		);
	return frame;
}
function renderBrowserGrid(info) {
	integrationBrowsers = Array.isArray(info.browsers)
		? info.browsers.filter(
				(browser) => browser.installed || browser.connected,
			)
		: [];
	const grid = $("#browser-grid");
	grid.replaceChildren();
	for (const browser of integrationBrowsers) {
		const [label, tone] = browserStatus(browser),
			card = element("button", "browser-card");
		card.type = "button";
		card.setAttribute("role", "listitem");
		card.dataset.browser = browser.name;
		const copy = element("span", "browser-card-copy");
		const subtitle = browser.connected
			? `Connected in ${browser.profiles || 1} profile${browser.profiles === 1 ? "" : "s"}`
			: browser.setupState === "approval"
				? "Downloaded; waiting for your approval"
				: browser.setupState === "disabled"
					? "Enable it in your browser"
					: browser.setupState === "removed"
						? "Previously removed from this browser"
						: "Extension not detected";
		copy.append(
			element("strong", "", browser.name),
			element("span", "browser-card-subtitle", subtitle),
		);
		const status = element("span", `browser-status ${tone}`, label);
		card.append(browserIcon(browser), copy, status);
		card.addEventListener("click", () => showBrowserDetail(browser.name));
		grid.append(card);
	}
	const connected = integrationBrowsers.filter(
		(browser) => browser.connected,
	).length;
	$("#browser-connection-summary").textContent = info.connectionError
		? `Setup check failed: ${info.connectionError}`
		: `${connected} browser${connected === 1 ? "" : "s"} connected`;
	if (
		selectedBrowserName &&
		integrationBrowsers.some(
			(browser) => browser.name === selectedBrowserName,
		)
	)
		showBrowserDetail(selectedBrowserName);
}
function renderAdvancedIntegration(cap) {
	if (!cap) return;
	const badge = $("#advanced-integration-badge");
	const banner = $("#advanced-integration-banner");
	const msg = $("#advanced-integration-message");
	const enableBtn = $("#advanced-enable-button");
	const repairBtn = $("#advanced-repair-button");
	const disableBtn = $("#advanced-disable-button");

	if (!badge || !enableBtn) return;

	const stateObj = cap.state || {};
	const stateKind = typeof stateObj === "string" ? stateObj : stateObj.state;

	badge.className = "advanced-integration-badge";
	banner.hidden = true;
	msg.textContent = "";

	enableBtn.hidden = true;
	enableBtn.disabled = false;
	repairBtn.hidden = true;
	disableBtn.hidden = true;

	if (stateKind === "active") {
		badge.textContent = "Active (Protected)";
		badge.classList.add("active");
		disableBtn.hidden = false;
	} else if (stateKind === "degraded") {
		badge.textContent = "Degraded (Rolled Back)";
		badge.classList.add("degraded");
		banner.hidden = false;
		msg.textContent = `Health check or conflict occurred: ${stateObj.reason || "Operation degraded"}. Standard browser integration remains active.`;
		repairBtn.hidden = false;
		disableBtn.hidden = false;
	} else if (stateKind === "unsupported") {
		badge.textContent = "Unsupported";
		badge.classList.add("unsupported");
		banner.hidden = false;
		msg.textContent = `Enhanced media integration is not supported on this platform: ${stateObj.reason || "Platform capability unavailable."}`;
		enableBtn.hidden = false;
		enableBtn.disabled = true;
		enableBtn.title = "Enhanced media integration is not supported on this platform.";
	} else {
		badge.textContent = "Disabled";
		enableBtn.hidden = false;
	}
}
async function refreshAdvancedIntegration() {
	try {
		const cap = await api.getAdvancedIntegration();
		renderAdvancedIntegration(cap);
	} catch (e) {
		console.warn("Could not load advanced integration capability:", e);
	}
}
function showBrowserDetail(name) {
	const browser = integrationBrowsers.find((item) => item.name === name);
	if (!browser) return;
	selectedBrowserName = name;
	$("#browser-grid").hidden = true;
	$("#browser-detail").hidden = false;
	const icon = $("#browser-detail-icon");
	icon.replaceChildren(...browserIcon(browser).childNodes);
	$("#browser-detail-name").textContent = browser.name;
	const [label, tone] = browserStatus(browser);
	$("#browser-detail-status").textContent = label;
	$("#browser-detail-status").className = `browser-detail-status ${tone}`;
	$("#extension-path").value = browser.extensionPath || "";
	const primary = $("#browser-primary-action");
	primary.disabled = !browser.installed;
	primary.dataset.browser = browser.name;
	const enableExisting = ["approval", "disabled"].includes(
		browser.setupState,
	);
	primary.dataset.action =
		browser.storeUrl && !enableExisting && !browser.connected
			? "store"
			: browser.name === "Firefox"
				? "prepare"
				: "open";
	primary.textContent =
		primary.dataset.action === "store"
			? browser.setupState === "removed"
				? "Reinstall extension"
				: "Install browser extension"
			: `Open ${browser.name}`;
	storeFallback.hidden =
		!browser.storeUrl || primary.dataset.action === "store";
	$("#browser-detail-summary").textContent = browser.connected
		? "The extension and desktop connector are ready. Refresh an existing video tab if its download button is missing."
		: browser.setupState === "removed"
			? `${browser.name} remembers an earlier removal and will not add this extension automatically. Reinstall it from the official store to reconnect.`
			: browser.setupState === "approval"
				? `${browser.name} has downloaded the extension. Enable it in the browser to finish setup.`
				: browser.setupState === "disabled"
					? `The extension is installed but disabled. Enable it in ${browser.name} to reconnect.`
					: "Complete this one-time setup for this browser. If you just installed the app, fully close and reopen this browser to see its extension approval prompt.";
	const steps = $("#browser-steps");
	steps.replaceChildren();
	const texts =
		browser.storeUrl && (enableExisting || browser.connected)
			? [
					`Click Open ${browser.name}. Use the browser's Action required menu to enable Open Download Manager Integration if that prompt is shown.`,
					`Otherwise, enter ${browser.extensionPage} in the address bar and enable Open Download Manager Integration.`,
					"Return here and click Refresh, then reload your video page. If the extension is missing, use Install from Web Store.",
				]
			: browser.storeUrl
				? [
						`Click ${browser.setupState === "removed" ? "Reinstall extension" : "Install browser extension"} to open the official Chrome Web Store listing in this browser.`,
						"Click Add to Chrome (or the equivalent button in your browser), then confirm the installation.",
						"The desktop connector is prepared automatically. Return here and click Refresh, then reload your video page.",
					]
				: browser.name === "Firefox"
					? [
							"Click Open Firefox. Open Download Manager prepares the Firefox extension, registers its local connector, and opens Firefox.",
							`Enter ${browser.extensionPage} in Firefox's address bar.`,
							"On the Firefox page, select This Firefox, then Load Temporary Add-on.",
							"Choose manifest.json inside the extension folder shown below.",
							"Return here, click Refresh, then reload your video page.",
						]
					: [
							`Click Open ${browser.name}.`,
							`Enter ${browser.extensionPage} in ${browser.name}'s address bar.`,
							"Turn on Developer mode on the Extensions page.",
							"Choose Load unpacked. In the Windows folder picker, paste the extension folder path shown below into the address bar.",
							"Select the extension folder itself, then return here and click Refresh.",
							"Reload your video page. The Download this video button should appear when playable media is detected.",
						];
	for (const text of texts) steps.append(element("li", "", text));
	document.querySelector(".extension-path-label").hidden = Boolean(
		browser.storeUrl,
	);
	document.querySelector(".extension-path-row").hidden = Boolean(
		browser.storeUrl,
	);
	$("#automatic-setup-copy").textContent = browser.storeUrl
		? "Open Download Manager prepares its connector automatically. Your browser asks you to confirm adding the published extension."
		: browser.name === "Firefox"
			? "Firefox requires a signed add-on for permanent installation. Until the add-on is published, Firefox asks you to load the included extension again after restarting the browser."
			: "Use the included extension folder for this browser.";
}
async function runBrowserSetup(action, browserName = selectedBrowserName) {
	const error = $("#browser-setup-error");
	error.hidden = true;
	try {
		if (!browserName) throw new Error("Choose a browser first.");
		if (action === "prepare") {
			const prepared = await call("browserSetup", {
				action: "prepare",
				browser: browserName,
			});
			if (prepared?.extensionPath)
				$("#extension-path").value = prepared.extensionPath;
			await call("browserSetup", {
				action: "open",
				browser: browserName,
			});
		} else await call("browserSetup", { action, browser: browserName });
		if (action === "copy") notify("Extension folder path copied.");
	} catch (problem) {
		error.textContent = messageOf(problem);
		error.hidden = false;
	}
}
$("#browser-back").addEventListener("click", () => {
	selectedBrowserName = null;
	$("#browser-detail").hidden = true;
	$("#browser-grid").hidden = false;
});
$("#browser-refresh").addEventListener("click", () =>
	refreshIntegrationInfo(true),
);
$("#browser-primary-action").addEventListener("click", (event) =>
	runBrowserSetup(
		event.currentTarget.dataset.action,
		event.currentTarget.dataset.browser,
	),
);
$("#copy-extension-path").addEventListener("click", () =>
	runBrowserSetup("copy"),
);
$("#show-extension-folder").addEventListener("click", () =>
	runBrowserSetup("folder"),
);
async function refreshIntegrationInfo(showErrors = false) {
	if (setupPolling) return;
	setupPolling = true;
	try {
		const info = (await call("integration")) || state.integration;
		state.integration = info;
		renderBrowserGrid(info);
		await refreshAdvancedIntegration();
	} catch (problem) {
		if (showErrors) {
			$("#browser-setup-error").textContent = messageOf(problem);
			$("#browser-setup-error").hidden = false;
		}
	} finally {
		setupPolling = false;
	}
}

$("#advanced-enable-button")?.addEventListener("click", async () => {
	const confirmed = confirm(
		"Enable Enhanced Media Integration?\n\nThis installs an opt-in platform adapter to observe player streams. Only selected and approved media requests are captured; unrelated web browsing traffic is never collected or persisted. A non-destructive health verification check will run automatically."
	);
	if (!confirmed) return;
	try {
		const cap = await api.enableAdvancedIntegration({ consent: true });
		renderAdvancedIntegration(cap);
	} catch (err) {
		await refreshAdvancedIntegration();
		alert(messageOf(err));
	}
});

$("#advanced-repair-button")?.addEventListener("click", async () => {
	try {
		const cap = await api.repairAdvancedIntegration();
		renderAdvancedIntegration(cap);
	} catch (err) {
		await refreshAdvancedIntegration();
		alert(messageOf(err));
	}
});

$("#advanced-disable-button")?.addEventListener("click", async () => {
	try {
		const cap = await api.disableAdvancedIntegration();
		renderAdvancedIntegration(cap);
	} catch (err) {
		await refreshAdvancedIntegration();
		alert(messageOf(err));
	}
});
const setupPoll = setInterval(() => {
	if (integrationMode) refreshIntegrationInfo();
}, 3000);
window.addEventListener("beforeunload", () => clearInterval(setupPoll));
const completionActions = element("div", "transfer-buttons");
completionActions.id = "download-complete-actions";
completionActions.hidden = true;
for (const [label, action] of [
	["Open", "open"],
	["Open with…", "openWith"],
	["Open folder", "reveal"],
	["Close", "close"],
]) {
	const button = element("button", "button", label);
	button.type = "button";
	button.addEventListener("click", () =>
		action === "close"
			? $("#details-dialog").close()
			: performAction(detailDownload(), action),
	);
	completionActions.append(button);
}
const completedPanel = element("section", "completed-panel");
const completedSummary = element("div", "completed-summary");
completedPanel.append(completedSummary);
const completedFields = {};
for (const [key, label] of [
	["url", "Address"],
	["path", "The file saved as"],
]) {
	const fieldLabel = element("label", "completed-field", label),
		input = element("input");
	input.readOnly = true;
	input.setAttribute("aria-label", label);
	fieldLabel.append(input);
	completedPanel.append(fieldLabel);
	completedFields[key] = input;
}
completedPanel.append(completionActions);
const suppressLabel = element("label", "checkbox-label"),
	suppressCompletion = element("input");
suppressCompletion.type = "checkbox";
suppressLabel.append(
	suppressCompletion,
	document.createTextNode("Don't show this dialog again"),
);
completedPanel.append(suppressLabel);
suppressCompletion.addEventListener("change", async () => {
	try {
		await call("settings", {
			hideCompletionDialog: suppressCompletion.checked,
		});
	} catch (error) {
		suppressCompletion.checked = !suppressCompletion.checked;
		notify(messageOf(error), true);
	}
});
$("#details-dialog").append(completedPanel);
const renderTransferDetails = renderDetails;
renderDetails = function () {
	renderTransferDetails();
	const d = detailDownload();
	if (!d || statusOf(d) !== "completed") return;
	completedSummary.replaceChildren(
		element("span", "completed-icon", "✓"),
		element(
			"div",
			"",
			`Download complete\nDownloaded ${bytes(d.total || d.downloaded)} (${Math.round(d.total || d.downloaded || 0)} Bytes)`,
		),
	);
	completedFields.url.value = d.url || "";
	completedFields.path.value = d.path || "";
	suppressCompletion.checked = state.settings.hideCompletionDialog === true;
};
function numeric(value) {
	const result = Number(value);
	return Number.isFinite(result) && result > 0 ? result : 0;
}
function redactUrl(raw) {
	if (!raw) return "";
	try {
		const u = new URL(raw);
		if (u.protocol === "ftp:" && (u.username || u.password)) {
			const port = u.port && u.port !== "21" ? `:${u.port}` : "";
			return `ftp://******:******@${u.hostname}${port}${u.pathname}`;
		}
		u.username = "";
		u.password = "";
		u.search = "";
		u.hash = "";
		return u.href;
	} catch {
		return raw;
	}
}
function statusOf(d) {
	const value = String(d.status || "queued").toLowerCase();
	return (
		{
			active: "downloading",
			running: "downloading",
			probing: "connecting",
			complete: "completed",
			done: "completed",
			error: "failed",
			canceled: "cancelled",
			pending: "queued",
		}[value] || value
	);
}
function categoryOf(d) {
	const value = String(d.category || "").toLowerCase();
	if (/video/.test(value)) return "Video";
	if (/audio|music/.test(value)) return "Audio";
	if (/doc|text/.test(value)) return "Documents";
	if (/arch|compress/.test(value)) return "Archives";
	if (/program/.test(value)) return "Programs";
	return "Other";
}
function bytes(value) {
	const size = numeric(value);
	if (!size) return "0 B";
	const power = Math.min(4, Math.floor(Math.log(size) / Math.log(1024)));
	return `${(size / 1024 ** power).toFixed(power ? 2 : 0)} ${["B", "KB", "MB", "GB", "TB"][power]}`;
}
function date(value) {
	if (!value) return "";
	const parsed = new Date(value);
	return Number.isNaN(parsed.getTime())
		? ""
		: parsed.toLocaleString([], { dateStyle: "short", timeStyle: "short" });
}
function timeLeft(d) {
	const total = numeric(d.total) || numeric(d.streamProgress?.estimatedBytes);
	if (!total || !numeric(d.speed)) return "";
	const seconds = Math.max(
		0,
		Math.ceil((total - numeric(d.downloaded)) / numeric(d.speed)),
	);
	return (!d.total || d.estimated ? "≈ " : "") + duration(seconds);
}
function sizeLabel(d) {
	if (statusOf(d) === "connecting" || statusOf(d) === "queued") return "Connecting…";
	return numeric(d.total)
		? (d.estimated ? "≈ " : "") + bytes(d.total)
		: d.streamProgress?.estimatedBytes
			? "≈ " + bytes(d.streamProgress.estimatedBytes)
			: d.kind === "hls"
				? "Calculating from segments…"
				: "Not supplied by server";
}
function duration(seconds) {
	return seconds < 60
		? `${Math.ceil(seconds)} sec`
		: seconds < 3600
			? `${Math.ceil(seconds / 60)} min`
			: `${Math.floor(seconds / 3600)} hr ${Math.floor((seconds % 3600) / 60)} min`;
}
function percentage(d) {
	return statusOf(d) === "completed"
		? 100
		: numeric(d.total)
			? Math.min(100, (numeric(d.downloaded) / numeric(d.total)) * 100)
			: d.streamProgress?.total
				? (d.streamProgress.completed / d.streamProgress.total) * 100
				: null;
}
function messageOf(error) {
	return error?.message || String(error || "The operation failed.");
}
function notify(message, error = false, options = {}) {
	clearTimeout(toastTimer);
	$("#toast-text").textContent = message;
	$("#toast").dataset.error = String(Boolean(error));

	const actionBtn = $("#toast-action");
	if (actionBtn) {
		if (options.actionText && typeof options.onAction === "function") {
			actionBtn.textContent = options.actionText;
			actionBtn.onclick = () => {
				$("#toast").hidden = true;
				options.onAction();
			};
			actionBtn.hidden = false;
		} else {
			actionBtn.hidden = true;
			actionBtn.onclick = null;
		}
	}

	$("#toast").hidden = false;
	toastTimer = setTimeout(
		() => {
			$("#toast").hidden = true;
		},
		error ? 14000 : 7000,
	);
}
async function call(method, ...args) {
	if (!api || typeof api[method] !== "function")
		throw new Error(
			"The download service is unavailable. Restart Open Download Manager.",
		);
	const result = await api[method](...args);
	if (result?.error && !result.downloads)
		throw new Error(
			typeof result.error === "string"
				? result.error
				: result.error.message || "The request failed.",
		);
	return result;
}
function acceptSnapshot(snapshot) {
	if (!snapshot || !Array.isArray(snapshot.downloads)) return;
	state = {
		downloads: snapshot.downloads,
		settings: snapshot.settings || state.settings,
		integration: snapshot.integration || state.integration,
		fileIcons: snapshot.fileIcons || state.fileIcons,
	};
	if (state.settings?.theme) {
		applyTheme(state.settings.theme);
	}
	const valid = new Set(state.downloads.map((d) => d.id));
	for (const id of selectedIds) if (!valid.has(id)) selectedIds.delete(id);
	if (!valid.has(selectedId))
		selectedId = selectedIds.values().next().value || null;
	if (!valid.has(selectionAnchorId)) selectionAnchorId = selectedId;
	$("#download-list").setAttribute("aria-busy", "false");
	$("#connection-label").textContent = "Connected";
	render();
	if (!$$("dialog[open]").length) {
		const review =
			progressMode &&
			state.downloads.find(
				(d) =>
					d.id === progressMode &&
					d.needsReview &&
					!dismissedReviews.has(d.id),
			);
		if (review) openReview(review.id);
	}
}
async function refresh() {
	acceptSnapshot(await call("snapshot"));
}
function selected() {
	return state.downloads.find((d) => d.id === selectedId);
}
function canResume(d) {
	return (
		d &&
		!d.needsReview &&
		["queued", "paused", "failed", "cancelled"].includes(statusOf(d))
	);
}
function canPause(d) {
	return d && ["connecting", "downloading"].includes(statusOf(d));
}
function matches(d) {
	const status = statusOf(d);
	if (filter === "completed" && status !== "completed") return false;
	if (filter === "unfinished" && status === "completed") return false;
	if (
		filter === "queued" &&
		!["queued", "scheduled", "paused"].includes(status)
	)
		return false;
	if (category && categoryOf(d) !== category) return false;
	return (
		!searchTerm ||
		`${d.filename || ""} ${d.url || ""}`.toLowerCase().includes(searchTerm)
	);
}
function paintSelection() {
	$$(".download-row").forEach((row) => {
		const active = selectedIds.has(row.dataset.id);
		row.classList.toggle("selected", active);
		row.setAttribute("aria-selected", String(active));
	});
	updateControls();
}
function selectDownload(
	id,
	{ additive = false, toggle = false, range = false } = {},
) {
	if (!id) {
		selectedIds.clear();
		selectedId = null;
		selectionAnchorId = null;
		paintSelection();
		return;
	}
	if (range) {
		const targetIndex = visibleDownloads.findIndex(
			(download) => download.id === id,
		);
		let anchorIndex = visibleDownloads.findIndex(
			(download) => download.id === selectionAnchorId,
		);
		if (anchorIndex < 0)
			anchorIndex = visibleDownloads.findIndex(
				(download) => download.id === selectedId,
			);
		if (anchorIndex < 0) anchorIndex = targetIndex;
		if (!additive) selectedIds.clear();
		const start = Math.min(anchorIndex, targetIndex),
			end = Math.max(anchorIndex, targetIndex);
		for (let index = start; index <= end; index++)
			if (visibleDownloads[index])
				selectedIds.add(visibleDownloads[index].id);
		if (!selectionAnchorId)
			selectionAnchorId = visibleDownloads[anchorIndex]?.id || id;
	} else {
		if (!additive) selectedIds.clear();
		if (toggle && selectedIds.has(id)) selectedIds.delete(id);
		else selectedIds.add(id);
		selectionAnchorId = id;
	}
	selectedId = selectedIds.has(id)
		? id
		: selectedIds.values().next().value || null;
	paintSelection();
}
function rowFor(d) {
	const status = statusOf(d),
		percent = percentage(d);
	const row = element("tr", "download-row");
	row.dataset.id = d.id;
	row.dataset.status = status;
	row.classList.toggle("selected", selectedIds.has(d.id));
	row.setAttribute("aria-selected", String(selectedIds.has(d.id)));
	const nameCell = element("td");
	nameCell.title = d.filename || d.url;
	const name = element("span", "file-name");
	const extension = (
			String(d.filename || d.path || "").match(
				/\.[a-z0-9]{1,12}$/i,
			)?.[0] || ""
		).toLowerCase(),
		nativeIcon = state.fileIcons?.[extension];
	if (nativeIcon) {
		const icon = element("img", "file-type-icon native-file-icon");
		icon.src = nativeIcon;
		icon.alt = "";
		name.append(icon);
	} else
		name.append(element("span", "file-type-icon file-type-fallback", ""));
	name.append(element("span", "", d.filename || "Preparing file…"));
	nameCell.append(name);
	const names = {
		completed: "Complete",
		downloading:
			percent === null ? "Receiving data…" : `${percent.toFixed(2)}%`,
		connecting: "Connecting…",
		assembling: "Finalizing…",
		queued: "Queued",
		scheduled: "Scheduled",
		paused: "Stopped",
		cancelled: "Cancelled",
		failed: "Error",
	};
	const statusCell = element(
		"td",
		`row-status ${status}`,
		names[status] || status,
	);
	statusCell.title = d.error || names[status] || status;
	const description =
		(status === "failed"
			? (d.error || d.notice || d.description)
			: (d.notice || d.description || d.error)) ||
		(d.scheduledAt && status === "scheduled"
			? `Starts ${date(d.scheduledAt)}`
			: "");
	const descriptionCell = element("td", "", description);
	descriptionCell.title = description;
	row.append(
		nameCell,
		element(
			"td",
			"",
			["queued", "scheduled", "paused"].includes(status) ? "✓" : "",
		),
		element("td", "", sizeLabel(d)),
		statusCell,
		element("td", "", status === "downloading" ? timeLeft(d) : ""),
		element(
			"td",
			"",
			status === "downloading" ? `${bytes(d.speed)}/sec` : "",
		),
		element("td", "", date(d.lastTry || d.createdAt)),
		descriptionCell,
	);
	row.addEventListener("click", (event) => {
		if (suppressRowClick) return;
		const additive = event.ctrlKey || event.metaKey;
		selectDownload(d.id, {
			additive,
			range: event.shiftKey,
			toggle: additive && !event.shiftKey,
		});
	});
	row.addEventListener("dblclick", () => showDetails(d.id));
	row.addEventListener("contextmenu", (event) => {
		event.preventDefault();
		selectDownload(d.id);
		showDetails(d.id);
	});
	return row;
}
function updateControls() {
	const d = selected(),
		busy = d && pending.has(d.id);
	$("#resume-selected").disabled = (!canResume(d) && !d?.needsReview) || busy;
	$("#stop-selected").disabled = !canPause(d) || busy;
	$("#remove-selected").disabled = !d || busy;
	$("#open-selected").disabled = !d || statusOf(d) !== "completed";
	$("#reveal-selected").disabled = !d;
	$("#details-selected").disabled = !d;
	$("#pause-all").disabled = !state.downloads.some(canPause);
	$("#stop-queue").disabled = $("#pause-all").disabled;
	$("#resume-all").disabled = !state.downloads.some(canResume);
	$("#remove-completed").disabled = !state.downloads.some(
		(d) => statusOf(d) === "completed",
	);
	$("#scheduler-start").disabled = $("#resume-all").disabled;
	$("#scheduler-stop").disabled = $("#pause-all").disabled;
}
function render() {
	visibleDownloads = state.downloads.filter(matches).sort((a, b) => {
		const result =
			sort === "total"
				? numeric(a.total) - numeric(b.total)
				: sort === "createdAt"
					? new Date(a.createdAt || 0) - new Date(b.createdAt || 0)
					: String(a[sort] || "").localeCompare(
							String(b[sort] || ""),
						);
		return ascending ? result : -result;
	});
	$("#download-list").replaceChildren(...visibleDownloads.map(rowFor));
	$("#empty-state").hidden = visibleDownloads.length > 0;
	$("#empty-message").textContent = searchTerm
		? "No downloads match your search."
		: "No downloads to display.";
	const complete = state.downloads.filter(
		(d) => statusOf(d) === "completed",
	).length;
	$("#list-count").textContent =
		`${visibleDownloads.length} ${visibleDownloads.length === 1 ? "download" : "downloads"}${state.downloads.length !== visibleDownloads.length ? ` of ${state.downloads.length}` : ""} (${complete} completed)`;
	$("#status-summary").textContent =
		`${bytes(state.downloads.reduce((sum, d) => sum + numeric(d.speed), 0))}/sec`;
	const integrationButton = $("#integration-button"),
		needsBrowser =
			Boolean(state.integration?.packaged) &&
			!state.integration?.connectedBrowsers?.length;
	integrationButton.classList.toggle("needs-attention", needsBrowser);
	integrationButton.title = needsBrowser
		? "No browser integration is connected. Click to set it up."
		: "Browser Integration";
	integrationButton.setAttribute("aria-label", integrationButton.title);
	updateControls();
	if ($("#details-dialog").open) renderDetails();
	if ($("#scheduler-dialog").open) renderScheduler();
}
async function performAction(d, action) {
	if (!d || pending.has(d.id)) return;
	if (
		action === "remove" &&
		!window.confirm(
			`Delete “${d.filename || "this download"}” from the download list?\n\nThe downloaded file will remain on your PC.`,
		)
	)
		return;
	pending.add(d.id);
	updateControls();
	try {
		await call("action", { id: d.id, action });
		await refresh();
	} catch (error) {
		notify(messageOf(error), true);
	} finally {
		pending.delete(d.id);
		updateControls();
		if ($("#details-dialog").open) renderDetails();
	}
}
async function bulkAction(action) {
	const downloads = state.downloads.filter(
		action === "pause" ? canPause : canResume,
	);
	const results = await Promise.allSettled(
		downloads.map((d) =>
			call("action", {
				id: d.id,
				action:
					action === "resume" &&
					["failed", "cancelled"].includes(statusOf(d))
						? "retry"
						: action,
			}),
		),
	);
	const failures = results.filter((r) => r.status === "rejected");
	if (failures.length)
		notify(
			`${failures.length} operation(s) failed: ${messageOf(failures[0].reason)}`,
			true,
		);
	try {
		await refresh();
	} catch (error) {
		notify(messageOf(error), true);
	}
}
async function removeCompleted() {
	const downloads = state.downloads.filter(
		(d) => statusOf(d) === "completed",
	);
	if (
		!downloads.length ||
		!window.confirm(
			`Delete ${downloads.length} completed download(s) from the list?\n\nThe downloaded files will remain on your PC.`,
		)
	)
		return;
	const results = await Promise.allSettled(
		downloads.map((d) => call("action", { id: d.id, action: "remove" })),
	);
	const failures = results.filter((r) => r.status === "rejected");
	if (failures.length) notify(messageOf(failures[0].reason), true);
	try {
		await refresh();
	} catch (error) {
		notify(messageOf(error), true);
	}
}
function showDialog(id) {
	const dialog = $(id);
	if (!dialog.open) dialog.showModal();
}
let chosenYouTubeFormat = null;
let currentVideoChoices = [];

function isYouTubePlaylistUrl(text) {
	if (!text || typeof text !== "string") return false;
	const trimmed = text.trim();
	if (!/^https?:\/\//i.test(trimmed)) return false;
	return /(?:youtube\.com\/(?:playlist\?list=|watch\?.*[?&]list=)|youtu\.be\/.*[?&]list=)([\w-]{10,64})/i.test(trimmed);
}
function isYouTubeVideoUrl(text) {
	if (!text || typeof text !== "string") return false;
	const trimmed = text.trim();
	if (!/^https?:\/\//i.test(trimmed)) return false;
	if (isYouTubePlaylistUrl(trimmed)) return false;
	return /(?:youtube\.com\/(?:watch\?.*v=|shorts\/|embed\/)|youtu\.be\/)([\w-]{11})/i.test(trimmed);
}
function checkPlaylistUrl() {
	const val = $("#download-urls")?.value || "";
	const playlistBanner = $("#playlist-banner");
	const videoBanner = $("#video-banner");
	const isPlaylist = isYouTubePlaylistUrl(val);
	const isVideo = isYouTubeVideoUrl(val);
	if (playlistBanner) playlistBanner.hidden = !isPlaylist;
	if (videoBanner) videoBanner.hidden = !isVideo;
}
function openAdd(text = "", scheduled = "") {
	pendingReviewId = null;
	chosenYouTubeFormat = null;
	currentVideoChoices = [];
	$("#add-error").hidden = true;
	$("#download-urls").value = text;
	$("#download-urls").readOnly = false;
	checkPlaylistUrl();
	$("#download-filename").value = "";
	$("#download-description").value = "";
	$("#download-category").value = "";
	rememberInput.checked = false;
	rememberInput.disabled = true;
	$("#download-schedule").value = scheduled;
	$("#add-directory").value = state.settings.defaultDirectory || "";
	refreshQueuesUI();
	showDialog("#add-dialog");
	$("#download-urls").focus();
}
async function openAddFromClipboard() {
	openAdd(await call("clipboard"));
}
function openReview(id) {
	if (!progressMode) {
		call("openProgress", { id }).catch((e) => notify(messageOf(e), true));
		return;
	}
	document.body.classList.add("review-window");
	document.title = "Download File Info";
	const d = state.downloads.find(
		(item) => item.id === id && item.needsReview,
	);
	if (!d || dismissedReviews.has(id)) return;
	openAdd(d.url || "");
	pendingReviewId = id;
	$("#download-urls").readOnly = true;
	$("#download-filename").value = d.filename || "";
	$("#download-description").value = d.description || "";
	$("#download-category").value = d.category || "";
	rememberInput.disabled = !$("#download-category").value;
	$("#add-directory").value =
		d.directory || state.settings.defaultDirectory || "";
}
async function dismissReview() {
	if (!pendingReviewId) return;
	const id = pendingReviewId;
	dismissedReviews.add(id);
	pendingReviewId = null;
	try {
		await call("action", { id, action: "remove" });
		await refresh();
		if (progressMode) await call("closeProgress");
	} catch (error) {
		notify(messageOf(error), true);
	}
}
function showDetails(id) {
	if (state.downloads.find((d) => d.id === id)?.needsReview) {
		openReview(id);
		return;
	}
	if (!progressMode) {
		call("openProgress", { id }).catch((e) => notify(messageOf(e), true));
		return;
	}
	document.body.classList.remove("review-window");
	detailId = id;
	const details = $("#connection-details");
	call("resizeProgress", { expanded: !details?.hidden }).catch(() => {});
	$("#detail-speed-limit").value = state.settings.speedLimitKbps || 0;
	$("#notify-complete").checked = state.settings.notifyOnComplete !== false;
	$("#reveal-complete").checked = state.settings.revealOnComplete === true;
	$("#completion-result").textContent = "";
	setDetailTab("status");
	renderDetails();
	showDialog("#details-dialog");
}
function renderDetails() {
	const d = state.downloads.find((d) => d.id === detailId);
	if (!d) {
		$("#details-dialog").close();
		return;
	}
	const status = statusOf(d),
		percent = percentage(d),
		content = $("#details-content");
	$("#details-title").textContent =
		`${percent !== null ? `${Math.floor(percent)}% ` : ""}${d.filename || "Download status"}`;
	const isWatchPage = d.url && (d.url.includes("youtube.com/watch") || d.url.includes("youtu.be/"));
	const sourceAddress = isWatchPage ? (d.referer || d.url || "") : (d.url || d.referer || "");
	$("#transfer-source").textContent = sourceAddress;
	$("#transfer-source").title = sourceAddress;
	const names = {
		completed: "Complete",
		downloading: "Receiving data…",
		connecting: "Connecting…",
		paused: "Stopped",
		queued: "Queued",
		scheduled: "Scheduled",
		failed: "Error",
		cancelled: "Cancelled",
		assembling: "Finalizing…",
	};
	const rows = [
		["Status", names[status] || status],
		["File size", sizeLabel(d)],
		[
			"Downloaded",
			`${bytes(d.downloaded)}${percent !== null ? ` (${percent.toFixed(2)}%)` : ""}`,
		],
		[
			"Transfer rate",
			status === "downloading" ? `${bytes(d.speed)}/sec` : "0 B/sec",
		],
		[
			"Time left",
			status === "downloading" ? timeLeft(d) || "Estimating…" : "—",
		],
		[
			"Resume capability",
			d.resumable === true
				? "Yes"
				: d.resumable === false
					? "No"
					: "Checking…",
		],
	];
	if (d.referer && d.referer !== d.url) {
		rows.push(["Referrer", d.referer]);
	}
	if (d.transferKind) {
		const transferKindLabels = {
			segmented_http: `Segmented HTTP (${d.connections || 1} connections)`,
			single_http: "Single HTTP stream",
			paired: "Paired audio & video streams",
			sabr: "YouTube SABR streaming",
			ytdlp: "yt-dlp fragment stream",
			hls: "HLS segment stream",
		};
		if (transferKindLabels[d.transferKind]) {
			rows.push(["Transfer type", transferKindLabels[d.transferKind]]);
		}
	}
	if (numeric(d.duration))
		rows.splice(1, 0, [
			"Video duration",
			`${Math.floor(d.duration / 60)} min ${Math.floor(d.duration % 60)} sec`,
		]);
	if (d.streamProgress?.total && status !== "completed")
		rows.push([
			"Segments",
			`${d.streamProgress.completed} / ${d.streamProgress.total}`,
		]);
	if (d.scheduledAt && status === "scheduled")
		rows.push(["Start at", date(d.scheduledAt)]);
	if (d.notice) rows.push(["Note", d.notice]);
	content.replaceChildren();
	for (const [label, value] of rows)
		content.append(element("dt", "", label), element("dd", "", value));
	const progress = $("#detail-progress");
	if (percent === null && ["downloading", "connecting"].includes(status))
		progress.removeAttribute("value");
	else progress.value = percent || 0;
	const segments = Array.isArray(d.connectionsProgress)
		? d.connectionsProgress
		: [];
	const bars = $("#segment-bars"),
		map = $("#segment-map"),
		caption = document.querySelector(".connection-caption");
	bars.replaceChildren();
	map.replaceChildren();

	const isSegmented = d.transferKind === "segmented_http";
	const isPaired = d.transferKind === "paired" || d.kind === "paired";
	const isHls = d.transferKind === "hls" || d.kind === "hls";

	// Show per-range map bar ONLY for segmented HTTP downloads with multiple connections.
	// Never display a duplicate overall bar for single stream, paired media, or SABR.
	if (isSegmented && segments.length > 1) {
		map.style.display = "flex";
		if (caption) {
			caption.style.display = "";
			caption.textContent = "Start positions and download progress by connections";
		}
	} else {
		map.style.display = "none";
		if (caption) {
			if (isPaired && segments.length > 0) {
				caption.style.display = "";
				caption.textContent = "Media tracks";
			} else if (isHls && segments.length > 0) {
				caption.style.display = "";
				caption.textContent = "HLS download streams";
			} else {
				caption.style.display = "none";
			}
		}
	}

	for (let index = 0; index < segments.length; index++) {
		const segment = segments[index];
		const row = element("tr");
		const segmentStatus =
			segment.status === "complete" || segment.status === "completed"
				? "Completed."
				: segment.status ||
					(status === "downloading"
						? "Receiving data…"
						: names[status] || status);
		const trackLabel = isPaired
			? (index === 0 ? "Video" : index === 1 ? "Audio" : String(index + 1))
			: String(index + 1);
		row.append(
			element("td", "", trackLabel),
			element("td", "", bytes(segment.downloaded)),
			element("td", "", segmentStatus),
		);
		bars.append(row);

		if (isSegmented && segments.length > 1) {
			const bar = element("progress");
			bar.max = 100;
			const total = numeric(segment.total);
			bar.value = total
				? Math.min(100, (numeric(segment.downloaded) / total) * 100)
				: 0;
			bar.setAttribute("aria-label", `Connection ${index + 1} progress`);
			if (!total && status === "downloading") bar.removeAttribute("value");
			map.append(bar);
		}
	}
	const noteText = isPaired
		? (segments.length === 2 ? "2 tracks (Video + Audio)" : `${segments.length} track(s)`)
		: isSegmented
			? `${segments.length} connection(s)`
			: segments.length > 1
				? `${segments.length} connection(s)`
				: "";
	$("#segment-note").textContent = noteText;
	const isError = (status === "error" || status === "failed");
	const errorCard = $("#detail-error-card");
	if (errorCard) {
		errorCard.hidden = !isError;
		if (isError) {
			const errText = d.error || "Download failed or was interrupted.";
			const isFfmpeg = /ffmpeg/i.test(errText);
			const isYtdlp = /yt-dlp/i.test(errText);
			const isExpired = /expired|HTTP 401|HTTP 403|HTTP 410/i.test(errText);
			$("#detail-error-title").textContent = isFfmpeg
				? "FFmpeg Assembly Required"
				: isYtdlp
					? "YouTube Media Tool Required"
					: isExpired
						? "Download Link Expired"
						: "Download Interrupted";
			$("#detail-error-summary").textContent = errText;
			$("#detail-error-view-btn").onclick = () => openErrorDialog(d.id);
			$("#detail-error-retry-btn").onclick = () => performAction(d, "resume");
		}
	}
	const connDetails = $("#connection-details");
	if (connDetails) {
		connDetails.hidden = isError;
	}

	const isFfmpegError = Boolean(d.error && /ffmpeg is required/i.test(d.error));
	const toolBanner = $("#media-tool-fix-banner");
	if (toolBanner) {
		toolBanner.hidden = !isFfmpegError;
	}
	if (isFfmpegError && (status === "error" || status === "failed") && !d._mediaToolPrompted) {
		d._mediaToolPrompted = true;
		openMediaToolDialog("ffmpeg", d.id);
	} else if (d.error && (status === "error" || status === "failed") && !d._notified) {
		d._notified = true;
		notify(d.error, true, {
			actionText: "View Details",
			onAction: () => openErrorDialog(d.id),
		});
	}
	$("#detail-open").disabled = status !== "completed";
	$("#detail-resume").disabled = !canResume(d) || pending.has(d.id);
	$("#detail-stop").disabled = !canPause(d) || pending.has(d.id);
	$("#detail-cancel").disabled =
		["completed", "cancelled"].includes(status) || pending.has(d.id);
	$(".transfer-buttons").hidden = status === "completed";
	$("#download-complete-actions").hidden = status !== "completed";
	$("#details-dialog").classList.toggle(
		"is-complete",
		status === "completed",
	);
	renderActionLedger(d);
}
function renderActionLedger(d) {
	const container = $("#completion-action-history");
	const retryBtn = $("#detail-retry-actions");
	if (!container) return;
	container.innerHTML = "";
	if (!d || !Array.isArray(d.actionLedger) || d.actionLedger.length === 0) {
		container.innerHTML = "<p class='field-help' style='margin: 4px 0;'>No post-download actions recorded yet.</p>";
		if (retryBtn) retryBtn.hidden = true;
		return;
	}
	const list = document.createElement("ul");
	list.style.listStyle = "none";
	list.style.padding = "0";
	list.style.margin = "4px 0";
	for (const entry of d.actionLedger) {
		const li = document.createElement("li");
		li.style.padding = "3px 0";
		li.style.fontSize = "12px";
		const isSuccess = entry.status === "success";
		const isFailed = entry.status === "failed";
		const icon = isSuccess ? "✓" : isFailed ? "⚠️" : "ℹ️";
		const color = isSuccess ? "#166534" : isFailed ? "#991b1b" : "#4b5563";
		li.innerHTML = `<span style="color: ${color}; font-weight: bold;">${icon} ${entry.actionType}</span>: <span>${entry.status}</span> ${entry.message ? `— <em>${entry.message}</em>` : ""}`;
		list.appendChild(li);
	}
	container.appendChild(list);
	if (retryBtn) {
		retryBtn.hidden = statusOf(d) !== "completed";
	}
}
function setDetailTab(name) {
	for (const tab of ["status", "speed", "completion"]) {
		const active = tab === name;
		$(`#tab-${tab}`).classList.toggle("active-tab", active);
		$(`#tab-${tab}`).setAttribute("aria-selected", String(active));
		$(`#${tab}-panel`).hidden = !active;
	}
}
function setView(nextFilter, nextCategory = "") {
	filter = nextFilter;
	category = nextCategory;
	$$("[data-filter],[data-category]").forEach((button) => {
		const active = nextCategory
			? button.dataset.category === nextCategory
			: button.dataset.filter === nextFilter;
		button.classList.toggle("selected", active);
		if (active) button.setAttribute("aria-current", "page");
		else button.removeAttribute("aria-current");
	});
	render();
}
function updateProxyFieldsState() {
	const enabled = $("#proxy-enabled").checked;
	const protocol = $("#proxy-protocol").value;
	const isDirect = protocol === "direct";
	const isSocks5 = protocol === "socks5";
	const container = $("#proxy-fields");
	if (container) {
		container.classList.toggle("disabled", !enabled);
	}
	$("#proxy-protocol").disabled = !enabled;
	$("#proxy-host").disabled = !enabled || isDirect;
	$("#proxy-port").disabled = !enabled || isDirect;
	$("#proxy-username").disabled = !enabled || isDirect;
	$("#proxy-password").disabled = !enabled || isDirect;
	$("#test-proxy-button").disabled = !enabled;

	const dnsGroup = $("#proxy-dns-group");
	if (dnsGroup) {
		dnsGroup.style.display = isSocks5 ? "flex" : "none";
		if ($("#proxy-resolve-dns-remotely")) {
			$("#proxy-resolve-dns-remotely").disabled = !enabled || !isSocks5;
		}
	}
}

function resetProxyTestStatus() {
	const statusEl = $("#proxy-test-status");
	if (statusEl) {
		statusEl.textContent = "";
		statusEl.className = "proxy-test-status";
	}
}

let currentDetectedProxyConfig = null;

function formatProxyDisplay(proxy) {
	if (!proxy || !proxy.enabled || proxy.protocol === "direct") {
		return "Direct (No Proxy)";
	}
	const userPrefix = proxy.username ? `${proxy.username}@` : "";
	return `${String(proxy.protocol || "http").toUpperCase()} ${userPrefix}${proxy.host}:${proxy.port}`;
}

function openProxyDiscoveryDialog(detected) {
	currentDetectedProxyConfig = detected;
	const dialog = $("#proxy-discovery-dialog");
	if (!dialog) return;

	// Source badge
	const sourceBadge = $("#proxy-discovery-source-badge");
	let sourceText = "System";
	if (typeof detected.source === "string") {
		sourceText = detected.source.charAt(0).toUpperCase() + detected.source.slice(1);
	} else if (detected.source && typeof detected.source === "object") {
		const key = Object.keys(detected.source)[0];
		sourceText = detected.source[key] || "Browser";
	}
	if (sourceBadge) sourceBadge.textContent = sourceText;

	// Mode badge
	const modeBadge = $("#proxy-discovery-mode-badge");
	const modeText =
		detected.mode === "pac"
			? "PAC Script"
			: detected.mode === "manual"
				? "Manual Proxy"
				: "Direct Connection";
	if (modeBadge) modeBadge.textContent = modeText;

	// Auth badge & note
	const authBadge = $("#proxy-discovery-auth-badge");
	const authNote = $("#proxy-discovery-auth-note");
	if (authBadge) authBadge.hidden = !detected.credentialsAvailable;
	if (authNote) authNote.hidden = !detected.credentialsAvailable;

	// PAC section
	const pacSection = $("#proxy-discovery-pac-section");
	const pacUrlEl = $("#proxy-discovery-pac-url");
	if (detected.mode === "pac" && detected.pacUrl) {
		if (pacSection) pacSection.hidden = false;
		if (pacUrlEl) pacUrlEl.textContent = detected.pacUrl;
	} else if (pacSection) {
		pacSection.hidden = true;
	}

	// Routes list
	const routesSection = $("#proxy-discovery-routes-section");
	const routesList = $("#proxy-discovery-routes-list");
	if (routesList) {
		routesList.innerHTML = "";
		if (Array.isArray(detected.routes) && detected.routes.length > 0) {
			if (routesSection) routesSection.hidden = false;
			for (const route of detected.routes) {
				const li = document.createElement("li");
				li.className = "proxy-route-item";
				const textSpan = document.createElement("span");
				textSpan.textContent = `${route.protocol.toUpperCase()}: ${route.host}:${route.port}`;
				const badgeSpan = document.createElement("span");
				badgeSpan.className = `proxy-route-badge ${route.supported ? "supported" : "unsupported"}`;
				badgeSpan.textContent = route.supported
					? "Supported"
					: route.reason || "Unsupported";
				li.appendChild(textSpan);
				li.appendChild(badgeSpan);
				routesList.appendChild(li);
			}
		} else if (routesSection) {
			routesSection.hidden = detected.mode === "pac";
		}
	}

	// Diff / Preview
	const currentProxy = {
		enabled: $("#proxy-enabled")?.checked === true,
		protocol: $("#proxy-protocol")?.value || "http",
		host: $("#proxy-host")?.value?.trim() || "",
		port: Number($("#proxy-port")?.value) || 8080,
		username: $("#proxy-username")?.value?.trim() || null,
	};
	const currentEl = $("#proxy-diff-current");
	if (currentEl) currentEl.textContent = formatProxyDisplay(currentProxy);

	const diffDiscovered = $("#proxy-diff-discovered");
	if (diffDiscovered) {
		if (detected.importSettings) {
			diffDiscovered.textContent = formatProxyDisplay(detected.importSettings);
		} else if (detected.mode === "pac") {
			diffDiscovered.textContent = "PAC Script (Auto-routing unavailable)";
		} else {
			diffDiscovered.textContent = "Direct (No Proxy)";
		}
	}

	// Unsupported warnings
	const unsuppSection = $("#proxy-discovery-unsupported-section");
	const unsuppList = $("#proxy-discovery-unsupported-list");
	if (unsuppList) {
		unsuppList.innerHTML = "";
		if (
			Array.isArray(detected.unsupportedFields) &&
			detected.unsupportedFields.length > 0
		) {
			if (unsuppSection) unsuppSection.hidden = false;
			for (const field of detected.unsupportedFields) {
				const item = document.createElement("div");
				item.textContent = `• ${field}`;
				unsuppList.appendChild(item);
			}
		} else if (unsuppSection) {
			unsuppSection.hidden = true;
		}
	}

	// Error & import button state
	const errorEl = $("#proxy-discovery-error");
	if (errorEl) {
		errorEl.hidden = true;
		errorEl.textContent = "";
	}
	const importBtn = $("#proxy-discovery-import-btn");
	if (importBtn) {
		importBtn.disabled = !detected.canImport || !detected.importSettings;
		importBtn.textContent = "Import Settings";
	}

	showDialog("#proxy-discovery-dialog");
}

let optionsConnectionOverrides = [];

function renderSiteOverrides() {
	const container = $("#site-overrides-list");
	if (!container) return;
	container.innerHTML = "";
	if (!Array.isArray(optionsConnectionOverrides) || optionsConnectionOverrides.length === 0) {
		const empty = document.createElement("div");
		empty.className = "field-help";
		empty.textContent = "No site exceptions configured.";
		container.appendChild(empty);
		return;
	}
	optionsConnectionOverrides.forEach((rule, index) => {
		const row = document.createElement("div");
		row.className = "site-override-row";

		const patternSpan = document.createElement("span");
		patternSpan.className = "site-override-pattern";
		patternSpan.textContent = rule.hostPattern;

		const connsSpan = document.createElement("span");
		connsSpan.className = "site-override-conns";
		connsSpan.textContent = `${rule.connections} conn${rule.connections === 1 ? "" : "s"}`;

		const removeBtn = document.createElement("button");
		removeBtn.type = "button";
		removeBtn.className = "small-button remove-override-button";
		removeBtn.dataset.index = index;
		removeBtn.textContent = "Remove";

		row.appendChild(patternSpan);
		row.appendChild(connsSpan);
		row.appendChild(removeBtn);
		container.appendChild(row);
	});
}

function updateWindowLimitFieldsState() {
	const enabled = $("#window-limit-enabled")?.checked === true;
	const container = $("#window-limit-fields");
	if (container) {
		container.classList.toggle("disabled", !enabled);
	}
	if ($("#window-limit-max-mb")) $("#window-limit-max-mb").disabled = !enabled;
	if ($("#window-limit-hours")) $("#window-limit-hours").disabled = !enabled;
}

function openSettings() {
	const settings = state.settings;
	if ($("#app-theme")) {
		$("#app-theme").value = settings.theme || "system";
	}
	optionsCategoryDirectories = { ...(settings.categoryDirectories || {}) };
	const currentCat = $("#options-category-select")?.value || "Archives";
	if ($("#category-directory")) {
		$("#category-directory").value = optionsCategoryDirectories[currentCat] || "";
	}
	if ($("#temporary-directory")) {
		$("#temporary-directory").value = settings.temporaryDirectory || "";
	}
	$("#start-with-windows").checked = settings.startWithWindows !== false;
	const clipboardMonitoring = settings.clipboardMonitoring || {};
	if ($("#clipboard-monitoring-enabled")) {
		$("#clipboard-monitoring-enabled").checked = clipboardMonitoring.enabled === true;
		$("#clipboard-auto-enqueue").checked = clipboardMonitoring.autoEnqueue === true;
		$("#clipboard-auto-enqueue").disabled = !$("#clipboard-monitoring-enabled").checked;
	}
	$("#default-directory").value = settings.defaultDirectory || "";
	$("#max-concurrent").value = settings.maxConcurrent || 3;
	$("#connections").value = settings.connections || 8;
	$("#speed-limit").value = settings.speedLimitKbps || 0;

	optionsConnectionOverrides = Array.isArray(settings.connectionOverrides)
		? [...settings.connectionOverrides]
		: [];
	renderSiteOverrides();

	const windowLimit = settings.windowLimit || {};
	if ($("#window-limit-enabled")) {
		$("#window-limit-enabled").checked = windowLimit.enabled === true;
		$("#window-limit-max-mb").value = windowLimit.maxBytes
			? Math.round(windowLimit.maxBytes / (1024 * 1024))
			: 2048;
		$("#window-limit-hours").value = windowLimit.windowHours || 12;
		updateWindowLimitFieldsState();

		const statusEl = $("#window-limit-status");
		if (statusEl) {
			if (windowLimit.enabled) {
				const currentMb = ((windowLimit.currentBytes || 0) / (1024 * 1024)).toFixed(1);
				const maxMb = ((windowLimit.maxBytes || 0) / (1024 * 1024)).toFixed(0);
				let text = `Current usage: ${currentMb} / ${maxMb} MB in rolling window.`;
				if (windowLimit.stoppedUntil) {
					text += ` (Quota exceeded; auto-resumes at ${windowLimit.stoppedUntil})`;
				}
				statusEl.textContent = text;
			} else {
				statusEl.textContent = "";
			}
		}
	}

	const proxy = settings.proxy || {};
	$("#proxy-enabled").checked = proxy.enabled === true;
	$("#proxy-protocol").value = proxy.protocol || "http";
	$("#proxy-host").value = proxy.host || "";
	$("#proxy-port").value = proxy.port || 8080;
	$("#proxy-username").value = proxy.username || "";
	$("#proxy-password").value = proxy.password || "";
	if ($("#proxy-resolve-dns-remotely")) {
		$("#proxy-resolve-dns-remotely").checked = proxy.resolveDnsRemotely === true;
	}
	updateProxyFieldsState();

	const capturePolicy = settings.capturePolicy || {};
	$("#capture-enabled").checked = capturePolicy.enabled !== false;
	$("#capture-mode").value = capturePolicy.confirmationMode || "askForUnknown";
	$("#capture-file-types").value = Array.isArray(capturePolicy.fileTypes) ? capturePolicy.fileTypes.join(" ").toUpperCase() : "";
	$("#capture-excluded-hosts").value = Array.isArray(capturePolicy.excludedHosts) ? capturePolicy.excludedHosts.join(", ") : "";
	$("#capture-excluded-addresses").value = Array.isArray(capturePolicy.excludedAddresses) ? capturePolicy.excludedAddresses.join(", ") : "";

	const actions = Array.isArray(settings.completionActions) ? settings.completionActions : [];
	const soundAction = actions.find((a) => a.type === "playSound");
	$("#settings-play-sound").checked = Boolean(soundAction);
	$("#settings-sound-file").value = soundAction?.soundFile || "";

	const scanAction = actions.find((a) => a.type === "antivirusScan");
	$("#settings-scan-antivirus").checked = Boolean(scanAction);
	$("#settings-scanner-executable").value = scanAction?.executable || "";
	$("#settings-scanner-args").value = Array.isArray(scanAction?.arguments) ? scanAction.arguments.join(" ") : "";

	const cmdAction = actions.find((a) => a.type === "executeCommand");
	$("#settings-post-command").checked = Boolean(cmdAction);
	$("#settings-command-executable").value = cmdAction?.executable || "";
	$("#settings-command-args").value = Array.isArray(cmdAction?.arguments) ? cmdAction.arguments.join(" ") : "";

	const powerAction = actions.find((a) => a.type === "powerAction");
	$("#settings-power-action").value = powerAction?.kind || "none";
	$("#settings-power-confirm").checked = powerAction ? powerAction.confirmed !== false : true;

	updateProxyFieldsState();
	resetProxyTestStatus();

	$("#settings-error").hidden = true;
	refreshMediaToolsStatus();
	refreshCredentialRules();
	showDialog("#settings-dialog");
}
async function refreshCredentialRules() {
	if (!api.listCredentialRules) return;
	const listEl = $("#credentials-list");
	if (!listEl) return;
	try {
		const rules = await api.listCredentialRules();
		listEl.innerHTML = "";
		if (!rules || rules.length === 0) {
			const tr = document.createElement("tr");
			tr.innerHTML = '<td colspan="7" style="text-align:center; color: var(--text-muted, #888); padding: 8px;">No stored site credentials.</td>';
			listEl.appendChild(tr);
			return;
		}
		for (const rule of rules) {
			const tr = document.createElement("tr");
			tr.style.borderBottom = "1px solid var(--border-color, #e5e7eb)";

			const tdScheme = document.createElement("td");
			tdScheme.style.padding = "4px";
			tdScheme.textContent = rule.scheme;

			const tdHost = document.createElement("td");
			tdHost.style.padding = "4px";
			tdHost.style.fontWeight = "600";
			tdHost.textContent = rule.host;

			const tdPort = document.createElement("td");
			tdPort.style.padding = "4px";
			tdPort.textContent = rule.port ? String(rule.port) : "default";

			const tdPath = document.createElement("td");
			tdPath.style.padding = "4px";
			tdPath.textContent = rule.pathPrefix;

			const tdUser = document.createElement("td");
			tdUser.style.padding = "4px";
			tdUser.textContent = rule.username;

			const tdPass = document.createElement("td");
			tdPass.style.padding = "4px";
			tdPass.textContent = "******";

			const tdAction = document.createElement("td");
			tdAction.style.padding = "4px";
			const delBtn = document.createElement("button");
			delBtn.type = "button";
			delBtn.className = "small-button delete-cred-btn";
			delBtn.textContent = "Delete";
			delBtn.style.color = "#dc2626";
			delBtn.dataset.id = rule.id;
			tdAction.appendChild(delBtn);

			tr.appendChild(tdScheme);
			tr.appendChild(tdHost);
			tr.appendChild(tdPort);
			tr.appendChild(tdPath);
			tr.appendChild(tdUser);
			tr.appendChild(tdPass);
			tr.appendChild(tdAction);

			listEl.appendChild(tr);
		}
	} catch (e) {
		console.error("Could not refresh credentials list:", e);
	}
}
async function refreshMediaToolsStatus() {
	if (!api.getMediaToolsStatus) return;
	try {
		const statuses = await api.getMediaToolsStatus();
		for (const st of statuses) {
			const type = st.toolType || st.tool_type;
			const isAvail = st.isAvailable || st.is_available;
			const src = st.source || "";
			const label = $(`#${type}-status`);
			const btn = $(`#install-${type}-btn`);
			if (label) {
				if (isAvail) {
					label.textContent = `Installed (${src || "ready"})`;
					label.style.color = "#166534";
				} else {
					label.textContent = "Not installed";
					label.style.color = "#991b1b";
				}
			}
			if (btn) {
				btn.textContent = isAvail ? "Reinstall" : "Install";
				btn.disabled = false;
			}
		}
	} catch (e) {
		console.error("Could not refresh media tools status:", e);
	}
}
async function installMediaToolAction(tool) {
	const btn = $(`#install-${tool}-btn`);
	const label = $(`#${tool}-status`);
	if (btn) { btn.disabled = true; btn.textContent = "Installing…"; }
	if (label) { label.textContent = "Downloading & installing…"; label.style.color = "#0369a1"; }
	try {
		await api.installMediaTool(tool);
		notify(`${tool} installed successfully.`);
		await refreshMediaToolsStatus();
	} catch (err) {
		notify(`Failed to install ${tool}: ${messageOf(err)}`, true);
		if (btn) btn.disabled = false;
		await refreshMediaToolsStatus();
	}
}
let activeMediaToolDownloadId = null;

function openMediaToolDialog(tool = "ffmpeg", downloadId = null) {
	activeMediaToolDownloadId = downloadId || state.selectedId || null;
	const dialog = $("#media-tool-dialog");
	if (!dialog) return;

	const upper = tool.toUpperCase();
	$("#media-tool-title").textContent = tool === "ffmpeg" ? "FFmpeg Required" : `${upper} Required`;
	$("#media-tool-dialog-heading").textContent = `${upper} is required to combine video and audio`;
	$("#media-tool-confirm-btn").textContent = `Download & Install ${upper} Now`;
	$("#media-tool-confirm-btn").disabled = false;
	$("#media-tool-cancel-btn").disabled = false;
	$("#media-tool-dialog-progress").hidden = true;
	$("#media-tool-dialog-error").hidden = true;

	showDialog("#media-tool-dialog");
}

let activeErrorDownloadId = null;

function openErrorDialog(downloadId = null) {
	const targetId = downloadId || state.selectedId;
	const d = state.downloads.find((item) => item.id === targetId);
	if (!d) return;

	activeErrorDownloadId = d.id;
	const errorDialog = $("#error-dialog");
	if (!errorDialog) return;

	const errText = d.error || "The download encountered an unrecoverable error.";
	const isFfmpeg = /ffmpeg/i.test(errText);
	const isYtdlp = /yt-dlp/i.test(errText);
	const isPremature = /prematurely|interrupted|truncated|stream ended/i.test(errText);

	let title = "Download Error";
	let heading = "Download could not be completed";
	let suggestion = "";
	let actionText = "";
	let onAction = null;

	if (isFfmpeg) {
		title = "Media Assembly Failed";
		heading = "FFmpeg is required to combine video and audio tracks";
		suggestion = "Open Media Tools in Options to install or reinstall FFmpeg.";
		actionText = "Open Media Tools";
		onAction = () => {
			hideDialog("#error-dialog");
			openSettings();
		};
	} else if (isYtdlp) {
		title = "YouTube Tool Required";
		heading = "yt-dlp is required to process this media stream";
		suggestion = "Install yt-dlp from Options > Media Tools.";
		actionText = "Open Media Tools";
		onAction = () => {
			hideDialog("#error-dialog");
			openSettings();
		};
	} else if (isPremature) {
		title = "Stream Interrupted";
		heading = "The connection was interrupted before all data was received";
		suggestion = "Click Retry to re-connect and resume downloading the remaining data.";
		actionText = "Retry Now";
		onAction = () => {
			hideDialog("#error-dialog");
			performAction(d, "resume");
		};
	} else if (/expired|HTTP 401|HTTP 403|HTTP 410/i.test(errText)) {
		title = "Link or Session Expired";
		heading = "The download link expired or authentication token timed out";
		suggestion = "Open the media page in your browser to refresh the link automatically, or click Resume after refreshing.";
		actionText = "Resume Transfer";
		onAction = () => {
			hideDialog("#error-dialog");
			performAction(d, "resume");
		};
	} else if (/proxy/i.test(errText)) {
		title = "Proxy Connection Failed";
		heading = "Could not reach the host through the configured proxy";
		suggestion = "Check your proxy host, port, and credentials in Options.";
		actionText = "Check Proxy Settings";
		onAction = () => {
			hideDialog("#error-dialog");
			openSettings();
		};
	}

	$("#error-dialog-title").textContent = title;
	$("#error-dialog-heading").textContent = heading;
	$("#error-dialog-description").textContent = errText;

	const suggEl = $("#error-dialog-fix-suggestion");
	const suggTextEl = $("#error-dialog-suggestion-text");
	const suggActionBtn = $("#error-dialog-suggestion-action");
	if (suggEl && suggestion) {
		suggEl.hidden = false;
		suggTextEl.textContent = suggestion;
		if (actionText && onAction) {
			suggActionBtn.textContent = actionText;
			suggActionBtn.onclick = onAction;
			suggActionBtn.hidden = false;
		} else {
			suggActionBtn.hidden = true;
		}
	} else if (suggEl) {
		suggEl.hidden = true;
	}

	// Technical details
	const techPre = $("#error-dialog-tech-text");
	const diagnosticData = [
		`File: ${d.filename || "unknown"}`,
		`URL: ${redactUrl(d.url) || "unknown"}`,
		`Kind: ${d.kind || "standard"}`,
		`Downloaded: ${d.downloaded || 0} bytes`,
		`Total: ${d.total || "unknown"} bytes`,
		`Status: ${d.status || "error"}`,
		`Error: ${errText}`,
		`Timestamp: ${new Date().toISOString()}`,
	].join("\n");
	if (techPre) {
		techPre.textContent = diagnosticData;
	}

	const copyBtn = $("#error-dialog-copy-btn");
	if (copyBtn) {
		copyBtn.onclick = async () => {
			try {
				if (navigator.clipboard) {
					await navigator.clipboard.writeText(diagnosticData);
				}
				notify("Diagnostic details copied to clipboard.");
			} catch (_) {
				notify("Could not copy to clipboard.", true);
			}
		};
	}

	const retryBtn = $("#error-dialog-retry-btn");
	if (retryBtn) {
		retryBtn.onclick = () => {
			hideDialog("#error-dialog");
			performAction(d, "resume");
		};
	}

	showDialog("#error-dialog");
}
let cachedQueues = [];

async function refreshQueuesUI() {
	try {
		cachedQueues = await call("listQueues");
	} catch (e) {
		console.warn("Failed to load queues:", e);
		cachedQueues = [];
	}

	// Populate Add Dialog select
	const queueSelect = $("#download-queue");
	if (queueSelect && Array.isArray(cachedQueues)) {
		const prevVal = queueSelect.value || "default";
		queueSelect.innerHTML = "";
		for (const q of cachedQueues) {
			const opt = document.createElement("option");
			opt.value = q.id;
			opt.textContent = q.name + (q.id === "default" ? " (Default)" : "");
			queueSelect.appendChild(opt);
		}
		if ([...queueSelect.options].some((o) => o.value === prevVal)) {
			queueSelect.value = prevVal;
		}
	}

	// Populate Scheduler queues panel
	const container = $("#queues-list-container");
	if (!container) return;
	container.innerHTML = "";
	if (!Array.isArray(cachedQueues) || cachedQueues.length === 0) {
		container.innerHTML = "<p class='field-help'>No queues available.</p>";
		return;
	}

	for (const q of cachedQueues) {
		const card = document.createElement("div");
		card.className = "queue-card";

		const info = document.createElement("div");
		info.className = "queue-card-info";

		const header = document.createElement("div");
		header.className = "queue-card-header";
		const nameSpan = document.createElement("span");
		nameSpan.textContent = q.name + (q.id === "default" ? " (Default)" : "");
		const badge = document.createElement("span");
		badge.className = `queue-badge ${q.running ? "running" : "paused"}`;
		badge.textContent = q.running ? "Running" : "Paused";
		header.append(nameSpan, badge);

		const meta = document.createElement("div");
		meta.className = "queue-card-meta";
		const itemCount = Array.isArray(q.itemIds) ? q.itemIds.length : 0;
		const activeLimit = `Max ${q.concurrencyLimit} active`;
		const speed =
			q.speedLimitKbps > 0 ? `${q.speedLimitKbps} KB/s` : "Unlimited speed";
		let schedInfo = "No schedule";
		if (q.schedule && q.schedule.enabled) {
			schedInfo = `Schedule: ${q.schedule.startTime}${q.schedule.stopTime ? " - " + q.schedule.stopTime : ""}`;
			if (q.schedule.nextRunAt) {
				schedInfo += ` (Next: ${date(q.schedule.nextRunAt)})`;
			}
		}
		meta.textContent = `${itemCount} downloads • ${activeLimit} • ${speed} • ${schedInfo}`;
		info.append(header, meta);

		const actions = document.createElement("div");
		actions.className = "queue-card-actions";

		const toggleBtn = document.createElement("button");
		toggleBtn.type = "button";
		toggleBtn.className = "small-button";
		toggleBtn.textContent = q.running ? "Pause" : "Start";
		toggleBtn.onclick = async () => {
			if (q.running) {
				await call("pauseQueue", { id: q.id });
			} else {
				await call("startQueue", { id: q.id });
			}
			await refreshQueuesUI();
			await refresh();
		};
		actions.appendChild(toggleBtn);

		const editBtn = document.createElement("button");
		editBtn.type = "button";
		editBtn.className = "small-button";
		editBtn.textContent = "Edit…";
		editBtn.onclick = () => openQueueEditDialog(q);
		actions.appendChild(editBtn);

		if (q.id !== "default") {
			const deleteBtn = document.createElement("button");
			deleteBtn.type = "button";
			deleteBtn.className = "small-button";
			deleteBtn.textContent = "Delete";
			deleteBtn.onclick = async () => {
				if (
					confirm(
						`Delete queue "${q.name}"? Downloads will be moved to the Default Queue.`,
					)
				) {
					await call("deleteQueue", {
						id: q.id,
						disposition: "moveToDefault",
					});
					await refreshQueuesUI();
					await refresh();
				}
			};
			actions.appendChild(deleteBtn);
		}

		card.append(info, actions);
		container.appendChild(card);
	}
}

function openQueueEditDialog(queue = null) {
	const errorEl = $("#queue-edit-error");
	if (errorEl) {
		errorEl.hidden = true;
		errorEl.textContent = "";
	}
	$("#queue-edit-id").value = queue ? queue.id : "";
	$("#queue-edit-title").textContent = queue
		? `Edit Queue: ${queue.name}`
		: "Create Download Queue";
	$("#queue-edit-name").value = queue ? queue.name : "";
	$("#queue-edit-name").disabled = queue ? queue.id === "default" : false;
	$("#queue-edit-concurrency").value = queue ? queue.concurrencyLimit : 2;
	$("#queue-edit-speed").value = queue ? queue.speedLimitKbps : 0;

	const sched = queue ? queue.schedule : null;
	const schedEnabled = Boolean(sched && sched.enabled);
	$("#queue-edit-schedule-enabled").checked = schedEnabled;
	const schedFields = $("#queue-schedule-fields");
	if (schedFields) schedFields.hidden = !schedEnabled;

	if (sched) {
		$("#queue-edit-start-time").value = sched.startTime || "22:00";
		$("#queue-edit-stop-time").value = sched.stopTime || "06:00";
		$("#queue-edit-missed-policy").value =
			sched.missedRunPolicy || "runOnStartup";
		const days = Array.isArray(sched.daysOfWeek)
			? sched.daysOfWeek
			: [1, 2, 3, 4, 5, 6, 7];
		$$("input[name=q-day]").forEach((cb) => {
			cb.checked = days.includes(Number(cb.value));
		});
	} else {
		$("#queue-edit-start-time").value = "22:00";
		$("#queue-edit-stop-time").value = "06:00";
		$("#queue-edit-missed-policy").value = "runOnStartup";
		$$("input[name=q-day]").forEach((cb) => {
			cb.checked = [1, 2, 3, 4, 5].includes(Number(cb.value));
		});
	}

	showDialog("#queue-edit-dialog");
}

function renderScheduler() {
	refreshQueuesUI();
	const downloads = state.downloads
		.filter((d) => statusOf(d) === "scheduled")
		.sort((a, b) => new Date(a.scheduledAt) - new Date(b.scheduledAt));
	const list = $("#scheduled-list");
	list.replaceChildren();
	if (!downloads.length)
		list.append(element("p", "field-help", "No scheduled downloads."));
	for (const d of downloads) {
		const row = element("div", "scheduled-item");
		const name = element("span", "", d.filename);
		name.title = d.filename;
		row.append(name, element("time", "", date(d.scheduledAt)));
		list.append(row);
	}
}
function openScheduler() {
	renderScheduler();
	showDialog("#scheduler-dialog");
}
async function openIntegration() {
	renderBrowserGrid(state.integration);
	showDialog("#integration-dialog");
}
const commands = {
	add: () => openAdd(),
	paste: async () => {
		try {
			await openAddFromClipboard();
		} catch (error) {
			notify(messageOf(error), true);
		}
	},
	find: () => $("#search").focus(),
	settings: openSettings,
	integration: openIntegration,
	scheduler: openScheduler,
	pauseAll: () => bulkAction("pause"),
	resumeAll: () => bulkAction("resume"),
	removeCompleted,
};
commands.find = () => {
	$(".list-search").classList.add("visible");
	$("#search").focus();
};
commands.details = () => {
	if (selectedId) showDetails(selectedId);
};
commands.remove = () => performAction(selected(), "remove");
commands.all = () => setView("all");
document.addEventListener("keydown", (event) => {
	if (event.ctrlKey && event.key.toLowerCase() === "f") commands.find();
	if (event.key === "Escape") $(".list-search").classList.remove("visible");
});
$("#new-download").addEventListener("click", commands.add);
$("#settings-button").addEventListener("click", openSettings);
$("#integration-button").addEventListener("click", openIntegration);
$("#scheduler-button").addEventListener("click", openScheduler);
$("#pause-all").addEventListener("click", commands.pauseAll);
$("#stop-queue").addEventListener("click", commands.pauseAll);
$("#resume-all").addEventListener("click", commands.resumeAll);
$("#remove-completed").addEventListener("click", removeCompleted);
$("#resume-selected").addEventListener("click", () => {
	const d = selected();
	if (d?.needsReview) {
		openReview(d.id);
		return;
	}
	performAction(
		d,
		d && ["failed", "cancelled"].includes(statusOf(d)) ? "retry" : "resume",
	);
});
$("#stop-selected").addEventListener("click", () =>
	performAction(selected(), "pause"),
);
$("#remove-selected").addEventListener("click", () =>
	performAction(selected(), "remove"),
);
$("#open-selected").addEventListener("click", () =>
	performAction(selected(), "open"),
);
$("#reveal-selected").addEventListener("click", () =>
	performAction(selected(), "reveal"),
);
$("#details-selected").addEventListener("click", () => {
	if (selectedId) showDetails(selectedId);
});
const detailDownload = () => state.downloads.find((d) => d.id === detailId);
$("#detail-open").addEventListener("click", () =>
	performAction(detailDownload(), "open"),
);
$("#detail-folder").addEventListener("click", () =>
	performAction(detailDownload(), "reveal"),
);
$("#detail-retry-actions")?.addEventListener("click", async () => {
	if (!detailId) return;
	try {
		$("#detail-retry-actions").disabled = true;
		await call("retryDownloadActions", { id: detailId });
		await refresh();
		renderDetails();
		notify("Post-download actions re-executed.");
	} catch (e) {
		notify(messageOf(e), true);
	} finally {
		$("#detail-retry-actions").disabled = false;
	}
});
$("#detail-stop").addEventListener("click", () =>
	performAction(detailDownload(), "pause"),
);
$("#detail-cancel").addEventListener("click", () =>
	performAction(detailDownload(), "cancel"),
);
$("#detail-resume").addEventListener("click", () => {
	const d = detailDownload();
	performAction(
		d,
		d && ["failed", "cancelled"].includes(statusOf(d)) ? "retry" : "resume",
	);
});
$("#toggle-details").addEventListener("click", async () => {
	const details = $("#connection-details");
	details.hidden = !details.hidden;
	document.body.classList.toggle("details-collapsed", details.hidden);
	$("#toggle-details").textContent = details.hidden
		? ">> Show details"
		: "<< Hide details";
	if (progressMode)
		try {
			await call("resizeProgress", { expanded: !details.hidden });
		} catch (error) {
			notify(messageOf(error), true);
		}
});
for (const tab of ["status", "speed", "completion"])
	$(`#tab-${tab}`).addEventListener("click", () => setDetailTab(tab));
$("#detail-speed-form").addEventListener("submit", async (event) => {
	event.preventDefault();
	$("#detail-speed-error").hidden = true;
	try {
		await call("settings", {
			speedLimitKbps: Number($("#detail-speed-limit").value),
		});
		await refresh();
		notify("Global speed limit saved.");
	} catch (error) {
		$("#detail-speed-error").textContent = messageOf(error);
		$("#detail-speed-error").hidden = false;
	}
});
$("#completion-form").addEventListener("submit", async (event) => {
	event.preventDefault();
	try {
		await call("settings", {
			notifyOnComplete: $("#notify-complete").checked,
			revealOnComplete: $("#reveal-complete").checked,
		});
		await refresh();
		$("#completion-result").textContent = " Saved.";
	} catch (error) {
		$("#completion-result").textContent = ` ${messageOf(error)}`;
	}
});
$("#scheduler-start").addEventListener("click", commands.resumeAll);
$("#scheduler-stop").addEventListener("click", commands.pauseAll);
$("#tab-scheduler-queues")?.addEventListener("click", () => {
	$("#tab-scheduler-queues").classList.add("active-tab");
	$("#tab-scheduler-queues").setAttribute("aria-selected", "true");
	$("#tab-scheduler-oneoff")?.classList.remove("active-tab");
	$("#tab-scheduler-oneoff")?.setAttribute("aria-selected", "false");
	const qPanel = $("#scheduler-queues-panel");
	if (qPanel) qPanel.hidden = false;
	const oPanel = $("#scheduler-oneoff-panel");
	if (oPanel) oPanel.hidden = true;
	refreshQueuesUI();
});
$("#tab-scheduler-oneoff")?.addEventListener("click", () => {
	$("#tab-scheduler-oneoff").classList.add("active-tab");
	$("#tab-scheduler-oneoff").setAttribute("aria-selected", "true");
	$("#tab-scheduler-queues")?.classList.remove("active-tab");
	$("#tab-scheduler-queues")?.setAttribute("aria-selected", "false");
	const oPanel = $("#scheduler-oneoff-panel");
	if (oPanel) oPanel.hidden = false;
	const qPanel = $("#scheduler-queues-panel");
	if (qPanel) qPanel.hidden = true;
});
$("#queue-create-btn")?.addEventListener("click", () => {
	openQueueEditDialog(null);
});
$("#queue-edit-close-btn")?.addEventListener("click", () => {
	$("#queue-edit-dialog")?.close();
});
$("#queue-edit-cancel-btn")?.addEventListener("click", () => {
	$("#queue-edit-dialog")?.close();
});
$("#queue-edit-schedule-enabled")?.addEventListener("change", (e) => {
	const fields = $("#queue-schedule-fields");
	if (fields) fields.hidden = !e.target.checked;
});
$("#queue-edit-form")?.addEventListener("submit", async (e) => {
	e.preventDefault();
	const errorEl = $("#queue-edit-error");
	if (errorEl) {
		errorEl.hidden = true;
		errorEl.textContent = "";
	}
	const id = $("#queue-edit-id")?.value.trim() || null;
	const name = $("#queue-edit-name")?.value.trim() || "";
	const concurrencyLimit = Number($("#queue-edit-concurrency")?.value || 2);
	const speedLimitKbps = Number($("#queue-edit-speed")?.value || 0);

	const schedEnabled = $("#queue-edit-schedule-enabled")?.checked;
	let schedule = null;
	if (schedEnabled) {
		const startTime = $("#queue-edit-start-time")?.value || "22:00";
		const stopTime = $("#queue-edit-stop-time")?.value || null;
		const missedRunPolicy = $("#queue-edit-missed-policy")?.value || "runOnStartup";
		const days = [];
		$$("input[name=q-day]:checked").forEach((cb) => {
			days.push(Number(cb.value));
		});
		schedule = {
			enabled: true,
			startTime,
			stopTime: stopTime || null,
			daysOfWeek: days.length > 0 ? days : [1, 2, 3, 4, 5, 6, 7],
			missedRunPolicy,
		};
	}

	try {
		if (id) {
			await call("updateQueue", {
				id,
				name,
				concurrencyLimit,
				speedLimitKbps,
				schedule,
			});
		} else {
			await call("createQueue", {
				name,
				concurrencyLimit,
				speedLimitKbps,
				schedule,
			});
		}
		$("#queue-edit-dialog")?.close();
		await refreshQueuesUI();
		await refresh();
	} catch (err) {
		if (errorEl) {
			errorEl.textContent = messageOf(err);
			errorEl.hidden = false;
		}
	}
});
$("#schedule-new").addEventListener("click", () => {
	const future = new Date(Date.now() + 3600000),
		scheduled = new Date(
			future.getTime() - future.getTimezoneOffset() * 60000,
		)
			.toISOString()
			.slice(0, 16);
	$("#scheduler-dialog").close();
	openAdd("", scheduled);
});
$("#grabber-button").addEventListener("click", () =>
	notify(
		"Website grabber projects are not available in this build. Use Add URL to add direct download links.",
	),
);
$$("[data-filter]").forEach((button) =>
	button.addEventListener("click", () => setView(button.dataset.filter)),
);
$$("[data-category]").forEach((button) =>
	button.addEventListener("click", () =>
		setView("all", button.dataset.category),
	),
);
$$("[data-sort]").forEach((button) =>
	button.addEventListener("click", () => {
		const next = button.dataset.sort;
		ascending = sort === next ? !ascending : true;
		sort = next;
		render();
	}),
);
$("#search").addEventListener("input", (event) => {
	searchTerm = event.target.value.trim().toLowerCase();
	render();
});
const categoryExpander = $(".tree-expander[aria-controls]");
function setCategoryTreeExpanded(expanded) {
	const children = $(`#${categoryExpander.getAttribute("aria-controls")}`);
	children.hidden = !expanded;
	categoryExpander.textContent = expanded ? "⊟" : "⊞";
	categoryExpander.setAttribute("aria-expanded", String(expanded));
	categoryExpander.setAttribute(
		"aria-label",
		`${expanded ? "Collapse" : "Expand"} All Downloads categories`,
	);
}
categoryExpander.addEventListener("click", (event) => {
	event.preventDefault();
	event.stopPropagation();
	setCategoryTreeExpanded(
		categoryExpander.getAttribute("aria-expanded") !== "true",
	);
});
categoryExpander.addEventListener("keydown", (event) => {
	if (!["Enter", " "].includes(event.key)) return;
	event.preventDefault();
	event.stopPropagation();
	setCategoryTreeExpanded(
		categoryExpander.getAttribute("aria-expanded") !== "true",
	);
});
const tableScroll = $("#table-scroll"),
	selectionRectangle = $("#selection-rectangle");
const tablePoint = (event) => {
	const bounds = tableScroll.getBoundingClientRect();
	return {
		x: event.clientX - bounds.left + tableScroll.scrollLeft,
		y: event.clientY - bounds.top + tableScroll.scrollTop,
	};
};
function initializeColumnResizing() {
	const table = $(".downloads-table"),
		columns = $$(".downloads-table col"),
		headers = $$(".downloads-table thead th");
	headers.forEach((header, index) => {
		const handle = element("span", "column-resizer");
		handle.setAttribute("aria-hidden", "true");
		header.append(handle);
		handle.addEventListener("pointerdown", (event) => {
			if (event.button !== 0) return;
			event.preventDefault();
			event.stopPropagation();
			const startX = event.clientX,
				startWidth = header.getBoundingClientRect().width,
				startTableWidth = table.getBoundingClientRect().width;
			handle.setPointerCapture(event.pointerId);
			document.body.classList.add("resizing-columns");
			const move = (moveEvent) => {
				const width = Math.max(
						index === 1 ? 20 : 45,
						startWidth + moveEvent.clientX - startX,
					),
					delta = width - startWidth;
				columns[index].style.width = `${width}px`;
				table.style.width = `${Math.max(735, startTableWidth + delta)}px`;
				table.style.minWidth = table.style.width;
			};
			const finish = (finishEvent) => {
				try {
					handle.releasePointerCapture(finishEvent.pointerId);
				} catch {}
				handle.removeEventListener("pointermove", move);
				handle.removeEventListener("pointerup", finish);
				handle.removeEventListener("pointercancel", finish);
				document.body.classList.remove("resizing-columns");
			};
			handle.addEventListener("pointermove", move);
			handle.addEventListener("pointerup", finish);
			handle.addEventListener("pointercancel", finish);
		});
	});
}
initializeColumnResizing();
tableScroll.addEventListener("pointerdown", (event) => {
	if (
		event.button !== 0 ||
		event.target.closest("thead") ||
		event.target.closest("button")
	)
		return;
	// A click synthesized for the pointer-up that ended a marquee is ignored.
	// A later, new pointer-down must always restore ordinary single-click use.
	suppressRowClick = false;
	tableScroll.focus({ preventScroll: true });
	const point = tablePoint(event),
		row = event.target.closest(".download-row");
	const additive = event.ctrlKey || event.metaKey,
		range = event.shiftKey;
	selectionDrag = {
		pointerId: event.pointerId,
		startX: point.x,
		startY: point.y,
		row,
		additive,
		range,
		base: additive || range ? new Set(selectedIds) : new Set(),
		moved: false,
	};
	tableScroll.setPointerCapture(event.pointerId);
});
tableScroll.addEventListener("pointermove", (event) => {
	if (!selectionDrag || selectionDrag.pointerId !== event.pointerId) return;
	const point = tablePoint(event),
		dx = point.x - selectionDrag.startX,
		dy = point.y - selectionDrag.startY;
	if (!selectionDrag.moved && Math.hypot(dx, dy) < 4) return;
	selectionDrag.moved = true;
	const left = Math.min(selectionDrag.startX, point.x),
		top = Math.min(selectionDrag.startY, point.y),
		right = Math.max(selectionDrag.startX, point.x),
		bottom = Math.max(selectionDrag.startY, point.y);
	Object.assign(selectionRectangle.style, {
		left: `${left}px`,
		top: `${top}px`,
		width: `${Math.max(1, right - left)}px`,
		height: `${Math.max(1, bottom - top)}px`,
	});
	selectionRectangle.hidden = false;
	selectedIds.clear();
	for (const id of selectionDrag.base) selectedIds.add(id);
	const viewport = tableScroll.getBoundingClientRect();
	for (const row of $$(".download-row")) {
		const rect = row.getBoundingClientRect(),
			rowLeft = rect.left - viewport.left + tableScroll.scrollLeft,
			rowTop = rect.top - viewport.top + tableScroll.scrollTop;
		if (
			rowLeft < right &&
			rowLeft + rect.width > left &&
			rowTop < bottom &&
			rowTop + rect.height > top
		)
			selectedIds.add(row.dataset.id);
	}
	selectedId = selectedIds.values().next().value || null;
	paintSelection();
	event.preventDefault();
});
function finishMarquee(event) {
	if (!selectionDrag || selectionDrag.pointerId !== event.pointerId) return;
	const { moved, row, additive, range } = selectionDrag;
	try {
		tableScroll.releasePointerCapture(event.pointerId);
	} catch {}
	selectionDrag = null;
	selectionRectangle.hidden = true;
	if (moved) {
		if (row) selectionAnchorId = row.dataset.id;
	} else if (row)
		selectDownload(row.dataset.id, {
			additive,
			range,
			toggle: additive && !range,
		});
	else selectDownload(null);
	suppressRowClick = true;
	setTimeout(() => {
		suppressRowClick = false;
	}, 0);
}
tableScroll.addEventListener("pointerup", finishMarquee);
tableScroll.addEventListener("pointercancel", finishMarquee);
$("#table-scroll").addEventListener("keydown", (event) => {
	if (!visibleDownloads.length) return;
	const index = visibleDownloads.findIndex((d) => d.id === selectedId);
	if (["ArrowDown", "ArrowUp", "Home", "End"].includes(event.key)) {
		event.preventDefault();
		const next =
			event.key === "Home"
				? 0
				: event.key === "End"
					? visibleDownloads.length - 1
					: Math.max(
							0,
							Math.min(
								visibleDownloads.length - 1,
								index + (event.key === "ArrowDown" ? 1 : -1),
							),
						);
		selectDownload(visibleDownloads[next].id, { range: event.shiftKey });
		$$(".download-row")
			.find((row) => row.dataset.id === selectedId)
			?.scrollIntoView({ block: "nearest" });
	}
	if (event.key === "Enter" && selectedId) showDetails(selectedId);
	if (event.key === "Delete") performAction(selected(), "remove");
});
$("#dismiss-toast").addEventListener("click", () => {
	$("#toast").hidden = true;
});
$$("[data-close]").forEach((button) =>
	button.addEventListener("click", () => {
		const dialog = button.closest("dialog");
		if (
			(progressMode && dialog.id === "add-dialog") ||
			(addMode && dialog.id === "add-dialog") ||
			(optionsMode && dialog.id === "settings-dialog") ||
			(schedulerMode && dialog.id === "scheduler-dialog")
		) {
			call("closeProgress").catch((error) =>
				notify(messageOf(error), true),
			);
			return;
		}
		dialog.close();
		if (dialog.id === "add-dialog") dismissReview();
	}),
);
$("#add-dialog").addEventListener("cancel", (event) => {
	if (progressMode || addMode) {
		event.preventDefault();
		call("closeProgress").catch((error) => notify(messageOf(error), true));
	} else dismissReview();
});
document.addEventListener("keydown", (event) => {
	if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "n") {
		event.preventDefault();
		if (!$$("dialog[open]").length) openAdd();
	}
	if (
		(event.ctrlKey || event.metaKey) &&
		event.key.toLowerCase() === "f" &&
		!$$("dialog[open]").length
	) {
		event.preventDefault();
		$("#search").focus();
	}
});
$("#paste-button").addEventListener("click", async () => {
	try {
		$("#download-urls").value = await call("clipboard");
		checkPlaylistUrl();
		$("#download-urls").focus();
	} catch (error) {
		$("#add-error").textContent = messageOf(error);
		$("#add-error").hidden = false;
	}
});
$("#download-urls").addEventListener("input", checkPlaylistUrl);

let currentPlaylistMetadata = null;

function renderPlaylistItems(entries) {
	const container = $("#playlist-items");
	container.innerHTML = "";
	for (const entry of entries) {
		const row = document.createElement("label");
		row.className = "playlist-item";
		const cb = document.createElement("input");
		cb.type = "checkbox";
		cb.checked = true;
		cb.dataset.id = entry.id;

		const thumb = document.createElement("img");
		thumb.className = "playlist-item-thumb";
		thumb.src = entry.thumbnail || "";
		thumb.alt = "";

		const info = document.createElement("div");
		info.className = "playlist-item-info";
		const title = document.createElement("div");
		title.className = "playlist-item-title";
		title.textContent = entry.title;
		const meta = document.createElement("div");
		meta.className = "playlist-item-meta";
		const durationStr = entry.duration
			? `${Math.floor(entry.duration / 60)}:${String(Math.floor(entry.duration % 60)).padStart(2, "0")}`
			: "";
		meta.textContent = [entry.channel, durationStr].filter(Boolean).join(" • ");

		info.appendChild(title);
		if (meta.textContent) info.appendChild(meta);

		row.appendChild(cb);
		row.appendChild(thumb);
		row.appendChild(info);
		container.appendChild(row);
	}
}

$("#playlist-banner-btn")?.addEventListener("click", async () => {
	const url = $("#download-urls").value.trim();
	if (!url) return;
	const btn = $("#playlist-banner-btn");
	const errorEl = $("#add-error");
	errorEl.hidden = true;
	try {
		btn.disabled = true;
		btn.textContent = "Loading…";
		const metadata = await call("getPlaylistInfo", url);
		currentPlaylistMetadata = metadata;
		$("#playlist-name").textContent = metadata.title || "YouTube Playlist";
		$("#playlist-uploader").textContent = metadata.uploader ? `By ${metadata.uploader}` : "";
		$("#playlist-count").textContent = `${metadata.entryCount || metadata.entries?.length || 0} videos`;
		$("#playlist-error").hidden = true;
		renderPlaylistItems(metadata.entries || []);
		showDialog("#playlist-dialog");
	} catch (err) {
		errorEl.textContent = messageOf(err);
		errorEl.hidden = false;
	} finally {
		btn.disabled = false;
		btn.textContent = "Select Videos…";
	}
});

$("#video-banner-btn")?.addEventListener("click", async () => {
	const url = $("#download-urls").value.trim();
	if (!url) return;
	const btn = $("#video-banner-btn");
	const errorEl = $("#add-error");
	errorEl.hidden = true;
	try {
		btn.disabled = true;
		btn.textContent = "Loading formats…";
		const choices = await call("getVideoFormats", url);
		currentVideoChoices = choices || [];
		if (!currentVideoChoices.length) throw new Error("No available formats found for this video.");
		$("#video-formats-name").textContent = currentVideoChoices[0].title || "YouTube Video";
		const dur = currentVideoChoices[0].duration;
		$("#video-formats-details").textContent = dur ? `Duration: ${Math.floor(dur / 60)}m ${Math.floor(dur % 60)}s` : "";
		const select = $("#video-format-select");
		select.innerHTML = "";
		currentVideoChoices.forEach((choice, idx) => {
			const opt = document.createElement("option");
			opt.value = String(idx);
			const sizeStr = choice.estimatedSize ? ` (~${(choice.estimatedSize / (1024 * 1024)).toFixed(1)} MB)` : "";
			opt.textContent = `${choice.format} ${choice.quality || ""} - ${choice.audioState}${sizeStr}`;
			select.appendChild(opt);
		});
		$("#video-formats-error").hidden = true;
		showDialog("#video-formats-dialog");
	} catch (err) {
		errorEl.textContent = messageOf(err);
		errorEl.hidden = false;
	} finally {
		btn.disabled = false;
		btn.textContent = "Choose Quality / Format…";
	}
});

$("#video-formats-form")?.addEventListener("submit", (e) => {
	e.preventDefault();
	const select = $("#video-format-select");
	const idx = Number(select.value);
	const choice = currentVideoChoices[idx];
	if (choice) {
		chosenYouTubeFormat = choice.youtubeFormat;
		const ext = choice.outputFormat || (choice.format === "MP3" ? "mp3" : "mp4");
		let title = choice.title;
		if (!title || title.trim().toLowerCase() === "video") {
			try {
				const currentUrl = $("#download-url")?.value || "";
				const u = new URL(currentUrl);
				const host = u.hostname.replace(/^www\./, "").replace(/\./g, "_");
				title = host ? `download_${host}` : "download";
			} catch {
				title = "download";
			}
		}
		$("#download-filename").value = `${title}.${ext}`;
		$("#download-category").value = ext === "mp3" ? "Audio" : "Video";
		$("#video-banner-btn").textContent = `Selected: ${choice.quality || choice.format} (${ext.toUpperCase()})`;
	}
	$("#video-formats-dialog").close();
});

$("#playlist-select-all")?.addEventListener("click", () => {
	$$("#playlist-items input[type='checkbox']").forEach((cb) => (cb.checked = true));
});

$("#playlist-deselect-all")?.addEventListener("click", () => {
	$$("#playlist-items input[type='checkbox']").forEach((cb) => (cb.checked = false));
});

$("#install-ffmpeg-btn")?.addEventListener("click", () => installMediaToolAction("ffmpeg"));
$("#install-ytdlp-btn")?.addEventListener("click", () => installMediaToolAction("yt-dlp"));
$("#media-tool-install-btn")?.addEventListener("click", () => {
	openMediaToolDialog("ffmpeg", state.selectedId);
});

$("#media-tool-confirm-btn")?.addEventListener("click", async () => {
	const confirmBtn = $("#media-tool-confirm-btn");
	const cancelBtn = $("#media-tool-cancel-btn");
	const progressArea = $("#media-tool-dialog-progress");
	const statusText = $("#media-tool-dialog-status-text");
	const errorEl = $("#media-tool-dialog-error");

	confirmBtn.disabled = true;
	cancelBtn.disabled = true;
	progressArea.hidden = false;
	errorEl.hidden = true;
	statusText.textContent = "Downloading & installing FFmpeg into managed tools directory… Please wait.";

	try {
		await api.installMediaTool("ffmpeg");
		statusText.textContent = "FFmpeg installed successfully! Resuming download…";
		notify("FFmpeg installed successfully.");
		await refreshMediaToolsStatus();

		const downloadIdToResume = activeMediaToolDownloadId;
		setTimeout(async () => {
			$("#media-tool-dialog")?.close();
			if (downloadIdToResume) {
				try {
					const d = state.downloads.find((item) => item.id === downloadIdToResume);
					if (d) await performAction(d, "resume");
				} catch (e) {
					console.error("Resume after tool install error:", e);
				}
			}
		}, 800);
	} catch (err) {
		errorEl.textContent = `Installation failed: ${messageOf(err)}`;
		errorEl.hidden = false;
		confirmBtn.disabled = false;
		confirmBtn.textContent = "Retry Install";
		cancelBtn.disabled = false;
		progressArea.hidden = true;
	}
});

$("#playlist-form")?.addEventListener("submit", async (e) => {
	e.preventDefault();
	if (!currentPlaylistMetadata) return;
	const checkboxes = $$("#playlist-items input[type='checkbox']");
	const items = (currentPlaylistMetadata.entries || []).map((entry, idx) => ({
		id: entry.id,
		url: entry.url,
		title: entry.title,
		selected: !!checkboxes[idx]?.checked,
	}));

	const selectedCount = items.filter((i) => i.selected).length;
	if (selectedCount === 0) {
		$("#playlist-error").textContent = "Select at least one video to download.";
		$("#playlist-error").hidden = false;
		return;
	}

	const btn = $("#playlist-download-btn");
	try {
		btn.disabled = true;
		await call("addPlaylistDownloads", {
			playlistId: currentPlaylistMetadata.id,
			playlistTitle: currentPlaylistMetadata.title,
			items,
			commonCategory: $("#download-category").value || undefined,
			commonDirectory: $("#add-directory").value || undefined,
		});
		$("#playlist-dialog").close();
		$("#add-dialog").close();
		setView("all");
		await refresh();
		notify(`Added ${selectedCount} playlist videos to downloads.`);
	} catch (err) {
		$("#playlist-error").textContent = messageOf(err);
		$("#playlist-error").hidden = false;
	} finally {
		btn.disabled = false;
	}
});
$("#add-browse").addEventListener("click", async () => {
	try {
		const path = await call("chooseDirectory");
		if (typeof path === "string" && path) $("#add-directory").value = path;
	} catch (error) {
		$("#add-error").textContent = messageOf(error);
		$("#add-error").hidden = false;
	}
});
$("#download-category").addEventListener("change", (event) => {
	const folder = state.settings.categoryDirectories?.[event.target.value];
	if (folder) $("#add-directory").value = folder;
	rememberInput.disabled = !event.target.value;
	if (rememberInput.disabled) rememberInput.checked = false;
});
$("#add-form").addEventListener("submit", async (event) => {
	event.preventDefault();
	const errorElement = $("#add-error");
	errorElement.hidden = true;
	const urls = $("#download-urls")
		.value.split(/\r?\n/)
		.map((url) => url.trim())
		.filter(Boolean);
	const filename = $("#download-filename").value.trim(),
		scheduled = $("#download-schedule").value;
	try {
		if (!urls.length)
			throw new Error("Enter at least one download address.");
		for (const url of urls) {
			let parsed;
			try {
				parsed = new URL(url);
			} catch {
				throw new Error(`Invalid address: ${url}`);
			}
			if (!["http:", "https:", "ftp:"].includes(parsed.protocol))
				throw new Error(
					"Download addresses must begin with http://, https://, or ftp://.",
				);
		}
		if (filename && urls.length !== 1)
			throw new Error(
				"A custom file name can only be used for a single address.",
			);
		const scheduledAt = scheduled
			? new Date(scheduled).toISOString()
			: undefined;
		if (scheduledAt && new Date(scheduledAt).getTime() <= Date.now())
			throw new Error(
				"Choose a future start time or leave Start at empty.",
			);
		$("#add-submit").disabled = true;
		$("#add-later").disabled = true;
		const directory = $("#add-directory").value,
			selectedCategory = $("#download-category").value,
			description = $("#download-description").value.trim();
		if (rememberInput.checked && selectedCategory && directory)
			await call("settings", {
				categoryDirectories: {
					...state.settings.categoryDirectories,
					[selectedCategory]: directory,
				},
			});
		const authEnabled = $("#add-auth-enabled")?.checked;
		const authUser = $("#add-auth-user")?.value.trim();
		const authPass = $("#add-auth-pass")?.value || "";
		const credentials = (authEnabled && authUser) ? { username: authUser, password: authPass || null } : undefined;
		const data = {
			...(filename ? { filename } : {}),
			...(scheduledAt ? { scheduledAt } : {}),
			...(directory ? { directory } : {}),
			...(selectedCategory ? { category: selectedCategory } : {}),
			...(chosenYouTubeFormat ? { youtubeFormat: chosenYouTubeFormat } : {}),
			...(credentials ? { credentials } : {}),
			...($("#download-queue")?.value ? { queueId: $("#download-queue").value } : {}),
			description,
			queueOnly: event.submitter?.dataset.queueOnly === "true",
		};
		const ids = pendingReviewId
			? await call("review", { id: pendingReviewId, ...data })
			: await call("add", { urls, ...data });
		pendingReviewId = null;
		chosenYouTubeFormat = null;
		if (!addMode) $("#add-dialog").close();
		setView("all");
		await refresh();
		if (!data.queueOnly && !scheduledAt && ids?.length === 1) {
			selectDownload(ids[0]);
			if (addMode) await call("openProgress", { id: ids[0] });
			else {
				showDetails(ids[0]);
				if (progressMode) await call("openProgress", { id: ids[0] });
			}
		} else if (progressMode) {
			await call("closeProgress");
		}
		if (addMode) await call("closeProgress");
	} catch (error) {
		errorElement.textContent = messageOf(error);
		errorElement.hidden = false;
	} finally {
		$("#add-submit").disabled = false;
		$("#add-later").disabled = false;
	}
});
$("#app-theme")?.addEventListener("change", (e) => {
	applyTheme(e.target.value);
});
$("#settings-dialog")?.addEventListener("close", () => {
	applyTheme(state.settings?.theme || "system");
});
$("#choose-directory").addEventListener("click", async () => {
	try {
		const path = await call("chooseDirectory");
		if (typeof path === "string" && path)
			$("#default-directory").value = path;
	} catch (error) {
		$("#settings-error").textContent = messageOf(error);
		$("#settings-error").hidden = false;
	}
});
$("#options-category-select")?.addEventListener("change", (e) => {
	const cat = e.target.value;
	$("#category-directory").value = optionsCategoryDirectories[cat] || "";
});
$("#category-directory")?.addEventListener("input", (e) => {
	const cat = $("#options-category-select").value;
	const val = e.target.value.trim();
	if (val) {
		optionsCategoryDirectories[cat] = val;
	} else {
		delete optionsCategoryDirectories[cat];
	}
});
$("#choose-category-directory")?.addEventListener("click", async () => {
	try {
		const path = await call("chooseDirectory");
		if (typeof path === "string" && path) {
			const cat = $("#options-category-select").value;
			optionsCategoryDirectories[cat] = path;
			$("#category-directory").value = path;
		}
	} catch (error) {
		$("#settings-error").textContent = messageOf(error);
		$("#settings-error").hidden = false;
	}
});
$("#reset-category-directory")?.addEventListener("click", () => {
	const cat = $("#options-category-select").value;
	delete optionsCategoryDirectories[cat];
	$("#category-directory").value = "";
});
$("#choose-temporary-directory")?.addEventListener("click", async () => {
	try {
		const path = await call("chooseDirectory");
		if (typeof path === "string" && path)
			$("#temporary-directory").value = path;
	} catch (error) {
		$("#settings-error").textContent = messageOf(error);
		$("#settings-error").hidden = false;
	}
});
$("#reset-temporary-directory")?.addEventListener("click", () => {
	$("#temporary-directory").value = "";
});
$("#clipboard-monitoring-enabled")?.addEventListener("change", () => {
	if ($("#clipboard-auto-enqueue")) {
		$("#clipboard-auto-enqueue").disabled = !$("#clipboard-monitoring-enabled").checked;
	}
});
$("#add-override-button")?.addEventListener("click", () => {
	const hostInput = $("#new-override-host");
	const connsInput = $("#new-override-connections");
	if (!hostInput || !connsInput) return;
	let host = hostInput.value.trim().toLowerCase().replace(/^https?:\/\//, "");
	if (host.includes("/")) host = host.split("/")[0];
	if (host.includes(":")) host = host.split(":")[0];
	const conns = Math.max(1, Math.min(32, Number(connsInput.value) || 1));

	if (!host) {
		alert("Please enter a valid host pattern (e.g. example.com or *.domain.com)");
		return;
	}

	optionsConnectionOverrides.push({ hostPattern: host, connections: conns });
	hostInput.value = "";
	connsInput.value = "1";
	renderSiteOverrides();
});
$("#site-overrides-list")?.addEventListener("click", (e) => {
	const button = e.target.closest(".remove-override-button");
	if (!button) return;
	const index = Number(button.dataset.index);
	if (!isNaN(index) && index >= 0 && index < optionsConnectionOverrides.length) {
		optionsConnectionOverrides.splice(index, 1);
		renderSiteOverrides();
	}
});
$("#window-limit-enabled")?.addEventListener("change", () => {
	updateWindowLimitFieldsState();
});
$("#proxy-enabled").addEventListener("change", () => {
	updateProxyFieldsState();
	resetProxyTestStatus();
});
$("#proxy-protocol")?.addEventListener("change", (e) => {
	const protocol = e.target.value;
	const portInput = $("#proxy-port");
	if (portInput) {
		if (protocol === "socks5" && (portInput.value === "8080" || !portInput.value)) {
			portInput.value = "1080";
		} else if ((protocol === "http" || protocol === "https") && portInput.value === "1080") {
			portInput.value = "8080";
		}
	}
	updateProxyFieldsState();
	resetProxyTestStatus();
});
$("#test-proxy-button").addEventListener("click", async () => {
	const statusEl = $("#proxy-test-status");
	const protocol = $("#proxy-protocol").value;
	const isDirect = protocol === "direct";
	const host = $("#proxy-host").value.trim();
	const port = Number($("#proxy-port").value) || (protocol === "socks5" ? 1080 : 8080);
	const username = $("#proxy-username").value.trim() || null;
	const password = $("#proxy-password").value || null;
	const resolveDnsRemotely = Boolean($("#proxy-resolve-dns-remotely")?.checked);

	if (!isDirect && !host) {
		statusEl.textContent = "Please enter a proxy host.";
		statusEl.className = "proxy-test-status error";
		return;
	}

	statusEl.textContent = isDirect ? "Testing direct connection…" : "Testing proxy connection…";
	statusEl.className = "proxy-test-status testing";
	$("#test-proxy-button").disabled = true;

	try {
		const res = await call("testProxy", {
			proxy: {
				enabled: true,
				protocol,
				host,
				port,
				username,
				password,
				resolveDnsRemotely,
			},
		});
		if (res && res.success) {
			statusEl.textContent = res.message || "Connection successful.";
			statusEl.className = "proxy-test-status success";
		} else {
			statusEl.textContent = res?.message || "Connection failed.";
			statusEl.className = "proxy-test-status error";
		}
	} catch (err) {
		statusEl.textContent = messageOf(err);
		statusEl.className = "proxy-test-status error";
	} finally {
		$("#test-proxy-button").disabled = !$("#proxy-enabled").checked;
	}
});

$("#detect-proxy-button")?.addEventListener("click", async () => {
	const btn = $("#detect-proxy-button");
	const origText = btn.textContent;
	btn.disabled = true;
	btn.textContent = "Detecting…";
	try {
		const detected = await call("detectProxy");
		openProxyDiscoveryDialog(detected);
	} catch (err) {
		const statusEl = $("#proxy-test-status");
		if (statusEl) {
			statusEl.textContent = `Discovery error: ${messageOf(err)}`;
			statusEl.className = "proxy-test-status error";
		}
	} finally {
		btn.disabled = false;
		btn.textContent = origText;
	}
});

$("#proxy-discovery-close-btn")?.addEventListener("click", () => {
	$("#proxy-discovery-dialog")?.close();
});

$("#proxy-discovery-cancel-btn")?.addEventListener("click", () => {
	$("#proxy-discovery-dialog")?.close();
});

$("#proxy-discovery-import-btn")?.addEventListener("click", async () => {
	if (!currentDetectedProxyConfig || !currentDetectedProxyConfig.importSettings) return;
	const btn = $("#proxy-discovery-import-btn");
	const errorEl = $("#proxy-discovery-error");
	btn.disabled = true;
	btn.textContent = "Importing…";
	if (errorEl) errorEl.hidden = true;

	try {
		const updated = await call("applyDetectedProxy", {
			detected: currentDetectedProxyConfig,
		});
		const p =
			(updated && updated.proxy) ||
			currentDetectedProxyConfig.importSettings;
		$("#proxy-enabled").checked = p.enabled === true;
		$("#proxy-protocol").value = p.protocol || "http";
		$("#proxy-host").value = p.host || "";
		$("#proxy-port").value = p.port || 8080;
		$("#proxy-username").value = p.username || "";
		$("#proxy-password").value = "";
		if ($("#proxy-resolve-dns-remotely")) {
			$("#proxy-resolve-dns-remotely").checked =
				p.resolveDnsRemotely === true;
		}
		updateProxyFieldsState();
		resetProxyTestStatus();

		$("#proxy-discovery-dialog")?.close();
		const statusEl = $("#proxy-test-status");
		if (statusEl) {
			statusEl.textContent = "Discovered proxy settings applied.";
			statusEl.className = "proxy-test-status success";
		}
	} catch (err) {
		if (errorEl) {
			errorEl.textContent = messageOf(err);
			errorEl.hidden = false;
		}
		btn.disabled = false;
		btn.textContent = "Import Settings";
	}
});
$("#settings-form").addEventListener("submit", async (event) => {
	event.preventDefault();
	const button = $("button[type=submit]", event.target);
	$("#settings-error").hidden = true;
	button.disabled = true;
	try {
		const proxy = {
			enabled: $("#proxy-enabled").checked,
			protocol: $("#proxy-protocol").value,
			host: $("#proxy-host").value.trim(),
			port: Number($("#proxy-port").value) || ($("#proxy-protocol").value === "socks5" ? 1080 : 8080),
			username: $("#proxy-username").value.trim() || null,
			password: $("#proxy-password").value || null,
			resolveDnsRemotely: Boolean($("#proxy-resolve-dns-remotely")?.checked),
		};
		const capturePolicy = {
			enabled: $("#capture-enabled").checked,
			confirmationMode: $("#capture-mode").value,
			fileTypes: $("#capture-file-types").value.split(/[\s,]+/).map(s => s.trim().toLowerCase().replace(/^\./, "")).filter(Boolean),
			excludedHosts: $("#capture-excluded-hosts").value.split(",").map(s => s.trim()).filter(Boolean),
			excludedAddresses: $("#capture-excluded-addresses").value.split(",").map(s => s.trim()).filter(Boolean),
		};
		const completionActions = [];
		if ($("#settings-play-sound").checked) {
			const soundFile = $("#settings-sound-file").value.trim() || null;
			completionActions.push({ type: "playSound", soundFile });
		}
		if ($("#settings-scan-antivirus").checked) {
			const executable = $("#settings-scanner-executable").value.trim();
			if (executable) {
				const args = $("#settings-scanner-args").value.trim().split(/\s+/).filter(Boolean);
				completionActions.push({ type: "antivirusScan", executable, arguments: args });
			}
		}
		if ($("#settings-post-command").checked) {
			const executable = $("#settings-command-executable").value.trim();
			if (executable) {
				const args = $("#settings-command-args").value.trim().split(/\s+/).filter(Boolean);
				completionActions.push({ type: "executeCommand", executable, arguments: args });
			}
		}
		const powerKind = $("#settings-power-action").value;
		if (powerKind && powerKind !== "none") {
			completionActions.push({
				type: "powerAction",
				kind: powerKind,
				confirmed: $("#settings-power-confirm").checked,
			});
		}
		const clipboardMonitoring = {
			enabled: Boolean($("#clipboard-monitoring-enabled")?.checked),
			autoEnqueue: Boolean($("#clipboard-auto-enqueue")?.checked),
		};
		const windowLimitEnabled = Boolean($("#window-limit-enabled")?.checked);
		const maxMb = Number($("#window-limit-max-mb")?.value) || 2048;
		const windowHours = Number($("#window-limit-hours")?.value) || 12;
		const existingLimit = state.settings?.windowLimit || {};
		const windowLimit = {
			enabled: windowLimitEnabled,
			windowHours,
			maxBytes: maxMb * 1024 * 1024,
			windowAnchor: existingLimit.windowAnchor || new Date().toISOString(),
			currentBytes: existingLimit.currentBytes || 0,
			stoppedUntil: existingLimit.stoppedUntil || null,
		};
		await call("settings", {
			theme: $("#app-theme")?.value || "system",
			startWithWindows: $("#start-with-windows").checked,
			defaultDirectory: $("#default-directory").value.trim(),
			maxConcurrent: Number($("#max-concurrent").value),
			connections: Number($("#connections").value),
			speedLimitKbps: Number($("#speed-limit").value),
			categoryDirectories: optionsCategoryDirectories,
			temporaryDirectory: $("#temporary-directory").value.trim() || undefined,
			proxy,
			capturePolicy,
			completionActions,
			clipboardMonitoring,
			connectionOverrides: optionsConnectionOverrides,
			windowLimit,
		});
		await refresh();
		if (optionsMode) await call("closeProgress");
		else $("#settings-dialog").close();
	} catch (error) {
		$("#settings-error").textContent = messageOf(error);
		$("#settings-error").hidden = false;
	} finally {
		button.disabled = false;
	}
});
window.addEventListener("downloader:api-error", (event) =>
	notify(messageOf(event.detail), true),
);
async function start() {
	try {
		await refresh();
		if (typeof api.subscribe === "function")
			unsubscribe = api.subscribe((snapshot) => {
				if (snapshot?.downloads) acceptSnapshot(snapshot);
				else refresh().catch((error) => notify(messageOf(error), true));
			});
		if (typeof api.onCommand === "function")
			unsubscribeCommands = api.onCommand(async (payload) => {
				const command =
					typeof payload === "string" ? payload : payload?.command;
				if (command === "dismissBrowserIntegration") {
					if ($("#integration-dialog").open)
						$("#integration-dialog").close();
					return;
				}
				if (command === "reviewDownload") {
					try {
						await refresh();
						if (!$$("dialog[open]").length) openReview(payload.id);
					} catch (error) {
						notify(messageOf(error), true);
					}
					return;
				}
				if (
					!$$("dialog[open]").length ||
					["pauseAll", "resumeAll", "removeCompleted"].includes(
						command,
					)
				)
					commands[command]?.();
			});
		if (window.__TAURI__?.event?.listen) {
			window.__TAURI__.event.listen("clipboard:candidate", (event) => {
				const candidate = event.payload;
				if (!candidate || !candidate.url) return;
				handleClipboardCandidate(candidate);
			});
		}
	} catch (error) {
		$("#download-list").setAttribute("aria-busy", "false");
		$("#list-count").textContent = "Download service unavailable";
		$("#connection-label").textContent = "Disconnected";
		notify(messageOf(error), true);
	}
}
window.addEventListener("beforeunload", () => {
	if (typeof unsubscribe === "function") unsubscribe();
	if (typeof unsubscribeCommands === "function") unsubscribeCommands();
});
if (progressMode) {
	$("#details-dialog").addEventListener("close", () => call("closeProgress"));
	new MutationObserver(() => {
		document.title = $("#details-title").textContent;
	}).observe($("#details-title"), {
		childList: true,
		characterData: true,
		subtree: true,
	});
	// The initial snapshot decides between confirmation and transfer progress.
}
if (integrationMode)
	$("#integration-dialog").addEventListener("close", () =>
		call("closeProgress"),
	);
if (optionsMode)
	$("#settings-dialog").addEventListener("close", () =>
		call("closeProgress"),
	);
if (schedulerMode)
	$("#scheduler-dialog").addEventListener("cancel", (event) => {
		event.preventDefault();
		call("closeProgress").catch((error) => notify(messageOf(error), true));
	});
start().then(async () => {
	if (progressMode) {
		showDetails(progressMode);
		return;
	}
	$("#add-auth-enabled")?.addEventListener("change", (e) => {
	const fields = $("#add-auth-fields");
	if (fields) fields.hidden = !e.target.checked;
});

$("#cred-save-btn")?.addEventListener("click", async () => {
	const scheme = $("#cred-scheme").value;
	const host = $("#cred-host").value.trim();
	const portVal = $("#cred-port").value.trim();
	const pathPrefix = $("#cred-path").value.trim() || "/";
	const username = $("#cred-user").value.trim();
	const secret = $("#cred-pass").value;
	const statusEl = $("#cred-status");

	if (!host) {
		if (statusEl) {
			statusEl.textContent = "Please enter a valid host.";
			statusEl.style.color = "#dc2626";
		}
		return;
	}
	if (!username) {
		if (statusEl) {
			statusEl.textContent = "Please enter a username.";
			statusEl.style.color = "#dc2626";
		}
		return;
	}

	const port = portVal ? parseInt(portVal, 10) : null;
	try {
		await api.saveCredentialRule({
			scheme,
			host,
			port,
			pathPrefix,
			username,
			secret
		});
		$("#cred-host").value = "";
		$("#cred-port").value = "";
		$("#cred-path").value = "/";
		$("#cred-user").value = "";
		$("#cred-pass").value = "";
		if (statusEl) {
			statusEl.textContent = "Login rule saved.";
			statusEl.style.color = "#166534";
			setTimeout(() => { statusEl.textContent = ""; }, 3000);
		}
		await refreshCredentialRules();
	} catch (err) {
		if (statusEl) {
			statusEl.textContent = `Error: ${messageOf(err)}`;
			statusEl.style.color = "#dc2626";
		}
	}
});

$("#credentials-list")?.addEventListener("click", async (e) => {
	const btn = e.target.closest(".delete-cred-btn");
	if (!btn) return;
	const id = btn.dataset.id;
	if (!id) return;
	try {
		await api.deleteCredentialRule(id);
		await refreshCredentialRules();
	} catch (err) {
		notify(`Failed to delete credential rule: ${messageOf(err)}`, true);
	}
});

let currentClipboardCandidate = null;

function handleClipboardCandidate(candidate) {
	if (!candidate || !candidate.url) return;
	currentClipboardCandidate = candidate;
	if (state?.settings?.clipboardMonitoring?.autoEnqueue) {
		openAdd(candidate.url);
		return;
	}
	const descEl = $("#clipboard-prompt-desc");
	if (descEl) {
		descEl.textContent = `Download "${candidate.filename}" from ${candidate.host}?`;
	}
	const promptEl = $("#clipboard-prompt");
	if (promptEl) {
		promptEl.hidden = false;
	}
}

function dismissClipboardPrompt() {
	const promptEl = $("#clipboard-prompt");
	if (promptEl) promptEl.hidden = true;
	if (currentClipboardCandidate) {
		if (typeof api.dismissClipboardCandidate === "function") {
			api.dismissClipboardCandidate(currentClipboardCandidate.url);
		}
		currentClipboardCandidate = null;
	}
}

$("#clipboard-prompt-close")?.addEventListener("click", dismissClipboardPrompt);
$("#clipboard-prompt-dismiss")?.addEventListener("click", dismissClipboardPrompt);
$("#clipboard-prompt-add")?.addEventListener("click", () => {
	const cand = currentClipboardCandidate;
	const promptEl = $("#clipboard-prompt");
	if (promptEl) promptEl.hidden = true;
	currentClipboardCandidate = null;
	if (cand?.url) {
		openAdd(cand.url);
	}
});

if (optionsMode) {
		openSettings();
		return;
	}
	if (addMode) {
		let text = "";
		if (new URLSearchParams(location.search).get("paste") === "1")
			try {
				text = await call("clipboard");
			} catch (error) {
				notify(messageOf(error), true);
			}
		openAdd(
			text,
			new URLSearchParams(location.search).get("scheduled") || "",
		);
		return;
	}
	if (schedulerMode) {
		openScheduler();
		return;
	}
	if (integrationMode) {
		selectedBrowserName = null;
		$("#browser-detail").hidden = true;
		$("#browser-grid").hidden = false;
		$("#integration-dialog").show();
		await refreshIntegrationInfo(true);
		return;
	}
	if (
		state.integration.packaged &&
		!state.integration.connectedBrowsers?.length &&
		!localStorage.getItem("browser-setup-introduced") &&
		!$$("dialog[open]").length
	) {
		await openIntegration();
		localStorage.setItem("browser-setup-introduced", "1");
	}
});
