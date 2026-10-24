import { spawn, spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import {
	copyFileSync,
	existsSync,
	mkdirSync,
	readdirSync,
	readFileSync,
	renameSync,
	rmSync,
	statSync,
	watch,
	writeFileSync,
} from "node:fs";
import path from "node:path";
import { createInterface } from "node:readline/promises";
import { fileURLToPath } from "node:url";
import { describe as describeFinding, privacyCheck } from "./privacy.mjs";

export const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
export const gpuiDir = path.join(root, "apps", "desktop-gpui");
export const testdataDir = path.join(gpuiDir, "target", "testdata");
export const distDir = path.join(root, "dist");
export const target = "x86_64-pc-windows-msvc";
const ffmpegBin = path.join(root, "target", "ffmpeg", "bin");
export const MISANS_URL = "https://hyperos.mi.com/font-download/MiSans.zip";
export const MISANS_WEIGHTS = ["Regular", "Medium", "Semibold", "Bold"];
const MISANS_WEIGHT_CLASS = { Regular: 400, Medium: 500, Semibold: 600, Bold: 700 };
export const misansDir = path.join(gpuiDir, "assets", "fonts", "misans");
const isWindows = process.platform === "win32";

export const NSIS = {
	tauriCli: "2.11.2",
	url: "https://github.com/tauri-apps/binary-releases/releases/download/nsis-3.11/nsis-3.11.zip",
	sha1: "ef7ff767e5cbd9edd22add3a32c9b8f4500bb10d",
	folder: "nsis-3.11",
	utilsUrl:
		"https://github.com/tauri-apps/nsis-tauri-utils/releases/download/nsis_tauri_utils-v0.5.3/nsis_tauri_utils.dll",
	utilsSha1: "75197fee3c6a814fe035788d1c34ead39349b860",
	utils: "Plugins/x86-unicode/additional/nsis_tauri_utils.dll",
	compiler: "Bin/makensis.exe",
	required: [
		"makensis.exe",
		"Bin/makensis.exe",
		"Stubs/lzma-x86-unicode",
		"Stubs/lzma_solid-x86-unicode",
		"Plugins/x86-unicode/additional/nsis_tauri_utils.dll",
		"Include/MUI2.nsh",
		"Include/FileFunc.nsh",
		"Include/x64.nsh",
		"Include/nsDialogs.nsh",
		"Include/WinMessages.nsh",
		"Include/Win/COM.nsh",
		"Include/Win/Propkey.nsh",
		"Include/Win/RestartManager.nsh",
	],
};

const COMMANDS = {
	setup: "Prepare a fresh machine: JS deps, FFmpeg/ONNX/DXC, patched gpui, MiSans font",
	dev: "Build and run the app (test data by default). --watch, --lang zh-CN|en, --real",
	check: "Compile-check the GPUI app and the CLI, tests included",
	test: "Run GPUI tests (optional filter). --all adds encoder, export, CLI and script tests",
	lint: "Format check, clippy and translation tests",
	fmt: "Format the GPUI app and the root crates FrameEver changed",
	export: "Export the test recording: fe export h264|hevc|av1 [--project <.cap>]",
	package: "Build the Windows installer into dist/. --skip-sidecars, --version <x.y.z>",
	release: "Check, tag and push a release: fe release <x.y.z> [--skip-tests]",
	notes: "Print the CHANGELOG.md section for a version: fe notes <x.y.z>",
	deps: "Show Rust dependency updates within version ranges and newer versions needing a manual bump. --apply updates both Cargo.lock files",
	doctor: "Check the environment and say how to fix anything missing",
	logs: "Print the app log tail: fe logs [--lines 200] [--grep <text>] [--real]",
	privacy: "Scan what you are about to push for private data (emails, local paths, keys). --staged, --all (every tracked file)",
};

export function parseArgs(argv) {
	const flags = {};
	const positional = [];
	for (let index = 0; index < argv.length; index += 1) {
		const value = argv[index];
		if (!value.startsWith("--")) {
			positional.push(value);
			continue;
		}
		const [key, inline] = value.slice(2).split("=", 2);
		if (inline !== undefined) {
			flags[key] = inline;
		} else if (argv[index + 1] !== undefined && !argv[index + 1].startsWith("--")) {
			flags[key] = argv[index + 1];
			index += 1;
		} else {
			flags[key] = true;
		}
	}
	return { flags, positional };
}

function pathKey(env) {
	return Object.keys(env).find((key) => key.toLowerCase() === "path") ?? "PATH";
}

export function withFfmpeg(env = process.env) {
	const key = pathKey(env);
	return { ...env, [key]: `${ffmpegBin}${path.delimiter}${env[key] ?? ""}` };
}

export function testProfileEnv(env = process.env) {
	return {
		...withFfmpeg(env),
		CAP_GPUI_APP_DATA_DIR: path.join(testdataDir, "appdata"),
		CAP_GPUI_RECORDINGS_DIR: path.join(testdataDir, "recordings"),
		CAP_GPUI_LOG_DIR: path.join(testdataDir, "logs"),
	};
}

function step(title) {
	console.log(`\n\u001b[36m▸ ${title}\u001b[0m`);
}

function fail(message) {
	console.error(`\n\u001b[31m✗ ${message}\u001b[0m`);
	process.exit(1);
}

function run(command, args, options = {}) {
	console.log(`\u001b[2m$ ${[command, ...args].join(" ")}\u001b[0m`);
	const result = spawnSync(command, args, {
		stdio: "inherit",
		cwd: options.cwd ?? root,
		env: options.env ?? process.env,
		shell: options.shell ?? false,
	});
	if (result.error) fail(`${command}: ${result.error.message}`);
	if (result.status !== 0 && !options.allowFailure) {
		fail(`${command} exited with ${result.status ?? result.signal}`);
	}
	return result.status ?? 1;
}

function capture(command, args, options = {}) {
	const result = spawnSync(command, args, {
		cwd: options.cwd ?? root,
		encoding: "utf8",
		shell: options.shell ?? false,
	});
	return result.status === 0 ? result.stdout.trim() : null;
}

function pnpm(args) {
	return isWindows ? ["cmd", ["/c", "pnpm", ...args]] : ["pnpm", args];
}

export function gitBash() {
	if (!isWindows) return "bash";
	const candidates = [];
	const git = capture("where", ["git"]);
	if (git) {
		for (const line of git.split(/\r?\n/)) {
			candidates.push(path.join(path.dirname(path.dirname(line.trim())), "bin", "bash.exe"));
		}
	}
	candidates.push(
		"C:\\Program Files\\Git\\bin\\bash.exe",
		"D:\\Program Files\\Git\\bin\\bash.exe",
	);
	const found = candidates.find((candidate) => existsSync(candidate));
	if (!found) fail("Git Bash not found. Install Git for Windows.");
	return found;
}

function requireTool(command, args, name, hint) {
	const [program, programArgs] = command === "pnpm" ? pnpm(args) : [command, args];
	const version = capture(program, programArgs);
	if (!version) fail(`${name} is missing. ${hint}`);
	console.log(`  ${name}: ${version.split(/\r?\n/)[0]}`);
	return version;
}

function setup({ flags }) {
	step("Toolchain");
	const node = Number(process.versions.node.split(".")[0]);
	if (node < 20) fail(`Node ${process.versions.node} is too old; FrameEver needs Node 20+.`);
	console.log(`  node: ${process.versions.node}`);
	requireTool("pnpm", ["--version"], "pnpm", "Run: npm install -g pnpm@10.5.2");
	requireTool("rustup", ["--version"], "rustup", "Install Rust from https://rustup.rs");
	requireTool("cargo", ["--version"], "cargo", "Install Rust from https://rustup.rs");
	run("rustup", ["toolchain", "install", "stable", "--profile", "minimal"], {
		allowFailure: true,
	});

	if (!flags.ci && (!existsSync(path.join(root, "node_modules")) || flags.install)) {
		step("JavaScript dependencies");
		run(...pnpm(["install", "--frozen-lockfile"]));
	}

	step("Native dependencies (FFmpeg, ONNX Runtime, DXC) and .cargo/config.toml");
	run(process.execPath, [path.join("scripts", "setup.js")], {
		env: { ...process.env, RUST_TARGET_TRIPLE: target },
	});

	step("Patched gpui checkout (../zed-cap)");
	run(gitBash(), [path.join("scripts", "prepare-gpui-dependency.sh")]);

	step("MiSans (Chinese UI font, fetched from Xiaomi; its license forbids committing it)");
	fetchMisans();

	if (isWindows) {
		step("NSIS installer compiler (Tauri's toolset, made large-address-aware)");
		prepareNsis();
	}

	if (!flags.ci) {
		step("Git hooks (privacy check before every push)");
		run("git", ["config", "core.hooksPath", HOOKS_DIR]);
	}

	console.log("\n\u001b[32m✓ Ready. Next: pnpm fe dev\u001b[0m");
}

export function nsisToolsetDir(env = process.env) {
	const local = env.LOCALAPPDATA ?? path.join(env.USERPROFILE ?? "", "AppData", "Local");
	return path.join(local, "tauri", "NSIS");
}

export function missingNsisFiles(dir = nsisToolsetDir()) {
	return NSIS.required.filter((file) => !existsSync(path.join(dir, file)));
}

const PE_LARGE_ADDRESS_AWARE = 0x0020;

function peCharacteristicsOffset(image) {
	const header = image.readUInt32LE(0x3c);
	if (image.toString("latin1", header, header + 4) !== "PE\0\0") {
		throw new Error("not a PE image");
	}
	return header + 22;
}

export function isLargeAddressAware(image) {
	return (image.readUInt16LE(peCharacteristicsOffset(image)) & PE_LARGE_ADDRESS_AWARE) !== 0;
}

export function setLargeAddressAware(image) {
	const patched = Buffer.from(image);
	const offset = peCharacteristicsOffset(patched);
	patched.writeUInt16LE(patched.readUInt16LE(offset) | PE_LARGE_ADDRESS_AWARE, offset);
	return patched;
}

function sha1(data) {
	return createHash("sha1").update(data).digest("hex");
}

function downloadVerified(url, expectedSha1, file) {
	mkdirSync(path.dirname(file), { recursive: true });
	if (!existsSync(file) || sha1(readFileSync(file)) !== expectedSha1) {
		run("curl", ["-fSL", "-o", file, url]);
	}
	const actual = sha1(readFileSync(file));
	if (actual !== expectedSha1) {
		rmSync(file, { force: true });
		fail(`${url} has SHA1 ${actual}, expected ${expectedSha1}`);
	}
	return file;
}

export function prepareNsis() {
	const dir = nsisToolsetDir();
	if (missingNsisFiles(dir).length > 0) {
		const cache = path.join(root, "target", "native-deps");
		const archive = downloadVerified(NSIS.url, NSIS.sha1, path.join(cache, `${NSIS.folder}.zip`));
		const parent = path.dirname(dir);
		mkdirSync(parent, { recursive: true });
		rmSync(dir, { recursive: true, force: true });
		rmSync(path.join(parent, NSIS.folder), { recursive: true, force: true });
		run(systemTar(), ["-xf", archive, "-C", parent]);
		renameSync(path.join(parent, NSIS.folder), dir);
		const utils = downloadVerified(NSIS.utilsUrl, NSIS.utilsSha1, path.join(cache, "nsis_tauri_utils.dll"));
		mkdirSync(path.dirname(path.join(dir, NSIS.utils)), { recursive: true });
		copyFileSync(utils, path.join(dir, NSIS.utils));
	}
	const still = missingNsisFiles(dir);
	if (still.length > 0) fail(`NSIS toolset at ${dir} is missing ${still.join(", ")}`);
	const compiler = path.join(dir, NSIS.compiler);
	const image = readFileSync(compiler);
	if (!isLargeAddressAware(image)) {
		writeFileSync(compiler, setLargeAddressAware(image));
	}
	if (!isLargeAddressAware(readFileSync(compiler))) fail(`Could not mark ${compiler} large-address-aware`);
	console.log(`  ${compiler}: large-address-aware`);
}

function systemTar() {
	return isWindows ? path.join(process.env.SystemRoot ?? "C:\\Windows", "System32", "tar.exe") : "tar";
}

export function missingMisans(dir = misansDir) {
	return MISANS_WEIGHTS.map((weight) => `MiSans-${weight}.ttf`).filter(
		(name) => !existsSync(path.join(dir, name)),
	);
}

function tableChecksum(font, offset, length) {
	let sum = 0;
	const padded = Buffer.alloc(Math.ceil(length / 4) * 4);
	font.copy(padded, 0, offset, offset + length);
	for (let index = 0; index < padded.length; index += 4) {
		sum = (sum + padded.readUInt32BE(index)) >>> 0;
	}
	return sum;
}

export function setWeightClass(input, weightClass) {
	const font = Buffer.from(input);
	const tables = new Map();
	const count = font.readUInt16BE(4);
	for (let index = 0; index < count; index += 1) {
		const record = 12 + index * 16;
		tables.set(font.toString("latin1", record, record + 4), {
			record,
			offset: font.readUInt32BE(record + 8),
			length: font.readUInt32BE(record + 12),
		});
	}
	const os2 = tables.get("OS/2");
	const head = tables.get("head");
	font.writeUInt16BE(weightClass, os2.offset + 4);
	font.writeUInt32BE(tableChecksum(font, os2.offset, os2.length), os2.record + 4);
	font.writeUInt32BE(0, head.offset + 8);
	font.writeUInt32BE(tableChecksum(font, head.offset, head.length), head.record + 4);
	const total = tableChecksum(font, 0, font.length);
	font.writeUInt32BE((0xb1b0afba - total) >>> 0, head.offset + 8);
	return font;
}

function fetchMisans() {
	if (missingMisans().length === 0) {
		console.log("  Already present.");
		return;
	}
	const cache = path.join(root, "target", "native-deps", "MiSans.zip");
	mkdirSync(path.dirname(cache), { recursive: true });
	if (!existsSync(cache)) run("curl", ["-fSL", "-o", cache, MISANS_URL]);
	const staging = path.join(root, "target", "native-deps", "misans-extract");
	mkdirSync(staging, { recursive: true });
	const members = MISANS_WEIGHTS.map((weight) => `MiSans/ttf/MiSans-${weight}.ttf`);
	run(systemTar(), ["-xf", cache, "-C", staging, ...members]);
	mkdirSync(misansDir, { recursive: true });
	for (const weight of MISANS_WEIGHTS) {
		const source = path.join(staging, "MiSans", "ttf", `MiSans-${weight}.ttf`);
		const font = setWeightClass(readFileSync(source), MISANS_WEIGHT_CLASS[weight]);
		writeFileSync(path.join(misansDir, `MiSans-${weight}.ttf`), font);
	}
	const still = missingMisans();
	if (still.length > 0) fail(`MiSans files missing after extraction: ${still.join(", ")}`);
}

function writeTestLanguage(lang) {
	const appdata = path.join(testdataDir, "appdata");
	mkdirSync(appdata, { recursive: true });
	const storePath = path.join(appdata, "store");
	let store = {};
	try {
		store = JSON.parse(readFileSync(storePath, "utf8"));
	} catch {}
	store.general_settings = { ...(store.general_settings ?? {}), language: lang };
	writeFileSync(storePath, JSON.stringify(store, null, 2));
}

function cargoBuildDebug() {
	return spawnSync("cargo", ["build"], { cwd: gpuiDir, stdio: "inherit" }).status === 0;
}

function appBinary() {
	return path.join(gpuiDir, "target", "debug", isWindows ? "cap-gpui.exe" : "cap-gpui");
}

export const devPidFile = path.join(testdataDir, "fe-dev.pid");

export function runCopyPath(binary = appBinary()) {
	const extension = path.extname(binary);
	return path.join(path.dirname(binary), `${path.basename(binary, extension)}.fe-run${extension}`);
}

function copyForRun() {
	const copy = runCopyPath();
	for (let attempt = 0; attempt < 20; attempt += 1) {
		try {
			copyFileSync(appBinary(), copy);
			return copy;
		} catch (error) {
			if (attempt === 19) throw error;
			Atomics.wait(new Int32Array(new SharedArrayBuffer(4)), 0, 0, 250);
		}
	}
	return copy;
}

function launchApp(env) {
	const child = spawn(copyForRun(), [], { cwd: gpuiDir, env, stdio: "inherit" });
	child.on("error", (error) => console.error(`cap-gpui: ${error.message}`));
	if (child.pid) {
		mkdirSync(testdataDir, { recursive: true });
		writeFileSync(devPidFile, String(child.pid));
	}
	return child;
}

const WATCH_DIRS = [
	"apps/desktop-gpui/src",
	"apps/desktop-gpui/assets",
	"apps/desktop-gpui/kernels/src",
	...[
		"camera",
		"scap-targets",
		"recording",
		"timestamp",
		"utils",
		"project",
		"rendering",
		"editor",
		"export",
		"enc-ffmpeg",
	].map((crate) => `crates/${crate}/src`),
];

function dev({ flags }) {
	if (flags.lang && !["zh-CN", "en"].includes(flags.lang)) {
		fail("--lang takes zh-CN or en");
	}
	const isolated = !flags.real;
	if (isolated) {
		mkdirSync(path.join(testdataDir, "recordings"), { recursive: true });
		if (flags.lang) writeTestLanguage(flags.lang);
	} else if (flags.lang) {
		fail("--lang only applies to the test profile; change the language in Settings instead");
	}
	const env = {
		...(isolated ? testProfileEnv() : withFfmpeg()),
		CAP_GPUI_DEV_RESTORE: path.join(testdataDir, "dev-restore.json"),
	};
	console.log(
		isolated
			? `Test profile: ${testdataDir}`
			: "\u001b[33mUsing your real FrameEver data.\u001b[0m",
	);

	step("Build");
	if (!cargoBuildDebug()) fail("Build failed");
	let app = launchApp(env);
	if (!flags.watch) {
		app.on("exit", (code) => process.exit(code ?? 0));
		process.on("SIGINT", () => app.kill());
		return;
	}

	console.log("\u001b[36mWatching for changes. Ctrl+C to stop.\u001b[0m");
	let timer = null;
	let building = false;
	let pending = false;
	const rebuild = () => {
		if (building) {
			pending = true;
			return;
		}
		building = true;
		step("Change detected: rebuilding");
		const ok = cargoBuildDebug();
		building = false;
		if (ok) {
			app.removeAllListeners("exit");
			app.kill();
			app = launchApp(env);
			console.log("\u001b[32m✓ Relaunched\u001b[0m");
		} else {
			console.log("\u001b[31m✗ Build failed; the running app was kept\u001b[0m");
		}
		if (pending) {
			pending = false;
			rebuild();
		}
	};
	for (const dir of WATCH_DIRS) {
		const full = path.join(root, dir);
		if (!existsSync(full)) continue;
		watch(full, { recursive: true }, () => {
			clearTimeout(timer);
			timer = setTimeout(rebuild, 400);
		});
	}
	process.on("SIGINT", () => {
		app.kill();
		process.exit(0);
	});
}

function check() {
	step("GPUI app");
	run("cargo", ["check", "--tests"], { cwd: gpuiDir });
	step("CLI");
	run("cargo", ["check", "-p", "cap", "--tests"]);
	console.log("\n\u001b[32m✓ Everything compiles\u001b[0m");
}

function test({ flags, positional }) {
	step("GPUI tests");
	run("cargo", ["test", "--bin", "cap-gpui", ...positional], {
		cwd: gpuiDir,
		env: withFfmpeg(),
	});
	if (flags.all) {
		step("Encoder, export and CLI tests");
		run("cargo", ["test", "-p", "cap-enc-ffmpeg", "--lib"], { env: withFfmpeg() });
		run("cargo", ["test", "-p", "cap-export", "--lib"], { env: withFfmpeg() });
		run("cargo", ["test", "-p", "cap", "--bin", "cap"], { env: withFfmpeg() });
		step("Packaging script tests");
		run(process.execPath, [
			"--test",
			"scripts/bundle-gpui.test.mjs",
			"scripts/fe.test.mjs",
			"scripts/privacy.test.mjs",
		]);
	}
	console.log("\n\u001b[32m✓ Tests passed\u001b[0m");
}

const ROOT_CRATES = ["cap", "cap-export", "cap-enc-ffmpeg", "cap-utils", "cap-cli-install"];

function fmt() {
	run("cargo", ["fmt"], { cwd: gpuiDir });
	run("cargo", ["fmt", ...ROOT_CRATES.flatMap((crate) => ["-p", crate])]);
}

function lint() {
	step("Format");
	run("cargo", ["fmt", "--check"], { cwd: gpuiDir });
	step("Clippy");
	run("cargo", ["clippy", "--tests"], { cwd: gpuiDir });
	run("cargo", ["clippy", ...ROOT_CRATES.flatMap((crate) => ["-p", crate]), "--tests"]);
	step("Translations");
	run("cargo", ["test", "--bin", "cap-gpui", "i18n::"], { cwd: gpuiDir, env: withFfmpeg() });
	console.log("\n\u001b[32m✓ Lint passed\u001b[0m");
}

export function firstRecording(dir) {
	if (!existsSync(dir)) return null;
	const bundles = readdirSync(dir, { withFileTypes: true })
		.filter((entry) => entry.isDirectory() && entry.name.endsWith(".cap"))
		.map((entry) => path.join(dir, entry.name))
		.sort((a, b) => statSync(b).mtimeMs - statSync(a).mtimeMs);
	return bundles[0] ?? null;
}

function exportVideo({ flags, positional }) {
	const codec = positional[0] ?? "h264";
	if (!["h264", "hevc", "av1"].includes(codec)) fail("Codec must be h264, hevc or av1");
	const project = flags.project ?? firstRecording(path.join(testdataDir, "recordings"));
	if (!project) {
		fail("No test recording yet. Run `pnpm fe dev`, record something, then try again.");
	}
	const outDir = path.join(testdataDir, "exports");
	mkdirSync(outDir, { recursive: true });
	const output = path.join(outDir, `${path.basename(project, ".cap")}-${codec}.mp4`);
	run(
		"cargo",
		["run", "-q", "-p", "cap", "--", "export", project, "-o", output, "--codec", codec],
		{ env: withFfmpeg() },
	);
	const size = (statSync(output).size / 1024 / 1024).toFixed(1);
	console.log(`\n\u001b[32m✓ ${output} (${size} MB)\u001b[0m`);
}

export function missingNativeDeps(base = root) {
	const required = [
		["target/ffmpeg/bin", ".dll"],
		["target/native-deps/dxc", ".dll"],
		["target/native-deps/onnxruntime/lib", ".dll"],
	];
	return required
		.filter(([dir, extension]) => {
			const full = path.join(base, dir);
			return !existsSync(full) || !readdirSync(full).some((name) => name.endsWith(extension));
		})
		.map(([dir]) => dir);
}

const versionManifests = [path.join(gpuiDir, "Cargo.toml")];

export function stampVersion(version, manifests = versionManifests) {
	for (const manifest of manifests) {
		const text = readFileSync(manifest, "utf8");
		const next = text.replace(/^version = ".*"$/m, `version = "${version}"`);
		if (next === text && !text.includes(`version = "${version}"`)) {
			fail(`package.version not found in ${manifest}`);
		}
		writeFileSync(manifest, next);
	}
}

export const SEMVER = /^\d+\.\d+\.\d+(-[0-9A-Za-z.-]+)?$/;

function packageInstaller({ flags }) {
	if (!isWindows) fail("Packaging targets Windows; run it on Windows.");
	const missing = [...missingNativeDeps(), ...missingMisans()];
	if (missing.length > 0) {
		fail(`Native dependencies are missing (${missing.join(", ")}). Run: pnpm fe setup`);
	}
	step("NSIS installer compiler");
	prepareNsis();
	if (typeof flags.version === "string") {
		if (!SEMVER.test(flags.version)) fail(`Not a version: ${flags.version}`);
		step(`Version ${flags.version}`);
		stampVersion(flags.version);
	}
	const started = Date.now();
	if (!flags["skip-sidecars"]) {
		step("Sidecars (frameever-muxer, frameever-cli)");
		run(gitBash(), [path.join("scripts", "build-desktop-binaries.sh"), target], {
			env: {
				...process.env,
				CARGO_PROFILE_RELEASE_DEBUG: "0",
				CARGO_PROFILE_RELEASE_STRIP: "symbols",
			},
		});
	}

	step("App and NSIS installer");
	const signing = Boolean(process.env.TAURI_SIGNING_PRIVATE_KEY);
	const bundleArgs = ["--bundles", "nsis"];
	if (!signing) {
		console.log("  No TAURI_SIGNING_PRIVATE_KEY: building an unsigned installer (no updater .sig).");
		const unsigned = path.join(gpuiDir, "target", "fe-unsigned.conf.json");
		mkdirSync(path.dirname(unsigned), { recursive: true });
		writeFileSync(unsigned, JSON.stringify({ bundle: { createUpdaterArtifacts: false } }));
		bundleArgs.push("--config", unsigned);
	}
	run(process.execPath, [path.join("scripts", "bundle-gpui.mjs"), target, ...bundleArgs], {
		env: { ...process.env, CI: "false", RUST_TARGET_TRIPLE: target },
	});

	const nsisDir = path.join(gpuiDir, "target", target, "release", "bundle", "nsis");
	const installers = readdirSync(nsisDir).filter((name) => name.endsWith("-setup.exe"));
	if (installers.length === 0) fail(`No installer in ${nsisDir}`);
	const newest = installers
		.map((name) => path.join(nsisDir, name))
		.sort((a, b) => statSync(b).mtimeMs - statSync(a).mtimeMs)[0];
	mkdirSync(distDir, { recursive: true });
	const copied = [newest, `${newest}.sig`].filter((file) => existsSync(file));
	for (const file of copied) copyFileSync(file, path.join(distDir, path.basename(file)));
	const minutes = ((Date.now() - started) / 60000).toFixed(1);
	const size = (statSync(newest).size / 1024 / 1024).toFixed(1);
	console.log(
		`\n\u001b[32m✓ ${path.join(distDir, path.basename(newest))} (${size} MB, ${minutes} min)\u001b[0m`,
	);
	if (copied.length > 1) console.log("  Updater signature copied next to it.");
}

export function changelogSection(text, version) {
	const lines = text.replace(/\r\n/g, "\n").split("\n");
	const heading = (line) => line.startsWith("## ");
	const start = lines.findIndex(
		(line) => heading(line) && line.slice(3).trim().replace(/^\[/, "").startsWith(`${version}]`),
	);
	if (start === -1) return null;
	const rest = lines.slice(start + 1);
	const end = rest.findIndex(heading);
	const body = (end === -1 ? rest : rest.slice(0, end))
		.filter((line) => line.trim() !== "---")
		.join("\n")
		.trim();
	return body || null;
}

export const UNRELEASED = "未发布";

export function stampReleaseDate(text, version, date) {
	return text.replace(
		new RegExp(`^## \\[${version.replace(/\./g, "\\.")}\\] — ${UNRELEASED}[ \\t]*(\\r?)$`, "m"),
		`## [${version}] — ${date}$1`,
	);
}

function localDate() {
	const now = new Date();
	const pad = (value) => String(value).padStart(2, "0");
	return `${now.getFullYear()}-${pad(now.getMonth() + 1)}-${pad(now.getDate())}`;
}

function releaseNotes(version) {
	const notes = changelogSection(readFileSync(path.join(root, "CHANGELOG.md"), "utf8"), version);
	if (!notes) fail(`CHANGELOG.md has no "## [${version}] — <date>" section. Write it first.`);
	return notes;
}

export function parseCargoUpdate(output) {
	const lines = output.split(/\r?\n/).map((line) => line.trim());
	return {
		changes: lines.filter((line) => /^(Updating|Adding|Removing|Downgrading) \S+ v/.test(line)),
		behind: lines.filter((line) => /^Unchanged \S+ v\S+ \(available: v/.test(line)),
	};
}

function deps({ flags }) {
	const workspaces = [
		{ name: "Root workspace (CLI, sidecars, shared crates)", cwd: root },
		{ name: "GPUI app", cwd: gpuiDir },
	];
	for (const workspace of workspaces) {
		step(workspace.name);
		const args = flags.apply ? ["update", "--verbose"] : ["update", "--dry-run", "--verbose"];
		const result = spawnSync("cargo", args, { cwd: workspace.cwd, encoding: "utf8" });
		if (result.status !== 0) fail(`cargo ${args.join(" ")} failed:\n${result.stderr}`);
		const { changes, behind } = parseCargoUpdate(`${result.stdout}\n${result.stderr}`);
		console.log(`  ${changes.length} change(s) within version ranges${flags.apply ? ", applied" : ""}`);
		console.log(`  ${behind.length} held back (newer major, pre-release pin or MSRV); bump these by hand:`);
		for (const line of behind) console.log(`    ${line}`);
	}
	if (flags.apply) {
		console.log("\n  Next: pnpm fe test --all, pnpm fe dev, pnpm fe export hevc, then commit both Cargo.lock files.");
	} else {
		console.log("\n  Apply the in-range changes with: pnpm fe deps --apply");
	}
}

function notes({ positional }) {
	const version = positional[0];
	if (!version || !SEMVER.test(version)) fail("Usage: pnpm fe notes <x.y.z>");
	process.stdout.write(`${releaseNotes(version)}\n`);
}

async function release({ flags, positional }) {
	const version = positional[0];
	if (!version || !SEMVER.test(version)) fail("Usage: pnpm fe release <x.y.z>");
	const tag = `v${version}`;
	step("Working tree");
	const dirty = capture("git", ["status", "--porcelain"]);
	if (dirty) fail(`Commit or stash your changes first:\n${dirty}`);
	if (capture("git", ["tag", "--list", tag])) fail(`Tag ${tag} already exists locally.`);
	const remote = capture("git", ["ls-remote", "--tags", "origin", `refs/tags/${tag}`]);
	if (remote) fail(`Tag ${tag} already exists on origin.`);
	step("Release notes (CHANGELOG.md)");
	console.log(releaseNotes(version).replace(/^/gm, "  "));
	const head = capture("git", ["log", "-1", "--format=%h %s"]);
	const branch = capture("git", ["branch", "--show-current"]);
	console.log(`  ${branch} @ ${head}`);
	if (!flags["skip-tests"]) {
		test({ flags: { all: true }, positional: [] });
	}
	step("Confirm");
	const date = localDate();
	console.log(
		`  This dates the CHANGELOG.md entry ${date}, commits it, and pushes ${branch} and tag ${tag} to origin. The release workflow then builds, signs and publishes FrameEver ${version}.`,
	);
	const prompt = createInterface({ input: process.stdin, output: process.stdout });
	const answer = (await prompt.question(`  Release ${tag}? [y/N] `)).trim().toLowerCase();
	prompt.close();
	if (answer !== "y" && answer !== "yes") {
		console.log("  Nothing changed.");
		return;
	}
	const changelog = path.join(root, "CHANGELOG.md");
	const text = readFileSync(changelog, "utf8");
	const stamped = stampReleaseDate(text, version, date);
	if (stamped !== text) {
		writeFileSync(changelog, stamped);
		run("git", ["add", "CHANGELOG.md"]);
		run("git", ["commit", "-m", `chore: 发布 ${version}`]);
	}
	run("git", ["tag", "-a", tag, "-m", `FrameEver ${version}`]);
	run("git", ["push", "origin", `HEAD:refs/heads/${branch}`]);
	run("git", ["push", "origin", tag]);
	const url = capture("git", ["remote", "get-url", "origin"]) ?? "";
	const slug = url.match(/github\.com[:/](.+?)(\.git)?$/)?.[1];
	console.log(`\n\u001b[32m✓ Pushed ${tag}\u001b[0m`);
	if (slug) console.log(`  Workflow: https://github.com/${slug}/actions`);
}

export function realLogsDir(env = process.env) {
	const local = env.LOCALAPPDATA ?? path.join(env.USERPROFILE ?? "", "AppData", "Local");
	return path.join(local, "io.github.anibullet.frameever", "logs");
}

export function latestLog(dir) {
	if (!existsSync(dir)) return null;
	const files = readdirSync(dir)
		.filter((name) => name.startsWith("frameever.log") && !name.endsWith(".jsonl"))
		.map((name) => path.join(dir, name))
		.sort((a, b) => statSync(b).mtimeMs - statSync(a).mtimeMs);
	return files[0] ?? null;
}

function logs({ flags }) {
	const dir = flags.real ? realLogsDir() : path.join(testdataDir, "logs");
	const file = latestLog(dir);
	if (!file) fail(`No log in ${dir}. Run the app first (pnpm fe dev).`);
	const count = Number(flags.lines ?? 200);
	let lines = readFileSync(file, "utf8").split(/\r?\n/);
	if (typeof flags.grep === "string") {
		const needle = flags.grep.toLowerCase();
		lines = lines.filter((line) => line.toLowerCase().includes(needle));
	}
	console.log(`\u001b[2m${file}\u001b[0m`);
	console.log(lines.slice(-count).join("\n"));
}

function doctor() {
	const checks = [];
	const add = (name, ok, detail, fix) => checks.push({ name, ok, detail, fix });
	const version = (command, args) => capture(command, args)?.split(/\r?\n/)[0] ?? null;

	const node = Number(process.versions.node.split(".")[0]);
	add("Node 20+", node >= 20, process.versions.node, "Install Node 20 or newer");
	const pnpmVersion = version(...pnpm(["--version"]));
	add("pnpm", Boolean(pnpmVersion), pnpmVersion, "npm install -g pnpm@10.5.2");
	add("node_modules", existsSync(path.join(root, "node_modules")), "", "pnpm fe setup --install");
	const toolchains = capture("rustup", ["toolchain", "list"]) ?? "";
	add("Rust stable", toolchains.includes("stable"), "", "rustup toolchain install stable");
	add(
		"Git Bash",
		(() => {
			try {
				return Boolean(gitBash());
			} catch {
				return false;
			}
		})(),
		"",
		"Install Git for Windows",
	);
	const missing = missingNativeDeps();
	add(
		"FFmpeg / ONNX Runtime / DXC",
		missing.length === 0,
		missing.join(", "),
		"pnpm fe setup",
	);
	const cargoConfig = path.join(root, ".cargo", "config.toml");
	const config = existsSync(cargoConfig) ? readFileSync(cargoConfig, "utf8") : "";
	add(
		".cargo/config.toml",
		config.includes("FFMPEG_DIR") && config.includes("LIBCLANG_PATH"),
		"",
		"pnpm fe setup",
	);
	const fonts = missingMisans();
	add("MiSans font files", fonts.length === 0, fonts.join(", "), "pnpm fe setup");
	add(
		"Patched gpui (../zed-cap)",
		existsSync(path.join(root, "..", "zed-cap", "crates", "gpui")),
		"",
		"pnpm fe setup",
	);
	add(
		"Privacy check before push",
		capture("git", ["config", "core.hooksPath"]) === HOOKS_DIR,
		"",
		"pnpm fe setup",
	);
	add(
		"Test recording (for fe export)",
		Boolean(firstRecording(path.join(testdataDir, "recordings"))),
		"",
		"pnpm fe dev, then record something",
	);

	for (const check of checks) {
		const mark = check.ok ? "\u001b[32m✓\u001b[0m" : "\u001b[31m✗\u001b[0m";
		const detail = check.detail ? ` \u001b[2m${check.detail}\u001b[0m` : "";
		console.log(`${mark} ${check.name}${detail}${check.ok ? "" : `  → ${check.fix}`}`);
	}
	const broken = checks.filter((check) => !check.ok && check.name !== "Test recording (for fe export)");
	if (broken.length > 0) process.exit(1);
}

export const HOOKS_DIR = ".githooks";

function readStdin() {
	try {
		return readFileSync(0, "utf8");
	} catch {
		return "";
	}
}

function privacy({ flags }) {
	const mode = flags.hook ? "hook" : flags.staged ? "staged" : flags.all ? "tree" : "outgoing";
	const { scope, findings } = privacyCheck({ mode, stdin: mode === "hook" ? readStdin() : "" });
	step(`Privacy check: ${scope}`);
	if (findings.length === 0) {
		console.log("\u001b[32m✓ No private data found\u001b[0m");
		return;
	}
	for (const finding of findings) console.log(`  ${describeFinding(finding)}`);
	fail(
		`${findings.length} possible leak(s). Remove them (rewrite the commit if it is already committed), or, if a match is public on purpose, add it to scripts/privacy-allow.json.`,
	);
}

function help() {
	console.log("Usage: pnpm fe <command> [options]\n");
	for (const [name, description] of Object.entries(COMMANDS)) {
		console.log(`  ${name.padEnd(8)} ${description}`);
	}
}

const HANDLERS = {
	setup,
	dev,
	check,
	test,
	lint,
	fmt,
	export: exportVideo,
	package: packageInstaller,
	release,
	notes,
	deps,
	doctor,
	logs,
	privacy,
};

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
	const [name, ...rest] = process.argv.slice(2);
	const handler = HANDLERS[name];
	if (!handler) {
		help();
		process.exit(name && name !== "help" ? 1 : 0);
	}
	await handler(parseArgs(rest));
}
