import assert from "node:assert/strict";
import { mkdirSync, mkdtempSync, readFileSync, rmSync, utimesSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import test from "node:test";
import {
	changelogSection,
	firstRecording,
	isLargeAddressAware,
	MISANS_WEIGHTS,
	missingMisans,
	missingNativeDeps,
	missingNsisFiles,
	NSIS,
	parseArgs,
	parseCargoUpdate,
	setLargeAddressAware,
	setWeightClass,
	SEMVER,
	stampReleaseDate,
	stampVersion,
	testProfileEnv,
	withFfmpeg,
} from "./fe.mjs";

test("flags take a value, an inline value, or stand alone", () => {
	assert.deepEqual(parseArgs(["hevc", "--project", "a.cap", "--watch", "--lang=zh-CN"]), {
		positional: ["hevc"],
		flags: { project: "a.cap", watch: true, lang: "zh-CN" },
	});
	assert.deepEqual(parseArgs(["--all", "--skip-tests"]).flags, {
		all: true,
		"skip-tests": true,
	});
});

test("FFmpeg goes first on the path, keeping the host's own path key", () => {
	const env = withFfmpeg({ Path: "C:\\Windows" });
	assert.equal(Object.keys(env).filter((key) => key.toLowerCase() === "path").length, 1);
	assert.match(env.Path, /target[\\/]ffmpeg[\\/]bin.C:\\Windows$/);
	const profile = testProfileEnv({ PATH: "/bin" });
	assert.match(profile.CAP_GPUI_APP_DATA_DIR, /target[\\/]testdata[\\/]appdata$/);
	assert.match(profile.CAP_GPUI_RECORDINGS_DIR, /testdata[\\/]recordings$/);
});

test("the newest .cap bundle is the test recording", () => {
	const dir = mkdtempSync(path.join(tmpdir(), "fe-"));
	assert.equal(firstRecording(path.join(dir, "missing")), null);
	mkdirSync(path.join(dir, "old.cap"));
	mkdirSync(path.join(dir, "new.cap"));
	mkdirSync(path.join(dir, "notes"));
	utimesSync(path.join(dir, "old.cap"), new Date(1000), new Date(1000));
	assert.equal(firstRecording(dir), path.join(dir, "new.cap"));
	rmSync(dir, { recursive: true });
});

test("packaging names each missing native dependency folder", () => {
	const dir = mkdtempSync(path.join(tmpdir(), "fe-"));
	assert.deepEqual(missingNativeDeps(dir), [
		"target/ffmpeg/bin",
		"target/native-deps/dxc",
		"target/native-deps/onnxruntime/lib",
	]);
	mkdirSync(path.join(dir, "target/ffmpeg/bin"), { recursive: true });
	writeFileSync(path.join(dir, "target/ffmpeg/bin/avcodec-61.dll"), "");
	assert.equal(missingNativeDeps(dir).length, 2);
	rmSync(dir, { recursive: true });
});

test("every MiSans weight the UI uses must be on disk", () => {
	const dir = mkdtempSync(path.join(tmpdir(), "fe-"));
	assert.equal(missingMisans(dir).length, MISANS_WEIGHTS.length);
	writeFileSync(path.join(dir, "MiSans-Regular.ttf"), "");
	assert.equal(missingMisans(dir).includes("MiSans-Regular.ttf"), false);
	rmSync(dir, { recursive: true });
});

test("rewriting a font's weight class changes only that field and keeps it valid", () => {
	const font = Buffer.alloc(12 + 2 * 16 + 64);
	font.writeUInt16BE(2, 4);
	font.write("OS/2", 12, "latin1");
	font.writeUInt32BE(44, 12 + 8);
	font.writeUInt32BE(8, 12 + 12);
	font.write("head", 28, "latin1");
	font.writeUInt32BE(52, 28 + 8);
	font.writeUInt32BE(12, 28 + 12);
	font.writeUInt16BE(330, 44 + 4);
	const out = setWeightClass(font, 400);
	assert.equal(out.readUInt16BE(44 + 4), 400);
	assert.equal(font.readUInt16BE(44 + 4), 330);
	let sum = 0;
	for (let index = 0; index < out.length; index += 4) sum = (sum + out.readUInt32BE(index)) >>> 0;
	assert.equal(sum, 0xb1b0afba);
});

test("marking a PE image large-address-aware sets only that flag", () => {
	const image = Buffer.alloc(0x100);
	image.writeUInt32LE(0x80, 0x3c);
	image.write("PE\0\0", 0x80, "latin1");
	image.writeUInt16LE(0x010f, 0x80 + 22);
	assert.equal(isLargeAddressAware(image), false);
	const patched = setLargeAddressAware(image);
	assert.equal(isLargeAddressAware(patched), true);
	assert.equal(patched.readUInt16LE(0x80 + 22), 0x012f);
	assert.equal(isLargeAddressAware(image), false);
	patched.writeUInt16LE(0x012f, 0x80 + 22);
	assert.deepEqual(setLargeAddressAware(patched), patched);
	assert.throws(() => isLargeAddressAware(Buffer.alloc(0x100)));
});

test("cargo update output splits into applied changes and held-back versions", () => {
	const output = [
		"    Updating crates.io index",
		"     Locking 3 packages to latest compatible versions",
		"    Updating serde v1.0.200 -> v1.0.210",
		"      Adding windows-core v0.62.2",
		"    Removing wasi v0.14.7+wasi-0.2.4",
		" Downgrading ort v2.0.0-rc.13 -> v2.0.0-rc.12 (available: v2.0.0-rc.13)",
		"   Unchanged wgpu v25.0.2 (available: v27.0.1)",
		"   Unchanged ort v2.0.0-rc.12 (available: v2.0.0-rc.15)",
		"note: pass `--verbose` to see 2 unchanged dependencies behind latest",
	].join("\r\n");
	const { changes, behind } = parseCargoUpdate(output);
	assert.equal(changes.length, 4);
	assert.deepEqual(behind, [
		"Unchanged wgpu v25.0.2 (available: v27.0.1)",
		"Unchanged ort v2.0.0-rc.12 (available: v2.0.0-rc.15)",
	]);
});

test("the NSIS toolset constants follow the pinned Tauri CLI", () => {
	const manifest = JSON.parse(
		readFileSync(new URL("../apps/desktop-gpui/package.json", import.meta.url), "utf8"),
	);
	assert.equal(
		manifest.devDependencies?.["@tauri-apps/cli"] ?? manifest.dependencies?.["@tauri-apps/cli"],
		NSIS.tauriCli,
		"Tauri CLI changed: copy NSIS_URL, NSIS_SHA1, NSIS_TAURI_UTILS_* and NSIS_REQUIRED_FILES from tauri-bundler's nsis/mod.rs into NSIS in scripts/fe.mjs",
	);
});

test("stamping a version rewrites only the package version", () => {
	const dir = mkdtempSync(path.join(tmpdir(), "fe-version-"));
	const manifest = path.join(dir, "Cargo.toml");
	const cargo = (version) =>
		["[package]", 'name = "app"', `version = "${version}"`, "", "[dependencies]", 'serde = { version = "1" }', ""].join("\n");
	writeFileSync(manifest, cargo("1.0.0-dev"));
	stampVersion("1.2.3", [manifest]);
	assert.equal(readFileSync(manifest, "utf8"), cargo("1.2.3"));
	rmSync(dir, { recursive: true, force: true });
});

test("the NSIS toolset counts as installed only when every file Tauri checks exists", () => {
	const dir = mkdtempSync(path.join(tmpdir(), "fe-nsis-"));
	assert.equal(missingNsisFiles(dir).length, NSIS.required.length);
	for (const file of NSIS.required) {
		mkdirSync(path.dirname(path.join(dir, file)), { recursive: true });
		writeFileSync(path.join(dir, file), "");
	}
	assert.deepEqual(missingNsisFiles(dir), []);
	assert.ok(NSIS.required.includes(NSIS.compiler));
	assert.ok(NSIS.required.includes(NSIS.utils));
	rmSync(dir, { recursive: true });
});

test("releasing dates only the unreleased entry for that version", () => {
	const text = "## [3.1.0] — 未发布\n\n- a\n\n## [3.0.0] — 2026-10-24\n";
	assert.equal(
		stampReleaseDate(text, "3.1.0", "2026-11-20"),
		"## [3.1.0] — 2026-11-20\n\n- a\n\n## [3.0.0] — 2026-10-24\n",
	);
	assert.equal(stampReleaseDate(text, "3.0.0", "2026-11-20"), text);
	assert.equal(stampReleaseDate(text, "3.1", "2026-11-20"), text);
	assert.equal(
		stampReleaseDate("## [3.1.0] — 未发布\r\n- a\r\n", "3.1.0", "2026-11-20"),
		"## [3.1.0] — 2026-11-20\r\n- a\r\n",
	);
});

test("release notes are the CHANGELOG section for exactly that version", () => {
	const text = [
		"# Changelog",
		"",
		"## [3.1.0] — 2026-11-01",
		"",
		"### 新增",
		"- 新功能",
		"",
		"---",
		"",
		"## [3.0.0] — 2026-10-24",
		"",
		"- 首个版本",
	].join("\r\n");
	assert.equal(changelogSection(text, "3.1.0"), "### 新增\n- 新功能");
	assert.equal(changelogSection(text, "3.0.0"), "- 首个版本");
	assert.equal(changelogSection(text, "3.0"), null);
	assert.equal(changelogSection(text, "2.9.0"), null);
});

test("release versions are plain semver with an optional pre-release", () => {
	for (const good of ["3.0.0", "3.0.0-beta.1", "10.2.3-rc"]) assert.match(good, SEMVER);
	for (const bad of ["v3.0.0", "3.0", "3.0.0.1", "three"]) assert.doesNotMatch(bad, SEMVER);
});

test("watch mode runs a copy so a rebuild can replace the binary", async () => {
	const { runCopyPath } = await import("./fe.mjs");
	assert.equal(
		runCopyPath(path.join("debug", "cap-gpui.exe")),
		path.join("debug", "cap-gpui.fe-run.exe"),
	);
});
