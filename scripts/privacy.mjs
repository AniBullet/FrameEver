import { spawnSync } from "node:child_process";
import { existsSync, readFileSync } from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
export const ALLOW_FILE = path.join(root, "scripts", "privacy-allow.json");
const ZERO_SHA = /^0+$/;

export const SECRET_RULES = [
	{ id: "private-key", pattern: /-----BEGIN (?:[A-Z0-9]+ )*PRIVATE KEY-----/ },
	{ id: "signing-key", pattern: /(?:minisign|rsign) encrypted secret key/i },
	{
		id: "github-token",
		pattern: /\bgh[pousr]_[A-Za-z0-9]{36,}|\bgithub_pat_[A-Za-z0-9_]{40,}/,
	},
	{ id: "anthropic-key", pattern: /\bsk-ant-[A-Za-z0-9_-]{20,}/ },
	{ id: "openai-key", pattern: /\bsk-(?:proj-)?[A-Za-z0-9]{32,}/ },
	{ id: "aws-key", pattern: /\bAKIA[0-9A-Z]{16}\b/ },
	{ id: "google-key", pattern: /\bAIza[0-9A-Za-z_-]{35}\b/ },
	{ id: "slack-token", pattern: /\bxox[abprs]-[A-Za-z0-9-]{10,}/ },
	{
		id: "secret-assignment",
		pattern:
			/\b(?:password|passwd|pwd|secret|token|api[_-]?key|access[_-]?key)\b["']?\s*[:=]\s*["'][^"'\s$<{]{8,}["']/i,
	},
	{
		id: "cn-id-number",
		pattern:
			/(?<![0-9A-Za-z])[1-9]\d{5}(?:19|20)\d{2}(?:0[1-9]|1[0-2])(?:0[1-9]|[12]\d|3[01])\d{3}[\dXx](?![0-9A-Za-z])/,
	},
	{ id: "cn-mobile", pattern: /(?<![0-9A-Za-z.+-])1[3-9]\d{9}(?![0-9A-Za-z])/ },
];

const EMAIL =
	/[A-Za-z0-9._%+-]+@[A-Za-z0-9-]+(?:\.[A-Za-z0-9-]+)*\.[A-Za-z]{2,}/g;

const ASSET_SUFFIX = /\.(png|jpe?g|gif|svg|webp|ico|icns|bmp|tiff?)$/i;

const SENSITIVE_FILES = [
	/(^|\/)\.env(\.(?!example$|sample$|template$)[^/]+)?$/,
	/\.(pem|key|p12|pfx|keystore|jks|ppk)$/i,
	/(^|\/)id_(rsa|dsa|ecdsa|ed25519)(\.pub)?$/,
	/(^|\/)settings\.local\.json$/,
	/(^|\/)\.npmrc$/,
	/\.log$/i,
];

export const SKIPPED_CONTENT = [
	/(^|\/)Cargo\.lock$/,
	/(^|\/)pnpm-lock\.yaml$/,
	/(^|\/)package-lock\.json$/,
];

export function loadAllow(file = ALLOW_FILE) {
	const raw = existsSync(file) ? JSON.parse(readFileSync(file, "utf8")) : {};
	return {
		emails: (raw.emails ?? []).map(
			(pattern) => new RegExp(`^(?:${pattern})$`, "i"),
		),
		files: (raw.files ?? []).map((pattern) => new RegExp(pattern)),
		text: (raw.text ?? []).map((pattern) => new RegExp(pattern)),
		publicNames: (raw.publicNames ?? []).map((name) => name.toLowerCase()),
	};
}

export function machineIdentity(env = process.env) {
	const words = new Set();
	const add = (value) => {
		const word = value?.trim();
		if (word && word.length >= 3) words.add(word);
	};
	add(env.USERNAME);
	add(env.USER);
	add(safeUsername());
	add(os.hostname());
	add(env.COMPUTERNAME);
	const home = env.USERPROFILE ?? env.HOME ?? os.homedir();
	if (home) add(path.basename(home));
	return [...words].filter(
		(word) => !/^(root|user|admin|runner|home)$/i.test(word),
	);
}

function safeUsername() {
	try {
		return os.userInfo().username;
	} catch {
		return null;
	}
}

function escapeRegExp(text) {
	return text.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}

export function mask(text) {
	return text.length <= 6 ? "***" : `${text.slice(0, 4)}***`;
}

export function scanLine(text, where, { allow, identity }) {
	if (allow.text.some((pattern) => pattern.test(text))) return [];
	const findings = [];
	for (const rule of SECRET_RULES) {
		const match = text.match(rule.pattern);
		if (match)
			findings.push({ ...where, rule: rule.id, excerpt: mask(match[0]) });
	}
	for (const email of text.match(EMAIL) ?? []) {
		if (ASSET_SUFFIX.test(email)) continue;
		if (!allow.emails.some((pattern) => pattern.test(email))) {
			findings.push({ ...where, rule: "email", excerpt: mask(email) });
		}
	}
	for (const word of identity) {
		const pattern = new RegExp(
			`(?<![A-Za-z0-9])${escapeRegExp(word)}(?![A-Za-z0-9])`,
		);
		if (pattern.test(text)) {
			findings.push({
				...where,
				rule: "machine-identity",
				excerpt: mask(word),
			});
		}
	}
	return findings;
}

export function scanFileName(file, allow) {
	const normalized = file.replace(/\\/g, "/");
	if (allow.files.some((pattern) => pattern.test(normalized))) return [];
	return SENSITIVE_FILES.some((pattern) => pattern.test(normalized))
		? [
				{
					file: normalized,
					line: 0,
					rule: "sensitive-file",
					excerpt: path.posix.basename(normalized),
				},
			]
		: [];
}

export function scanIdentity(commit, role, email, allow) {
	return allow.emails.some((pattern) => pattern.test(email))
		? []
		: [
				{
					commit,
					file: "(commit)",
					line: 0,
					rule: `${role}-email`,
					excerpt: mask(email),
				},
			];
}

function contentScanned(file, allow) {
	return (
		!SKIPPED_CONTENT.some((pattern) => pattern.test(file)) &&
		!allow.files.some((pattern) => pattern.test(file))
	);
}

export function scanPatch(patch, options, commit) {
	const findings = [];
	let file = null;
	let line = 0;
	for (const raw of patch.split(/\r?\n/)) {
		if (raw.startsWith("+++ ")) {
			file = raw === "+++ /dev/null" ? null : raw.slice(4).replace(/^b\//, "");
			if (file)
				findings.push(
					...scanFileName(file, options.allow).map((f) => ({ ...f, commit })),
				);
			continue;
		}
		const hunk = raw.match(/^@@ -\d+(?:,\d+)? \+(\d+)/);
		if (hunk) {
			line = Number(hunk[1]);
			continue;
		}
		if (!file) continue;
		if (raw.startsWith("+")) {
			if (contentScanned(file, options.allow)) {
				findings.push(
					...scanLine(raw.slice(1), { commit, file, line }, options),
				);
			}
			line += 1;
		} else if (!raw.startsWith("-") && !raw.startsWith("\\")) {
			line += 1;
		}
	}
	return findings;
}

const COMMIT_MARK = "\u0001commit\u0001";

export function scanLog(log, options) {
	const findings = [];
	for (const chunk of log.split(COMMIT_MARK).slice(1)) {
		const [header, ...rest] = chunk.split("\u0002");
		const [sha, authorEmail, committerEmail] = header.split("\u0000");
		const commit = sha.slice(0, 9);
		const body = rest.join("\u0002");
		const split = body.indexOf("\u0003");
		const message = split === -1 ? body : body.slice(0, split);
		const patch = split === -1 ? "" : body.slice(split + 1);
		findings.push(
			...scanIdentity(commit, "author", authorEmail, options.allow),
		);
		findings.push(
			...scanIdentity(commit, "committer", committerEmail, options.allow),
		);
		message.split(/\r?\n/).forEach((text, index) => {
			findings.push(
				...scanLine(
					text,
					{ commit, file: "(message)", line: index + 1 },
					options,
				),
			);
		});
		findings.push(...scanPatch(patch, options, commit));
	}
	return findings;
}

function git(args, input) {
	const result = spawnSync("git", args, {
		cwd: root,
		encoding: "utf8",
		input,
		maxBuffer: 1024 * 1024 * 512,
	});
	if (result.status !== 0)
		throw new Error(`git ${args.join(" ")}: ${result.stderr.trim()}`);
	return result.stdout;
}

function logFor(revisions) {
	return git([
		"log",
		"-p",
		"--no-color",
		"--no-ext-diff",
		"--unified=0",
		`--format=${COMMIT_MARK}%H%x00%ae%x00%ce%x02%B%x03`,
		...revisions,
	]);
}

export function rangesFromPrePush(stdin) {
	const ranges = [];
	for (const line of stdin.split(/\r?\n/)) {
		const [localRef, localSha, , remoteSha] = line.trim().split(/\s+/);
		if (!localRef || !localSha || ZERO_SHA.test(localSha)) continue;
		ranges.push(
			remoteSha && !ZERO_SHA.test(remoteSha)
				? [`${remoteSha}..${localSha}`]
				: [localSha, "--not", "--remotes"],
		);
	}
	return ranges;
}

function outgoingRange() {
	const upstream = spawnSync(
		"git",
		["rev-parse", "--abbrev-ref", "@{upstream}"],
		{
			cwd: root,
			encoding: "utf8",
		},
	);
	return upstream.status === 0
		? [`${upstream.stdout.trim()}..HEAD`]
		: ["HEAD", "--not", "--remotes"];
}

function scanTree(options) {
	const findings = [];
	for (const file of git(["ls-files", "-z"]).split("\0").filter(Boolean)) {
		findings.push(...scanFileName(file, options.allow));
		if (!contentScanned(file, options.allow)) continue;
		const full = path.join(root, file);
		if (!existsSync(full)) continue;
		const data = readFileSync(full);
		if (data.includes(0)) continue;
		data
			.toString("utf8")
			.split(/\r?\n/)
			.forEach((text, index) => {
				findings.push(...scanLine(text, { file, line: index + 1 }, options));
			});
	}
	return findings;
}

export function describe(finding) {
	const where = finding.line ? `${finding.file}:${finding.line}` : finding.file;
	const commit = finding.commit ? `${finding.commit} ` : "";
	return `${commit}${where}  [${finding.rule}] ${finding.excerpt}`;
}

export function privacyCheck({ mode = "outgoing", stdin = "" } = {}) {
	const allow = loadAllow();
	const identity = machineIdentity().filter(
		(word) => !allow.publicNames.includes(word.toLowerCase()),
	);
	const options = { allow, identity };
	if (mode === "tree")
		return { scope: "tracked files at HEAD", findings: scanTree(options) };
	if (mode === "staged") {
		const patch = git([
			"diff",
			"--cached",
			"--no-color",
			"--no-ext-diff",
			"--unified=0",
		]);
		return {
			scope: "staged changes",
			findings: scanPatch(patch, options, null),
		};
	}
	const ranges = mode === "hook" ? rangesFromPrePush(stdin) : [outgoingRange()];
	const findings = ranges.flatMap((range) => {
		try {
			return scanLog(logFor(range), options);
		} catch {
			return scanLog(
				logFor([range[0].split("..").pop(), "--not", "--remotes"]),
				options,
			);
		}
	});
	return {
		scope:
			ranges.map((range) => range.join(" ")).join(", ") || "nothing to push",
		findings,
	};
}
