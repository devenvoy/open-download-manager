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
	// The React renderer: TypeScript, JSX, the file rules from
	// docs/REACT-UI-MIGRATION-PLAN.md §5.2 and §10.5. The type-aware rules
	// arrive with the generated IPC types; the parser-only rules below catch
	// the classes that do not need a program.
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
		files: ["tools/**/*.mjs"],
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
