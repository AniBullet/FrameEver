import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import {
	copyFileSync,
	existsSync,
	mkdirSync,
	mkdtempSync,
	readdirSync,
	readFileSync,
	rmSync,
	writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";
import { gitBash } from "./fe.mjs";

const repositoryRoot = fileURLToPath(new URL("../", import.meta.url));
const patchDirectory = "apps/desktop-gpui/patches";

test("GPUI patches remain applicable after a Windows-style Git checkout", (t) => {
	const fixture = mkdtempSync(path.join(tmpdir(), "cap-gpui-patch-checkout-"));
	t.after(() => rmSync(fixture, { recursive: true, force: true }));
	const emptyConfig = path.join(fixture, "empty.gitconfig");
	writeFileSync(emptyConfig, "");
	const env = {
		...Object.fromEntries(
			Object.entries(process.env).filter(([key]) => !key.startsWith("GIT_")),
		),
		GIT_CONFIG_NOSYSTEM: "1",
		GIT_CONFIG_GLOBAL: emptyConfig,
		GIT_ATTR_NOSYSTEM: "1",
	};
	const git = (...args) => {
		const result = spawnSync("git", args, {
			cwd: fixture,
			env,
			encoding: "utf8",
			maxBuffer: 4 * 1024 * 1024,
		});
		assert.ifError(result.error);
		assert.equal(result.status, 0, `git ${args.join(" ")}: ${result.stderr}`);
		return result.stdout;
	};

	git("init", "--quiet");
	git("config", "core.autocrlf", "true");
	git("config", "core.attributesFile", emptyConfig);
	copyFileSync(
		path.join(repositoryRoot, ".gitattributes"),
		path.join(fixture, ".gitattributes"),
	);
	mkdirSync(path.join(fixture, patchDirectory), { recursive: true });
	const patches = readdirSync(path.join(repositoryRoot, patchDirectory))
		.filter((file) => file.endsWith(".patch"))
		.map((file) => `${patchDirectory}/${file}`);
	assert.ok(patches.length > 0);
	const originalBytes = new Map();
	for (const patch of patches) {
		const bytes = readFileSync(path.join(repositoryRoot, patch));
		assert.equal(bytes.includes(Buffer.from("\r\n")), false, patch);
		originalBytes.set(patch, bytes);
		writeFileSync(path.join(fixture, patch), bytes);
	}
	git("add", "--", ".gitattributes", ...patches);
	for (const patch of patches) rmSync(path.join(fixture, patch));
	git("checkout-index", "--force", "--", ...patches);
	for (const patch of patches) {
		assert.deepEqual(
			readFileSync(path.join(fixture, patch)),
			originalBytes.get(patch),
			`${patch} must retain LF line endings`,
		);
		assert.notEqual(
			git("apply", "--numstat", "--unidiff-zero", "--", patch),
			"",
		);
	}
});

function prepareFixture(t) {
	const fixture = mkdtempSync(path.join(tmpdir(), "frameever-gpui-prepare-"));
	t.after(() => rmSync(fixture, { recursive: true, force: true }));
	const repository = path.join(fixture, "frameever");
	const patches = path.join(repository, patchDirectory);
	mkdirSync(patches, { recursive: true });
	mkdirSync(path.join(repository, "scripts"));
	copyFileSync(
		path.join(repositoryRoot, "scripts", "prepare-gpui-dependency.sh"),
		path.join(repository, "scripts", "prepare-gpui-dependency.sh"),
	);

	const mirror = path.join(fixture, "mirror", "fake-gpui");
	const source = path.join(fixture, "source", "fake-gpui-1.0.0");
	mkdirSync(mirror, { recursive: true });
	mkdirSync(path.join(source, "src"), { recursive: true });
	writeFileSync(path.join(source, "src", "lib.rs"), "pub fn original() {}\n");
	const archive = path.join(mirror, "fake-gpui-1.0.0.crate");
	const tar = spawnSync(
		"tar",
		["-czf", "../mirror/fake-gpui/fake-gpui-1.0.0.crate", "fake-gpui-1.0.0"],
		{ cwd: path.dirname(source), encoding: "utf8" },
	);
	assert.equal(tar.status, 0, tar.stderr);

	const checksum = createHash("sha256")
		.update(readFileSync(archive))
		.digest("hex");
	const pins = path.join(patches, "gpui-pre.pins");
	const pin = (value) => writeFileSync(pins, `fake-gpui 1.0.0 ${value} gpui\n`);
	pin(checksum);
	writeFileSync(
		path.join(patches, "gpui-pre-windows.patch"),
		[
			"diff --git a/crates/gpui/src/lib.rs b/crates/gpui/src/lib.rs",
			"--- a/crates/gpui/src/lib.rs",
			"+++ b/crates/gpui/src/lib.rs",
			"@@ -1 +1 @@",
			"-pub fn original() {}",
			"+pub fn patched() {}",
			"",
		].join("\n"),
	);

	const target = path.join(fixture, "gpui-pre-patched");
	const prepare = () =>
		spawnSync(
			gitBash(),
			[
				path
					.join(repository, "scripts", "prepare-gpui-dependency.sh")
					.replaceAll("\\", "/"),
			],
			{
				encoding: "utf8",
				env: {
					...process.env,
					FRAMEEVER_CRATES_URL: `file:///${path.join(fixture, "mirror").replaceAll("\\", "/")}`,
				},
			},
		);
	return { patches, pin, prepare, target };
}

test("prepare unpacks the pinned crates and applies the patch", (t) => {
	const { prepare, target } = prepareFixture(t);
	const result = prepare();
	assert.equal(result.status, 0, result.stderr);
	assert.match(
		readFileSync(path.join(target, "crates", "gpui", "src", "lib.rs"), "utf8"),
		/patched/,
	);
});

test("prepare leaves a current folder alone and rebuilds a stale one", (t) => {
	const { patches, prepare, target } = prepareFixture(t);
	assert.equal(prepare().status, 0);
	const marker = path.join(target, "marker");
	writeFileSync(marker, "");
	assert.equal(prepare().status, 0);
	assert.ok(existsSync(marker));

	const patch = path.join(patches, "gpui-pre-windows.patch");
	writeFileSync(
		patch,
		readFileSync(patch, "utf8").replace("patched", "repatched"),
	);
	const result = prepare();
	assert.equal(result.status, 0, result.stderr);
	assert.equal(existsSync(marker), false);
	assert.match(
		readFileSync(path.join(target, "crates", "gpui", "src", "lib.rs"), "utf8"),
		/repatched/,
	);
});

test("prepare refuses a crate that does not match its pin", (t) => {
	const { pin, prepare, target } = prepareFixture(t);
	pin("0".repeat(64));
	const result = prepare();
	assert.notEqual(result.status, 0);
	assert.match(result.stderr, /pinned checksum/);
	assert.equal(existsSync(target), false);
});

test("prepare refuses a folder it did not create", (t) => {
	const { prepare, target } = prepareFixture(t);
	mkdirSync(target);
	writeFileSync(path.join(target, "keep"), "");
	const result = prepare();
	assert.notEqual(result.status, 0);
	assert.ok(existsSync(path.join(target, "keep")));
});
