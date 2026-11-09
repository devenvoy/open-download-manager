// Created: 2026-10-24
// Reason: Initial public release
// Last updated: 2026-11-10
// Reason: refactor: modularize data formatters and presentation transformers from app.js
// Description: Pure data presentation formatters for sizes, dates, durations, percentages, categories, and redacted URLs.

"use strict";

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
	const value = String(d?.status || "queued").toLowerCase();
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
	const value = String(d?.category || "").toLowerCase();
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

function duration(seconds) {
	return seconds < 60
		? `${Math.ceil(seconds)} sec`
		: seconds < 3600
			? `${Math.ceil(seconds / 60)} min`
			: `${Math.floor(seconds / 3600)} hr ${Math.floor((seconds % 3600) / 60)} min`;
}

function timeLeft(d) {
	if (!d) return "";
	const total = numeric(d.total) || numeric(d.streamProgress?.estimatedBytes);
	if (!total || !numeric(d.speed)) return "";
	const seconds = Math.max(
		0,
		Math.ceil((total - numeric(d.downloaded)) / numeric(d.speed)),
	);
	return (!d.total || d.estimated ? "≈ " : "") + duration(seconds);
}

function sizeLabel(d) {
	if (!d) return "";
	if (statusOf(d) === "connecting" || statusOf(d) === "queued") return "Connecting…";
	return numeric(d.total)
		? (d.estimated ? "≈ " : "") + bytes(d.total)
		: d.streamProgress?.estimatedBytes
			? "≈ " + bytes(d.streamProgress.estimatedBytes)
			: d.kind === "hls"
				? "Calculating from segments…"
				: "Not supplied by server";
}

function percentage(d) {
	if (!d) return null;
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

function formatProxyDisplay(proxy) {
	if (!proxy || !proxy.enabled || proxy.protocol === "direct") {
		return "Direct (No Proxy)";
	}
	const userPrefix = proxy.username ? `${proxy.username}@` : "";
	return `${String(proxy.protocol || "http").toUpperCase()} ${userPrefix}${proxy.host}:${proxy.port}`;
}

if (typeof window !== "undefined") {
	window.numeric = numeric;
	window.redactUrl = redactUrl;
	window.statusOf = statusOf;
	window.categoryOf = categoryOf;
	window.bytes = bytes;
	window.date = date;
	window.duration = duration;
	window.timeLeft = timeLeft;
	window.sizeLabel = sizeLabel;
	window.percentage = percentage;
	window.messageOf = messageOf;
	window.formatProxyDisplay = formatProxyDisplay;
}

if (typeof module !== "undefined" && module.exports) {
	module.exports = {
		numeric,
		redactUrl,
		statusOf,
		categoryOf,
		bytes,
		date,
		duration,
		timeLeft,
		sizeLabel,
		percentage,
		messageOf,
		formatProxyDisplay,
	};
}
