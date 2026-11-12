// Created: 2026-11-12
// Reason: fix(torrent): catch state members that are used but never declared
// Description: Static guard for the class of bug where a script reads a member
// of an object literal that was never declared. The lint step cannot see it:
// `state.filesHash.set(...)` is a member access, not an undefined identifier,
// so it only fails at runtime with "undefined is not an object".

"use strict";

const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const test = require("node:test");

const RENDERER_DIR = __dirname;

// Members that every object inherits. They are never declared in a literal.
const INHERITED = new Set([
	"length","toString","valueOf","hasOwnProperty","constructor","prototype",
	"get","set","has","delete","clear","size","keys","values","entries","forEach",
	"map","filter","reduce","some","every","find","findIndex","push","pop","shift",
	"unshift","slice","splice","concat","join","includes","indexOf","sort","reverse",
	"flat","flatMap","fill","then","catch","finally","toJSON","call","apply","bind",
	"name","message","stack","add","test","exec","trim","split","replace","match",
]);

// Reads a top-level object literal and the keys it declares.
function readLiteral(src, name) {
	const pattern = new RegExp(
		`^(?:const|let|var)\\s+${name}\\s*=\\s*\\{`,
		"m",
	);
	const match = pattern.exec(src);
	if (!match) return null;
	const open = src.indexOf("{", match.index);
	let depth = 0;
	let end = open;
	for (let i = open; i < src.length; i++) {
		if (src[i] === "{") depth++;
		else if (src[i] === "}") {
			depth--;
			if (depth === 0) {
				end = i;
				break;
			}
		}
	}
	return src.slice(open, end + 1);
}

function objectLiterals(src) {
	const names = [];
	const pattern = /^(?:const|let|var)\s+([A-Za-z_$][\w$]*)\s*=\s*\{/gm;
	let match;
	while ((match = pattern.exec(src))) names.push(match[1]);
	return names;
}

function declaredKeys(src, name) {
	const keys = new Set();
	const literal = readLiteral(src, name);
	if (!literal) return keys;
	for (const key of literal.matchAll(/(?:^|[,{\n])\s*(?:"|')?([A-Za-z_$][\w$]*)(?:"|')?\s*[:,}]/g)) {
		keys.add(key[1]);
	}
	// A key that is assigned later counts as declared.
	for (const key of src.matchAll(new RegExp(`\\b${name}\\.([A-Za-z_$][\\w$]*)\\s*=`, "g"))) {
		keys.add(key[1]);
	}
	return keys;
}

// A computed assignment (`bag[key] = ...`) means the object holds keys that
// cannot be read from the source, so it cannot be checked.
function hasComputedWrite(src, name) {
	return new RegExp(`\\b${name}\\[[^\\]]*\\]\\s*=`).test(src);
}

function readOnlyFiles() {
	return fs
		.readdirSync(RENDERER_DIR)
		.filter(file => file.endsWith(".js"))
		.map(file => path.join(RENDERER_DIR, file));
}

test("every object literal member that is read is declared", () => {
	const problems = [];
	for (const file of readOnlyFiles()) {
		const src = fs.readFileSync(file, "utf8");
		for (const name of objectLiterals(src)) {
			if (hasComputedWrite(src, name)) continue;
			const declared = declaredKeys(src, name);
			const usage = new RegExp(`\\b${name}\\.([A-Za-z_$][\\w$]*)`, "g");
			for (const member of src.matchAll(usage)) {
				const key = member[1];
				if (INHERITED.has(key)) continue;
				if (declared.has(key)) continue;
				const line = src.slice(0, member.index).split("\n").length;
				problems.push(
					`${path.basename(file)}:${line} ${name}.${key} is read but never declared`,
				);
			}
		}
	}
	assert.deepEqual(problems, [], problems.join("\n"));
});

test("the guard catches a member that was never declared", () => {
	// The guard is only worth having if it really fails on a missing member.
	const source = [
		"const state = {",
		"\tfiles: new Map(),",
		"};",
		"function use(torrent) {",
		"\treturn state.filesHash.set(torrent.id, torrent.infoHash);",
		"}",
	].join("\n");
	const name = "state";
	const declared = declaredKeys(source, name);
	assert.ok(declared.has("files"));
	assert.ok(!declared.has("filesHash"));
});
