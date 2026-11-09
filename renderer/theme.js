// Created: 2026-11-08
// Reason: feat(theme): add light, dark, and system theme options to the desktop app (Closes #4)
// Last updated: 2026-11-10
// Reason: refactor: modularize theme management from app.js
// Description: Theme resolution and application logic for light, dark, and system color schemes.

"use strict";

let currentThemeSetting = "system";

function getEffectiveTheme(themeSetting, matchMediaFn = (typeof window !== "undefined" && window.matchMedia ? window.matchMedia.bind(window) : null)) {
	if (themeSetting === "light" || themeSetting === "dark") {
		return themeSetting;
	}
	if (matchMediaFn) {
		return matchMediaFn("(prefers-color-scheme: dark)").matches ? "dark" : "light";
	}
	return "light";
}

function applyTheme(themeSetting = "system") {
	currentThemeSetting = themeSetting || "system";
	const effective = getEffectiveTheme(currentThemeSetting);
	if (typeof document !== "undefined" && document.documentElement) {
		document.documentElement.setAttribute("data-theme", effective);
		document.documentElement.setAttribute("data-theme-setting", currentThemeSetting);
		document.documentElement.style.colorScheme = effective;
	}
	return effective;
}

function initTheme() {
	if (typeof window !== "undefined" && window.matchMedia) {
		window.matchMedia("(prefers-color-scheme: dark)").addEventListener("change", () => {
			if (currentThemeSetting === "system") {
				applyTheme("system");
			}
		});
	}
	applyTheme("system");
}

if (typeof window !== "undefined") {
	window.getEffectiveTheme = getEffectiveTheme;
	window.applyTheme = applyTheme;
	window.initTheme = initTheme;
}

if (typeof module !== "undefined" && module.exports) {
	module.exports = { getEffectiveTheme, applyTheme, initTheme };
}
