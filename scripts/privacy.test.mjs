import assert from "node:assert/strict";
import { mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import test from "node:test";
import {
	ALLOW_FILE,
	loadAllow,
	machineIdentity,
	rangesFromPrePush,
	scanFileName,
	scanLine,
	scanLog,
	scanPatch,
} from "./privacy.mjs";

const allow = loadAllow(ALLOW_FILE);
const options = { allow, identity: ["Jdoe", "DESKTOP-7Q2X"] };
const at = { file: "a.txt", line: 1 };
const rules = (findings) => findings.map((finding) => finding.rule);

test("keys, tokens and private key blocks are caught and masked", () => {
	const token = `ghp_${"a1".repeat(18)}`;
	const findings = scanLine(`const t = "${token}";`, at, options);
	assert.ok(rules(findings).includes("github-token"));
	assert.ok(findings.every((finding) => !finding.excerpt.includes(token)));
	assert.deepEqual(
		rules(scanLine("-----BEGIN OPENSSH PRIVATE KEY-----", at, options)),
		["private-key"],
	);
	assert.deepEqual(
		rules(
			scanLine("untrusted comment: rsign encrypted secret key", at, options),
		),
		["signing-key"],
	);
	assert.deepEqual(
		rules(scanLine(`key = "AKIA${"A".repeat(16)}"`, at, options)),
		["aws-key"],
	);
	assert.deepEqual(rules(scanLine('password: "hunter2hunter2"', at, options)), [
		"secret-assignment",
	]);
});

test("secret names without values and placeholders pass", () => {
	assert.deepEqual(
		scanLine(`TAURI_SIGNING_PRIVATE_KEY: ${"$"}{{ secrets.KEY }}`, at, options),
		[],
	);
	assert.deepEqual(scanLine(`token: "${"$"}{GITHUB_TOKEN}"`, at, options), []);
	assert.deepEqual(
		scanLine("let num_tokens = segment.n_tokens();", at, options),
		[],
	);
});

test("personal emails are caught, noreply and example addresses pass", () => {
	assert.deepEqual(
		rules(scanLine("Author: someone.real@gmail.com", at, options)),
		["email"],
	);
	assert.deepEqual(rules(scanLine("mail me at 12345@qq.com", at, options)), [
		"email",
	]);
	assert.deepEqual(
		scanLine(
			"Co-Authored-By: Claude <noreply@anthropic.com> 1+x@users.noreply.github.com",
			at,
			options,
		),
		[],
	);
	assert.deepEqual(
		scanLine(
			"user@example.com and git@github.com:AniBullet/FrameEver",
			at,
			options,
		),
		[],
	);
	assert.deepEqual(scanLine("icons/128x128@2x.png", at, options), []);
});

test("this machine's user and host names are caught, case-sensitively", () => {
	assert.deepEqual(
		rules(scanLine("C:\\Users\\Jdoe\\AppData\\Local\\Temp", at, options)),
		["machine-identity"],
	);
	assert.deepEqual(rules(scanLine("built on DESKTOP-7Q2X", at, options)), [
		"machine-identity",
	]);
	assert.deepEqual(scanLine("jdoe-style naming and Jdoes", at, options), []);
	const identity = machineIdentity({
		USERNAME: "Jdoe",
		USERPROFILE: "C:\\Users\\Jdoe",
		HOME: undefined,
	});
	assert.ok(identity.includes("Jdoe"));
});

test("Chinese mobile and ID numbers are caught, other long numbers pass", () => {
	assert.deepEqual(rules(scanLine("call 13812345678", at, options)), [
		"cn-mobile",
	]);
	assert.deepEqual(rules(scanLine("id 11010519491231002X", at, options)), [
		"cn-id-number",
	]);
	assert.deepEqual(
		scanLine("timestamp 1729764000123 and v1.13812345678", at, options),
		[],
	);
});

test("sensitive file names are caught", () => {
	for (const file of [
		".env",
		"config/.env.local",
		"certs/server.pem",
		"id_ed25519",
		".claude/settings.local.json",
		"frameever.log",
	]) {
		assert.deepEqual(
			rules(scanFileName(file, allow)),
			["sensitive-file"],
			file,
		);
	}
	for (const file of [".env.example", "src/keyboard.rs", "docs/key.md"]) {
		assert.deepEqual(scanFileName(file, allow), [], file);
	}
});

test("a patch reports added lines with their line numbers and skips lock files", () => {
	const patch = [
		"diff --git a/src/a.rs b/src/a.rs",
		"--- a/src/a.rs",
		"+++ b/src/a.rs",
		"@@ -10,0 +11,2 @@",
		"+let ok = 1;",
		'+let mail = "x.y@gmail.com";',
		"diff --git a/Cargo.lock b/Cargo.lock",
		"--- a/Cargo.lock",
		"+++ b/Cargo.lock",
		"@@ -1,0 +1 @@",
		'+checksum = "someone@gmail.com"',
		"--- a/old.txt",
		"+++ /dev/null",
		"@@ -1 +0,0 @@",
		"-removed@gmail.com",
	].join("\n");
	const findings = scanPatch(patch, options, "abc");
	assert.equal(findings.length, 1);
	assert.deepEqual(
		{
			file: findings[0].file,
			line: findings[0].line,
			rule: findings[0].rule,
			commit: findings[0].commit,
		},
		{ file: "src/a.rs", line: 12, rule: "email", commit: "abc" },
	);
});

test("commit identities and messages are scanned", () => {
	const log =
		"\u0001commit\u0001abcdef1234\u0000me@gmail.com\u000045991057+AniBullet@users.noreply.github.com\u0002fix: thing\n\nsee C:\\Users\\Jdoe\n\u0003\n";
	assert.deepEqual(rules(scanLog(log, options)).sort(), [
		"author-email",
		"machine-identity",
	]);
});

test("pre-push input becomes ranges; deleted refs are skipped", () => {
	const local = "a".repeat(40);
	const remote = "b".repeat(40);
	const zero = "0".repeat(40);
	assert.deepEqual(
		rangesFromPrePush(
			`refs/heads/main ${local} refs/heads/main ${remote}\nrefs/heads/new ${local} refs/heads/new ${zero}\n(delete) ${zero} refs/heads/old ${remote}\n`,
		),
		[[`${remote}..${local}`], [local, "--not", "--remotes"]],
	);
});

test("the allow file adds emails, files and public names", () => {
	const dir = mkdtempSync(path.join(tmpdir(), "privacy-"));
	const file = path.join(dir, "allow.json");
	writeFileSync(
		file,
		JSON.stringify({
			emails: ["ok@corp\\.cn"],
			files: ["^fixtures/"],
			publicNames: ["Brand"],
		}),
	);
	const custom = loadAllow(file);
	assert.deepEqual(
		scanLine("ok@corp.cn", at, { allow: custom, identity: [] }),
		[],
	);
	assert.deepEqual(scanFileName("fixtures/server.pem", custom), []);
	assert.deepEqual(custom.publicNames, ["brand"]);
	rmSync(dir, { recursive: true });
});
