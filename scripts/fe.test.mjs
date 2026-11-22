import assert from "node:assert/strict";
import { spawn, spawnSync } from "node:child_process";
import {
	mkdirSync,
	mkdtempSync,
	readFileSync,
	rmSync,
	statSync,
	utimesSync,
	writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import test from "node:test";
import {
	changelogSection,
	converterAssetName,
	converterEnv,
	crateTestArgs,
	designCss,
	designDir,
	designJs,
	easeMove,
	expandResource,
	firstRecording,
	iconFiles,
	iconTarball,
	installedPayloadFiles,
	installerEnv,
	installerPayloadEnv,
	isLargeAddressAware,
	macAssetNames,
	macBundleConfig,
	missingChineseFont,
	missingNativeDeps,
	missingNsisFiles,
	NSIS,
	parseArgs,
	parseCargoUpdate,
	patchedGpuiPlatformCrate,
	SEMVER,
	setLargeAddressAware,
	setupTarget,
	signatureKeyId,
	staleIncrementalDirs,
	stampReleaseDate,
	stampVersion,
	testProfileEnv,
	updateManifest,
	vendoredIcon,
	versionManifests,
	withFfmpeg,
	writeFakeCapAniBullet2,
} from "./fe.mjs";

const sampleTokens = () => ({
	brand: {
		dark: {
			neutral: [[0.1, 0.01, 266]],
			fixed: { overlay: [0.1, 0.01, 266, 0.6] },
		},
		light: {
			neutral: [[0.9, 0.01, 266]],
			fixed: { overlay: [0.2, 0.04, 266, 0.6] },
		},
	},
	semantic: [
		{ name: "muted_foreground", dark: "neutral.1", light: "neutral.1" },
		{ name: "overlay", dark: "fixed.overlay", light: "fixed.overlay" },
	],
	componentVars: [
		{ name: "slider_bar", dark: "muted_foreground", light: "fixed.overlay" },
	],
	type: {
		scale: [{ name: "body", size: 13.5, line: 20, weight: 400, tracking: "0" }],
	},
	space: { "0-5": 2 },
	radius: { md: 12, full: 999 },
	size: { row: 34 },
	motion: {
		duration: { fast: 120 },
		ease: { effects: "cubic-bezier(0.2, 0, 0, 1)" },
		move: { zeta: 0.75, omega: 10 },
	},
});

test("design tokens become one CSS rule per theme, with semantic names pointing at the ramps", () => {
	const css = designCss(sampleTokens());
	const [dark, light] = css.split('[data-theme="light"]');
	assert.match(dark, /--neutral-1: oklch\(0\.1 0\.01 266\);/);
	assert.match(light, /--neutral-1: oklch\(0\.9 0\.01 266\);/);
	assert.match(dark, /--muted-foreground: var\(--neutral-1\);/);
	assert.match(dark, /--overlay: oklch\(0\.1 0\.01 266 \/ 0\.6\);/);
	assert.match(light, /--slider-bar: var\(--overlay\);/);
	assert.match(dark, /--text-body: 13\.5px;/);
	assert.match(dark, /--radius-md: 12px;/);
	assert.match(dark, /--duration-fast: 120ms;/);
	assert.match(dark, /--ease-effects: cubic-bezier\(0\.2, 0, 0, 1\);/);
	assert.match(dark, /--ease-move: linear\(/);
});

test("a semantic token named like its fixed value keeps the value instead of pointing at itself", () => {
	const css = designCss(sampleTokens());
	assert.doesNotMatch(css, /--overlay: var\(--overlay\);/);
	assert.equal(css.match(/--overlay: oklch/g).length, 2);
});

test("a token pointing at nothing stops the build", () => {
	const tokens = sampleTokens();
	tokens.semantic[0].light = "neutral.2";
	assert.throws(() => designCss(tokens), /neutral\.2/);
	tokens.semantic[0].light = "neutral.1";
	tokens.componentVars[0].dark = "nonsense";
	assert.throws(() => designCss(tokens), /nonsense/);
});

test("the movement curve lands on 1 and overshoots by under 3%", () => {
	const points = easeMove({ zeta: 0.75, omega: 10 });
	assert.equal(points.at(-1), 1);
	assert.equal(points[0], 0);
	assert.ok(Math.max(...points) > 1 && Math.max(...points) < 1.03);
});

test("the committed design tokens match tokens.json", () => {
	const read = (name) =>
		readFileSync(path.join(designDir, name), "utf8").replaceAll("\r\n", "\n");
	const tokens = JSON.parse(read("tokens.json"));
	assert.equal(read("tokens.css"), designCss(tokens), "Run: pnpm fe design");
	assert.equal(read("tokens.js"), designJs(tokens), "Run: pnpm fe design");
});

test("flags take a value, an inline value, or stand alone", () => {
	assert.deepEqual(
		parseArgs(["hevc", "--project", "a.ever", "--watch", "--lang=zh-CN"]),
		{
			positional: ["hevc"],
			flags: { project: "a.ever", watch: true, lang: "zh-CN" },
		},
	);
	assert.deepEqual(parseArgs(["--all", "--skip-tests"]).flags, {
		all: true,
		"skip-tests": true,
	});
});

test("a filtered root-workspace crate test runs that crate's library tests", () => {
	assert.deepEqual(crateTestArgs("frameever-utils", ["chapter"], {}), [
		"test",
		"-p",
		"frameever-utils",
		"--lib",
		"chapter",
	]);
	assert.deepEqual(
		crateTestArgs("frameever-recording", ["click_highlight"], {
			ignored: true,
			optimized: true,
		}),
		[
			"test",
			"--config",
			"profile.dev.package.frameever-recording.opt-level=3",
			"-p",
			"frameever-recording",
			"--lib",
			"click_highlight",
			"--",
			"--ignored",
			"--nocapture",
		],
	);
	assert.throws(() => crateTestArgs(true, [], {}), /--crate <package>/);
});

test("FFmpeg goes first on the path, keeping the host's own path key", () => {
	const env = withFfmpeg({ Path: "C:\\Windows" });
	assert.equal(
		Object.keys(env).filter((key) => key.toLowerCase() === "path").length,
		1,
	);
	assert.match(env.Path, /target[\\/]ffmpeg[\\/]bin.C:\\Windows$/);
	const profile = testProfileEnv({ PATH: "/bin" });
	assert.match(
		profile.FRAMEEVER_GPUI_APP_DATA_DIR,
		/target[\\/]testdata[\\/]appdata$/,
	);
	assert.match(profile.FRAMEEVER_GPUI_STORAGE_DIR, /testdata[\\/]storage$/);
});

test("the newest .ever bundle is the test recording", () => {
	const dir = mkdtempSync(path.join(tmpdir(), "fe-"));
	assert.equal(firstRecording(path.join(dir, "missing")), null);
	mkdirSync(path.join(dir, "old.ever"));
	mkdirSync(path.join(dir, "new.ever"));
	mkdirSync(path.join(dir, "notes"));
	utimesSync(path.join(dir, "old.ever"), new Date(1000), new Date(1000));
	assert.equal(firstRecording(dir), path.join(dir, "new.ever"));
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

test("the Chinese UI font and its licence must be on disk", () => {
	const dir = mkdtempSync(path.join(tmpdir(), "fe-"));
	assert.deepEqual(missingChineseFont(dir), [
		"HarmonyOS_Sans_SC.ttf",
		"LICENSE-HarmonyOS-Sans.txt",
	]);
	writeFileSync(path.join(dir, "HarmonyOS_Sans_SC.ttf"), "");
	assert.deepEqual(missingChineseFont(dir), ["LICENSE-HarmonyOS-Sans.txt"]);
	writeFileSync(path.join(dir, "LICENSE-HarmonyOS-Sans.txt"), "");
	assert.deepEqual(missingChineseFont(dir), []);
	rmSync(dir, { recursive: true });
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
		readFileSync(
			new URL("../apps/desktop-gpui/package.json", import.meta.url),
			"utf8",
		),
	);
	assert.equal(
		manifest.devDependencies?.["@tauri-apps/cli"] ??
			manifest.dependencies?.["@tauri-apps/cli"],
		NSIS.tauriCli,
		"Tauri CLI changed: copy NSIS_URL, NSIS_SHA1, NSIS_TAURI_UTILS_* and NSIS_REQUIRED_FILES from tauri-bundler's nsis/mod.rs into NSIS in scripts/fe.mjs",
	);
});

test("stamping a version rewrites only the package version", () => {
	const dir = mkdtempSync(path.join(tmpdir(), "fe-version-"));
	const manifest = path.join(dir, "Cargo.toml");
	const cargo = (version) =>
		[
			"[package]",
			'name = "app"',
			`version = "${version}"`,
			"",
			"[dependencies]",
			'serde = { version = "1" }',
			"",
		].join("\n");
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
	for (const good of ["3.0.0", "3.0.0-beta.1", "10.2.3-rc"])
		assert.match(good, SEMVER);
	for (const bad of ["v3.0.0", "3.0", "3.0.0.1", "three"])
		assert.doesNotMatch(bad, SEMVER);
});

test("watch mode runs a copy so a rebuild can replace the binary", async () => {
	const { runCopyPath } = await import("./fe.mjs");
	assert.equal(
		runCopyPath(path.join("debug", "frameever-gpui.exe")),
		path.join("debug", "frameever-gpui.fe-run.exe"),
	);
});

test("a new dev run first stops the previous dev copy, never the installed app", async () => {
	const { previousDevAppStops } = await import("./fe.mjs");
	const copy = path.join("debug", "frameever-gpui.fe-run.exe");
	assert.deepEqual(previousDevAppStops("4242\n", copy, true), [
		[
			"taskkill",
			[
				"/F",
				"/T",
				"/FI",
				"IMAGENAME eq frameever-gpui.fe-run.exe",
				"/PID",
				"4242",
			],
		],
		["taskkill", ["/F", "/IM", "frameever-gpui.fe-run.exe"]],
	]);
	assert.deepEqual(previousDevAppStops("", copy, true), [
		["taskkill", ["/F", "/IM", "frameever-gpui.fe-run.exe"]],
	]);
});

test("on macOS a recorded PID is stopped only while the test app has it", async () => {
	const { previousDevAppStops } = await import("./fe.mjs");
	const copy = "/repo/target/debug/frameever-gpui.fe-run";
	const stops = (command) =>
		previousDevAppStops("4242", copy, false, () => command);
	assert.deepEqual(stops(copy), [["kill", ["4242"]]]);
	assert.deepEqual(stops("/bin/zsh"), []);
	assert.deepEqual(stops("/repo/target/debug/frameever-gpui"), []);
	assert.deepEqual(stops(""), []);
});

test("a recorded PID that now belongs to another program is left running", {
	skip: process.platform !== "win32",
}, async () => {
	const { previousDevAppStops } = await import("./fe.mjs");
	const other = spawn(process.execPath, ["-e", "setTimeout(() => {}, 30000)"], {
		stdio: "ignore",
	});
	const exited = new Promise((resolve) => other.once("exit", resolve));
	try {
		const copy = `frameever-gpui.test-${process.pid}.exe`;
		const stops = previousDevAppStops(String(other.pid), copy, true);
		for (const [command, args] of stops) {
			spawnSync(command, args, { stdio: "ignore" });
		}
		const outcome = await Promise.race([
			exited.then(() => "stopped"),
			new Promise((resolve) => setTimeout(resolve, 500, "running")),
		]);
		assert.equal(outcome, "running");
	} finally {
		other.kill();
	}
});

test("each crate keeps its newest incremental caches and drops the rest", () => {
	const dir = mkdtempSync(path.join(tmpdir(), "fe-"));
	assert.deepEqual(staleIncrementalDirs(path.join(dir, "missing")), []);
	const ages = {
		"frameever_gpui-aaa": 1,
		"frameever_gpui-bbb": 2,
		"frameever_gpui-ccc": 3,
		"frameever_editor-ddd": 1,
		"build_script_build-eee": 5,
	};
	for (const [name, age] of Object.entries(ages)) {
		mkdirSync(path.join(dir, name));
		const time = new Date(Date.now() - age * 60_000);
		utimesSync(path.join(dir, name), time, time);
	}
	assert.deepEqual(staleIncrementalDirs(dir, 2), [
		path.join(dir, "frameever_gpui-ccc"),
	]);
	rmSync(dir, { recursive: true });
});

test("the converter download is named after the release version", () => {
	assert.equal(
		converterAssetName("1.2.3"),
		"FrameEver-Converter_1.2.3_x64.exe",
	);
});

test("a release stamps the converter with the same version as the app", () => {
	assert.ok(
		versionManifests.some((manifest) =>
			manifest.endsWith(path.join("converter", "Cargo.toml")),
		),
	);
});

test("a release stamps the CLI with the same version as the app", () => {
	assert.ok(
		versionManifests.some((manifest) =>
			manifest.endsWith(path.join("apps", "cli", "Cargo.toml")),
		),
	);
});

test("a release stamps the installer front end with the same version as the app", () => {
	assert.ok(
		versionManifests.some((manifest) =>
			manifest.endsWith(path.join("installer", "Cargo.toml")),
		),
	);
});

test("the installer's space and progress count every file the NSIS installer copies", () => {
	const dir = mkdtempSync(path.join(tmpdir(), "fe-payload-"));
	const write = (relative, bytes) => {
		const file = path.join(dir, relative);
		mkdirSync(path.dirname(file), { recursive: true });
		writeFileSync(file, "x".repeat(bytes));
	};
	const triple = "x86_64-pc-windows-msvc";
	write(`target/${triple}/release/FrameEver.exe`, 10);
	write(`binaries/frameever-cli-${triple}.exe`, 5);
	write("assets/music/a.mp3", 3);
	write("assets/music/b.MP3", 3);
	write("assets/music/notes.txt", 100);
	write("target/installer/frameever-setup.exe", 2);
	const config = {
		bundle: {
			externalBin: ["binaries/frameever-cli", "binaries/frameever-muxer"],
			resources: { "assets/music/*.mp3": "assets/music/" },
		},
	};
	const files = installedPayloadFiles(triple, dir, config)
		.map((file) => path.relative(dir, file).replaceAll("\\", "/"))
		.sort();
	assert.deepEqual(files, [
		"assets/music/a.mp3",
		"assets/music/b.MP3",
		"binaries/frameever-cli-x86_64-pc-windows-msvc.exe",
		"target/installer/frameever-setup.exe",
		"target/x86_64-pc-windows-msvc/release/FrameEver.exe",
	]);
	assert.deepEqual(expandResource(dir, "missing/*"), []);
	rmSync(dir, { recursive: true });
});

test("the installer dev run fakes the existing install and never touches the real one", () => {
	const env = installerEnv("T", "older", "1.0.0", {});
	assert.equal(
		env.FRAMEEVER_INSTALLER_INSTALLED,
		`${path.join("T", "Programs", "FrameEver")}|0.9.0`,
	);
	assert.equal(env.LOCALAPPDATA, path.join("T", "Local"));
	assert.equal(
		installerEnv("T", null, "1.0.0", {}).FRAMEEVER_INSTALLER_INSTALLED,
		"none",
	);
	assert.ok(
		installerEnv(
			"T",
			"same",
			"1.0.0",
			{},
		).FRAMEEVER_INSTALLER_INSTALLED.endsWith("|1.0.0"),
	);
});

test("the release installer front end refuses a missing or empty NSIS payload", () => {
	const dir = mkdtempSync(path.join(tmpdir(), "fe-payload-"));
	const nsis = path.join(dir, "FrameEver_1.0.0_x64-setup.exe");
	assert.throws(() => installerPayloadEnv(nsis, 10, {}), /No NSIS installer/);
	writeFileSync(nsis, "");
	assert.throws(() => installerPayloadEnv(nsis, 10, {}), /is empty/);
	writeFileSync(nsis, "MZ");
	assert.throws(() => installerPayloadEnv(nsis, 0, {}), /installed size/);
	const env = installerPayloadEnv(nsis, 10, { KEEP: "1" });
	assert.equal(env.FRAMEEVER_INSTALLER_PAYLOAD, nsis);
	assert.equal(env.FRAMEEVER_INSTALLER_INSTALLED_BYTES, "10");
	assert.equal(env.KEEP, "1");
	rmSync(dir, { recursive: true });
});

test("the fake Cap-aniBullet 2.x install has projects, paired videos and a screenshot where 2.x kept them", () => {
	const dir = mkdtempSync(path.join(tmpdir(), "fe-converter-"));
	writeFakeCapAniBullet2(dir);
	const env = converterEnv(dir, {});
	const base = path.join(env.FRAMEEVER_CONVERTER_VIDEOS_DIR, "Cap Recordings");
	const store = JSON.parse(
		readFileSync(
			path.join(env.APPDATA, "so.cap.desktop.anibullet", "store"),
			"utf8",
		),
	);
	assert.equal(store.general_settings.recordingQuality, "high");
	for (const relative of [
		"recordings/Studio demo.cap/recording-meta.json",
		"exports/video/Instant clip.mp4",
		"exports/video/Instant clip_compressed.mp4",
		"exports/screenshot/Screenshot.png",
	]) {
		assert.ok(statSync(path.join(base, relative)).isFile(), relative);
	}
	assert.equal(env.FRAMEEVER_GPUI_APP_DATA_DIR, path.join(dir, "FrameEver"));
	rmSync(dir, { recursive: true, force: true });
});

test("a vendored icon keeps its source and takes the design stroke", () => {
	const lucide = {
		package: "lucide-static",
		version: "1.64.0",
		license: "ISC",
	};
	const svg =
		'<!-- @license lucide-static v1.64.0 - ISC -->\n<svg fill="none" stroke-width="2"><path/></svg>';
	assert.equal(
		vendoredIcon(svg, lucide, 1.75),
		'<!-- @license lucide-static v1.64.0 - ISC -->\n<svg fill="none" stroke-width="1.75"><path/></svg>',
	);
	assert.match(
		vendoredIcon(svg, lucide, 1.75, { filled: true }),
		/<svg fill="currentColor" stroke-width="1.75">/,
	);
});

test("a Tabler icon gains the licence line Lucide's files already carry", () => {
	const tabler = {
		package: "@tabler/icons",
		version: "3.50.0",
		license: "MIT",
	};
	const icon = vendoredIcon('<svg stroke-width="2"/>', tabler, 1.75);
	assert.equal(
		icon,
		'<!-- @license @tabler/icons v3.50.0 - MIT -->\n<svg stroke-width="1.75"/>',
	);
});

test("an icon without a 2 px stroke is refused", () => {
	assert.throws(
		() =>
			vendoredIcon(
				"<svg/>",
				{ package: "x", version: "1", license: "MIT" },
				1.75,
			),
		/2 px stroke/,
	);
});

test("icon tarballs come from the release pinned in the manifest", () => {
	assert.equal(
		iconTarball({ package: "@tabler/icons", version: "3.50.0" }),
		"https://registry.npmjs.org/@tabler/icons/-/icons-3.50.0.tgz",
	);
});

test("the filled variants name their Lucide source", () => {
	const files = iconFiles({
		lucide: ["x"],
		lucideFilled: { "record-fill": "circle" },
		tabler: ["blur"],
	});
	assert.deepEqual(files, [
		{ file: "x", release: "lucide", source: "x", filled: false },
		{ file: "record-fill", release: "lucide", source: "circle", filled: true },
		{ file: "blur", release: "tabler", source: "blur", filled: false },
	]);
});

test("patched gpui tests run the platform crate of the machine", () => {
	assert.equal(patchedGpuiPlatformCrate("win32"), "gpui_windows");
	assert.equal(patchedGpuiPlatformCrate("darwin"), "gpui_apple");
});

test("setup prepares native dependencies for the machine it runs on", () => {
	assert.equal(setupTarget("win32", "x64"), "x86_64-pc-windows-msvc");
	assert.equal(setupTarget("darwin", "arm64"), "aarch64-apple-darwin");
	assert.equal(setupTarget("darwin", "x64"), "x86_64-apple-darwin");
});

test("on macOS the FFmpeg dylibs are found from any target folder", () => {
	const fallback = withFfmpeg(
		{ PATH: "/bin", DYLD_FALLBACK_LIBRARY_PATH: "/usr/lib" },
		"darwin",
	).DYLD_FALLBACK_LIBRARY_PATH;
	assert.match(fallback, /target[\\/]native-deps[\\/]lib:/);
	assert.match(
		fallback,
		/target[\\/]Frameworks[\\/]Spacedrive\.framework[\\/]Libraries:/,
	);
	assert.match(fallback, /:\/usr\/lib$/);
	assert.equal(
		withFfmpeg({ PATH: "/bin" }, "win32").DYLD_FALLBACK_LIBRARY_PATH,
		undefined,
	);
});

test("macOS packaging names its own missing native dependencies", () => {
	const dir = mkdtempSync(path.join(tmpdir(), "fe-"));
	assert.deepEqual(missingNativeDeps(dir, "darwin"), [
		"target/native-deps/Spacedrive.framework",
		"target/native-deps/onnxruntime/lib",
	]);
	mkdirSync(
		path.join(dir, "target/native-deps/Spacedrive.framework/Libraries"),
		{
			recursive: true,
		},
	);
	mkdirSync(path.join(dir, "target/native-deps/onnxruntime/lib"), {
		recursive: true,
	});
	writeFileSync(
		path.join(dir, "target/native-deps/onnxruntime/lib/libonnxruntime.dylib"),
		"",
	);
	assert.deepEqual(missingNativeDeps(dir, "darwin"), []);
	rmSync(dir, { recursive: true });
});

test("a macOS package is named after its version and architecture", () => {
	assert.deepEqual(macAssetNames("1.0.0", "aarch64-apple-darwin"), {
		dmg: "FrameEver_1.0.0_aarch64.dmg",
		archive: "FrameEver_1.0.0_aarch64.app.tar.gz",
	});
	assert.equal(
		macAssetNames("1.0.0-beta.1", "x86_64-apple-darwin").archive,
		"FrameEver_1.0.0-beta.1_x64.app.tar.gz",
	);
});

test("without an Apple identity the macOS app is signed ad hoc, and the bundler never makes the updater archive", () => {
	assert.deepEqual(macBundleConfig({}), {
		bundle: {
			createUpdaterArtifacts: false,
			macOS: { signingIdentity: "-" },
		},
	});
	assert.deepEqual(
		macBundleConfig({
			TAURI_SIGNING_PRIVATE_KEY: "key",
			APPLE_SIGNING_IDENTITY: "Developer ID Application: aniBullet",
		}),
		{ bundle: { createUpdaterArtifacts: false } },
	);
});

const minisign = (keyId) =>
	Buffer.from(
		`untrusted comment: test\n${Buffer.concat([
			Buffer.from("Ed"),
			Buffer.from(keyId, "hex"),
			Buffer.alloc(32, 1),
		]).toString("base64")}\n`,
	).toString("base64");

test("a signature and a public key name the key that made them", () => {
	assert.equal(
		signatureKeyId(minisign("7a6f5207b3a2600b")),
		"7a6f5207b3a2600b",
	);
	assert.equal(
		signatureKeyId(`${minisign("0102030405060708")}\n`),
		"0102030405060708",
	);
});

test("the update manifest lists every platform that was built", () => {
	const key = "7a6f5207b3a2600b";
	const manifest = updateManifest({
		version: "1.0.0",
		notes: "First release\n",
		pubDate: "2026-11-15T00:00:00Z",
		repo: "AniBullet/FrameEver",
		publicKey: minisign(key),
		signatures: {
			"FrameEver_1.0.0_x64-setup.exe": `${minisign(key)}\n`,
			"FrameEver_1.0.0_aarch64.app.tar.gz": minisign(key),
		},
	});
	assert.deepEqual(manifest, {
		version: "1.0.0",
		notes: "First release\n",
		pub_date: "2026-11-15T00:00:00Z",
		platforms: {
			"windows-x86_64": {
				url: "https://github.com/AniBullet/FrameEver/releases/download/v1.0.0/FrameEver_1.0.0_x64-setup.exe",
				signature: minisign(key),
			},
			"darwin-aarch64": {
				url: "https://github.com/AniBullet/FrameEver/releases/download/v1.0.0/FrameEver_1.0.0_aarch64.app.tar.gz",
				signature: minisign(key),
			},
		},
	});
});

test("the update manifest refuses what an installed app could not use", () => {
	const key = "7a6f5207b3a2600b";
	const build = (signatures, publicKey = minisign(key)) =>
		updateManifest({
			version: "1.0.0",
			notes: "Notes",
			pubDate: "2026-11-15T00:00:00Z",
			repo: "AniBullet/FrameEver",
			publicKey,
			signatures,
		});
	assert.throws(
		() => build({ "FrameEver 1.0.0-setup.exe": minisign(key) }),
		/spaces/,
	);
	assert.throws(() => build({ "FrameEver_1.0.0_x64-setup.exe": " " }), /empty/);
	assert.throws(
		() =>
			build({ "FrameEver_1.0.0_x64-setup.exe": minisign("0102030405060708") }),
		/signed by key 0102030405060708/,
	);
	assert.throws(
		() =>
			build({
				"FrameEver_1.0.0_x64-setup.exe": minisign(key),
				"FrameEver_1.0.1_x64-setup.exe": minisign(key),
			}),
		/two packages for windows-x86_64/,
	);
	assert.throws(() => build({ "notes.md": minisign(key) }), /no platform/i);
	assert.throws(() => build({}), /no signed package/i);
	assert.throws(
		() =>
			build(
				{ "FrameEver_1.0.0_x64-setup.exe": "bm90IGEga2V5" },
				"bm90IGEga2V5",
			),
		/not a minisign/i,
	);
});
