import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";
import path from "node:path";
import test from "node:test";
import {
	bundleArguments,
	bundleEnvironment,
	gpuiBinaryPath,
	gpuiDirectory,
	gpuiPlatformConfig,
	mainBinaryName,
	parseOptions,
	targetPlatform,
} from "./bundle-gpui.mjs";

const readJson = (relative) =>
	JSON.parse(readFileSync(new URL(relative, import.meta.url), "utf8"));

const gpuiConfig = readJson("../apps/desktop-gpui/tauri.conf.json");
const gpuiPackage = readJson("../apps/desktop-gpui/package.json");
const repoRoot = path.dirname(path.dirname(gpuiDirectory));

test("targets map to their packaging platform and main binary", () => {
	assert.equal(targetPlatform("aarch64-apple-darwin"), "darwin");
	assert.equal(targetPlatform("x86_64-pc-windows-msvc"), "win32");
	assert.equal(targetPlatform("x86_64-unknown-linux-gnu"), null);
	assert.equal(targetPlatform("wasm32-unknown-unknown"), null);
	assert.equal(mainBinaryName("x86_64-pc-windows-msvc"), "FrameEver.exe");
	assert.equal(mainBinaryName("aarch64-apple-darwin"), "FrameEver");
	assert.equal(
		gpuiBinaryPath("x86_64-pc-windows-msvc"),
		path.join(
			gpuiDirectory,
			"target/x86_64-pc-windows-msvc/release/frameever-gpui.exe",
		),
	);
});

test("installers are signed for the app's own GitHub Releases updater", () => {
	assert.equal(gpuiConfig.plugins.updater, undefined);
	assert.equal(gpuiConfig.bundle.createUpdaterArtifacts, true);
	const installer = readFileSync(
		new URL("../apps/desktop-gpui/src/installer.rs", import.meta.url),
		"utf8",
	);
	assert.match(installer, /option_env!\("FRAMEEVER_UPDATER_PUBKEY"\)/);
	assert.doesNotMatch(installer, /crabnebula/);
});

test("the GPUI app ships as FrameEver, apart from official Cap", () => {
	const brand = readFileSync(
		new URL("../crates/utils/src/brand.rs", import.meta.url),
		"utf8",
	);
	const constant = (name) =>
		new RegExp(`pub const ${name}: &str = "([^"]+)";`).exec(brand)?.[1];
	assert.equal(gpuiConfig.productName, "FrameEver");
	assert.equal(gpuiConfig.mainBinaryName, "FrameEver");
	assert.equal(gpuiConfig.identifier, constant("APP_ID"));
	assert.deepEqual(gpuiConfig.plugins["deep-link"].desktop.schemes, [
		constant("URL_SCHEME"),
	]);
	assert.notEqual(gpuiConfig.identifier, "so.cap.desktop");
});

test("the bundle registers the project extension, on macOS as a package", () => {
	const brand = readFileSync(
		path.join(repoRoot, "crates", "utils", "src", "brand.rs"),
		"utf8",
	);
	const extension = /pub const PROJECT_EXTENSION: &str = "([^"]+)";/.exec(
		brand,
	)?.[1];
	assert.ok(extension);
	assert.deepEqual(
		gpuiConfig.bundle.fileAssociations.map((association) => association.ext),
		[[extension]],
	);
	const plist = readFileSync(path.join(gpuiDirectory, "Info.plist"), "utf8");
	assert.match(
		plist,
		new RegExp(
			`<key>CFBundleTypeExtensions</key>\\s*<array>\\s*<string>${extension}</string>\\s*</array>`,
		),
	);
	assert.match(plist, /<key>LSTypeIsPackage<\/key>\s*<true\/>/);
});

test("every committed file the GPUI bundle references exists", () => {
	const files = [
		...gpuiConfig.bundle.icon,
		gpuiConfig.bundle.macOS.entitlements,
		gpuiConfig.bundle.macOS.dmg.background,
		gpuiConfig.bundle.windows.nsis.headerImage,
		gpuiConfig.bundle.windows.nsis.sidebarImage,
		gpuiConfig.bundle.windows.nsis.installerIcon,
		gpuiConfig.bundle.windows.nsis.installerHooks,
		"Info.plist",
	];
	for (const file of files) {
		assert.ok(existsSync(path.join(gpuiDirectory, file)), file);
	}
	for (const source of Object.keys(gpuiConfig.bundle.resources)) {
		assert.ok(
			existsSync(path.join(gpuiDirectory, path.dirname(source))),
			source,
		);
	}
	for (const binary of gpuiConfig.bundle.externalBin) {
		assert.ok(
			existsSync(path.join(gpuiDirectory, path.dirname(binary))),
			binary,
		);
	}
});

test("the bundle reads only the GPUI app and the shared build output", () => {
	const sources = [
		...gpuiConfig.bundle.icon,
		...gpuiConfig.bundle.externalBin,
		...Object.keys(gpuiConfig.bundle.resources),
		gpuiConfig.bundle.macOS.entitlements,
		gpuiConfig.bundle.macOS.dmg.background,
		...Object.values(gpuiConfig.bundle.windows.nsis).filter(
			(value) => typeof value === "string" && /[/.]/.test(value),
		),
	];
	const allowed = [gpuiDirectory, path.join(repoRoot, "target")];
	for (const source of sources) {
		const resolved = path.resolve(gpuiDirectory, source);
		assert.ok(
			allowed.some((root) => resolved.startsWith(root + path.sep)),
			source,
		);
	}
});

test("the Tauri CLI comes from the GPUI app's own package", () => {
	assert.equal(gpuiPackage.scripts.tauri, "tauri");
	assert.ok(gpuiPackage.devDependencies["@tauri-apps/cli"]);
	const workspace = readFileSync(
		path.join(repoRoot, "pnpm-workspace.yaml"),
		"utf8",
	);
	assert.match(workspace, /^ {2}- apps\/desktop-gpui$/m);
});

test("the GPUI bundle ships one CLI sidecar instead of two identical copies", () => {
	const names = gpuiConfig.bundle.externalBin.map((binary) =>
		path.basename(binary),
	);
	assert.deepEqual(names, ["frameever-muxer", "frameever-cli"]);
	assert.equal(gpuiConfig.bundle.windows.webviewInstallMode.type, "skip");
	assert.ok(
		!Object.values(gpuiConfig.bundle.resources).some((destination) =>
			destination.includes("rive"),
		),
	);
});

test("the Windows installer compresses with a window large enough to span both binaries", () => {
	assert.equal(gpuiConfig.bundle.windows.nsis.compression, "lzma");
	const hooks = readFileSync(
		path.join(gpuiDirectory, gpuiConfig.bundle.windows.nsis.installerHooks),
		"utf8",
	);
	assert.ok(hooks.startsWith("\uFEFF"));
	const directives = hooks
		.replace(/^\uFEFF/, "")
		.split(/\r?\n/)
		.filter((line) => line.trim() && !line.trim().startsWith(";"));
	assert.equal(directives[0], "SetCompressorDictSize 128");
	assert.ok(
		directives.indexOf("SetCompressorDictSize 128") <
			directives.findIndex((line) => line.startsWith("!macro")),
	);
});

test("the Windows installer ends idle MCP servers but never a CLI recording", () => {
	const hooks = readFileSync(
		path.join(gpuiDirectory, gpuiConfig.bundle.windows.nsis.installerHooks),
		"utf8",
	);
	const macro = (name) =>
		hooks.match(
			new RegExp(`!macro ${name}\\r?\\n([\\s\\S]*?)!macroend`),
		)?.[1] ?? "";
	for (const hook of ["NSIS_HOOK_PREINSTALL", "NSIS_HOOK_PREUNINSTALL"]) {
		assert.match(macro(hook), /!insertmacro EndFrameEverMcpServers/, hook);
	}
	assert.doesNotMatch(hooks, /CheckIfAppIsRunning "frameever-cli\.exe"/);
	const end = macro("EndFrameEverMcpServers");
	assert.match(end, /\$\$_\.ExecutablePath -eq \$\$cli/);
	assert.match(end, /-match '\\smcp\\s\+serve\(\\s\|\$\$\)'/);
	assert.match(
		end,
		/Join-Path \$\$env:FRAMEEVER_INSTALL_DIR 'frameever-cli\.exe'/,
	);
	const lines = end
		.split(/\r?\n/)
		.map((line) => line.trim())
		.filter(Boolean);
	assert.equal(lines[0], "Push $0");
	assert.deepEqual(lines.slice(-2), ["Pop $0", "Pop $0"]);
	assert.equal(lines.filter((line) => line.startsWith("Push ")).length, 1);
});

test("platform resources point at the shared native dependencies", () => {
	assert.deepEqual(gpuiPlatformConfig("darwin"), {
		bundle: {
			resources: {
				"../../target/native-deps/onnxruntime/lib/libonnxruntime.dylib":
					"onnxruntime/lib/libonnxruntime.dylib",
			},
		},
	});
	const windows = gpuiPlatformConfig("win32").bundle.resources;
	assert.equal(
		windows["target/installer/frameever-setup.exe"],
		"frameever-setup.exe",
	);
	assert.equal(windows["../../target/ffmpeg/bin/*.dll"], "./");
	assert.equal(windows["../../target/native-deps/onnxruntime/lib/*.dll"], "./");
	assert.equal(windows["../../target/native-deps/vcruntime/*.dll"], "./");
});

test("the Windows installer copies nothing while a CLI from this install runs", () => {
	const hooks = readFileSync(
		path.join(gpuiDirectory, gpuiConfig.bundle.windows.nsis.installerHooks),
		"utf8",
	);
	const macro = (name) =>
		hooks.match(
			new RegExp(`!macro ${name}(?: [A-Z_]+)?\\r?\\n([\\s\\S]*?)!macroend`),
		)?.[1] ?? "";
	for (const hook of ["NSIS_HOOK_PREINSTALL", "NSIS_HOOK_PREUNINSTALL"]) {
		const body = macro(hook);
		assert.match(body, /!insertmacro WaitForFrameEverCommandLineTools "/, hook);
		assert.ok(
			body.indexOf("EndFrameEverMcpServers") <
				body.indexOf("WaitForFrameEverCommandLineTools"),
			hook,
		);
	}
	const wait = macro("WaitForFrameEverCommandLineTools");
	assert.match(
		wait,
		/\$\$_\.Name -eq 'frameever-cli\.exe' -or \$\$_\.Name -eq 'frameever-muxer\.exe'/,
	);
	assert.match(wait, /GetDirectoryName\(\$\$_\.ExecutablePath\) -eq \$\$dir/);
	assert.match(wait, /exit 2 \} catch \{ exit 0 \}/);
	assert.match(wait, /\$\{If\} \$0 == 2/);
	const probe = wait.indexOf('FindProcessCurrentUser "frameever-cli.exe"');
	assert.ok(probe >= 0 && probe < wait.indexOf("nsExec::ExecToStack"));
	assert.match(wait, /FindProcessCurrentUser "frameever-muxer\.exe"/);
	assert.match(
		wait,
		/MB_RETRYCANCEL[^\n]*\/SD IDCANCEL IDRETRY \$\{FrameEverToolsCheck\}/,
	);
	assert.match(wait, /Abort "\$\{ABORT_TEXT\}"/);
	for (const name of [
		"FrameEverToolsRunning",
		"FrameEverNotUpdated",
		"FrameEverNotUninstalled",
	]) {
		for (const language of ["1033", "2052"]) {
			assert.match(
				hooks,
				new RegExp(`^LangString ${name} ${language} "`, "m"),
				`${name} ${language}`,
			);
		}
	}
	const lines = wait
		.split(/\r?\n/)
		.map((line) => line.trim())
		.filter(Boolean);
	assert.deepEqual(lines.slice(1, 3), ["Push $0", "Push $1"]);
	assert.deepEqual(lines.slice(-3), [
		"Pop $1",
		"Pop $0",
		"!undef FrameEverToolsCheck",
	]);
});

test("tauri bundle runs against the GPUI crate with its pinned toolchain", () => {
	assert.deepEqual(bundleArguments("x86_64-pc-windows-msvc", ["--verbose"]), [
		"tauri",
		"bundle",
		"--target",
		"x86_64-pc-windows-msvc",
		"--verbose",
	]);
	const environment = bundleEnvironment({ PATH: "/bin" });
	assert.equal(environment.PATH, "/bin");
	assert.equal(environment.TAURI_APP_PATH, gpuiDirectory);
	assert.match(environment.RUSTUP_TOOLCHAIN, /^\d+\.\d+\.\d+$/);
});

test("signing builds can stop before bundling and rebundle without rebuilding", () => {
	assert.deepEqual(parseOptions(["--no-bundle"]), {
		skipBuild: false,
		noBundle: true,
		bundleArgs: [],
	});
	assert.deepEqual(parseOptions(["--skip-build", "--bundles", "nsis"]), {
		skipBuild: true,
		noBundle: false,
		bundleArgs: ["--bundles", "nsis"],
	});
});
