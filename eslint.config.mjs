import js from "@eslint/js";
import globals from "globals";

// The renderer scripts publish helpers on `window` for each other. A file
// that defines them must not be told they are globals, otherwise the
// definition itself looks like a redeclaration.
const rendererGlobals = [
	"numeric","redactUrl","statusOf","categoryOf","bytes","date","duration",
	"timeLeft","sizeLabel","percentage","messageOf","formatProxyDisplay",
	"getEffectiveTheme","applyTheme","initTheme","initializeColumnResizing",
	"downloader","TorrentView",
];

export default [
	{
		ignores: ["node_modules/**", "target/**", "dist/**", "src-tauri/target/**"],
	},
	{
		files: ["renderer/**/*.js"],
		languageOptions: {
			ecmaVersion: 2023,
			sourceType: "script",
			globals: {
				...globals.browser,
				...globals.node,
				module: "readonly",
			},
		},
		rules: {
			...js.configs.recommended.rules,
			"no-undef": "error",
			"no-unused-vars": ["warn", { args: "none", varsIgnorePattern: "^_" }],
			"no-console": "off",
			"no-empty": ["error", { allowEmptyCatch: true }],
			"no-prototype-builtins": "error",
			"no-constant-condition": ["error", { checkLoops: false }],
			"no-dupe-keys": "error",
			"no-dupe-args": "error",
			"no-duplicate-case": "error",
			"no-self-compare": "error",
			"no-sparse-arrays": "error",
			"no-unreachable": "error",
			"valid-typeof": "error",
			"no-fallthrough": "error",
			"no-cond-assign": "error",
			"no-import-assign": "error",
			"no-global-assign": "error",
			"no-obj-calls": "error",
			"no-unsafe-negation": "error",
			"use-isnan": "error",
		},
	},
	{
		// The consumer scripts may use the shared helpers.
		files: ["renderer/app.js", "renderer/torrents.js"],
		languageOptions: {
			globals: {
				...Object.fromEntries(rendererGlobals.map(n => [n, "readonly"])),
			},
		},
	},
	{
		files: ["extension/**/*.js"],
		languageOptions: {
			ecmaVersion: 2023,
			sourceType: "script",
			globals: {
				...globals.browser,
				...globals.webextensions,
				...globals.node,
				module: "readonly",
				chrome: "readonly",
				browser: "readonly",
				importScripts: "readonly",
			},
		},
		rules: {
			...js.configs.recommended.rules,
			"no-undef": "error",
			"no-unused-vars": ["warn", { args: "none", varsIgnorePattern: "^_" }],
			"no-console": "off",
			"no-empty": ["error", { allowEmptyCatch: true }],
			"no-prototype-builtins": "error",
			"no-dupe-keys": "error",
			"no-duplicate-case": "error",
			"no-self-compare": "error",
			"no-unreachable": "error",
			"valid-typeof": "error",
			"no-fallthrough": "error",
			"no-cond-assign": "error",
			"no-global-assign": "error",
			"no-obj-calls": "error",
			"use-isnan": "error",
		},
	},
	{
		files: ["tools/**/*.mjs", "extension/pack.mjs"],
		languageOptions: {
			ecmaVersion: 2023,
			sourceType: "module",
			globals: { ...globals.node },
		},
		rules: {
			...js.configs.recommended.rules,
			"no-undef": "error",
			"no-unused-vars": ["warn", { args: "none", varsIgnorePattern: "^_" }],
		},
	},
];
