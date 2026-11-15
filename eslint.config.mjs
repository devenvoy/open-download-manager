import js from "@eslint/js";
import globals from "globals";
import tseslint from "typescript-eslint";
import reactHooks from "eslint-plugin-react-hooks";

export default [
	{
		ignores: [
			"node_modules/**",
			"target/**",
			"dist/**",
			"renderer/dist/**",
			"src-tauri/target/**",
			"reference/**",
			"graphify-out/**",
			"coverage/**",
		],
	},
	// The React renderer: TypeScript, JSX, and strict modular feature rules.
	// Catches undeclared state access, missing props, and enforces the 300-line limit.
	{
		files: ["renderer/**/*.{ts,tsx}"],
		languageOptions: {
			parser: tseslint.parser,
			ecmaVersion: 2023,
			sourceType: "module",
			parserOptions: { ecmaFeatures: { jsx: true } },
			globals: { ...globals.browser },
		},
		plugins: {
			"@typescript-eslint": tseslint.plugin,
			"react-hooks": reactHooks,
		},
		rules: {
			...js.configs.recommended.rules,
			// TypeScript resolves identifiers; no-undef misreads type-only names.
			"no-undef": "off",
			"no-unused-vars": "off",
			"no-unreachable": "error",
			"no-dupe-keys": "error",
			"no-fallthrough": "error",
			"use-isnan": "error",
			"@typescript-eslint/no-unused-vars": ["warn", { argsIgnorePattern: "^_" }],
			"@typescript-eslint/no-explicit-any": "error",
			"@typescript-eslint/consistent-type-imports": "error",
			"@typescript-eslint/no-unused-expressions": "error",
			"react-hooks/rules-of-hooks": "error",
			"react-hooks/exhaustive-deps": "warn",
			"max-lines": ["error", { max: 300, skipBlankLines: true, skipComments: true }],
			"no-restricted-syntax": [
				"error",
				{
					selector: "AssignmentExpression[left.object.name='window']",
					message:
						"Do not hang state on window. Use lib/ipc or lib/ui.",
				},
			],
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
