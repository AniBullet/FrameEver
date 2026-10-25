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
			"target/x86_64-pc-windows-msvc/release/cap-gpui.exe",
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
	const directives = hooks
		.split(/\r?\n/)
		.filter((line) => line.trim() && !line.trim().startsWith(";"));
	assert.equal(directives[0], "SetCompressorDictSize 128");
	assert.ok(
		directives.indexOf("SetCompressorDictSize 128") <
			directives.findIndex((line) => line.startsWith("!macro")),
	);
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
	assert.equal(windows["../../target/ffmpeg/bin/*.dll"], "./");
	assert.equal(windows["../../target/native-deps/onnxruntime/lib/*.dll"], "./");
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
