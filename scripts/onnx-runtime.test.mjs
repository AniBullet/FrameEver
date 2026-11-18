import assert from "node:assert/strict";
import test from "node:test";
import { onnxRuntimeAsset } from "./onnx-runtime.mjs";

const targets = [
	["win32", "x86_64"],
	["win32", "aarch64"],
	["darwin", "aarch64"],
	["darwin", "x86_64"],
];

test("every supported target downloads a pinned package from nuget.org", () => {
	for (const [platform, arch] of targets) {
		const asset = onnxRuntimeAsset(platform, arch);
		const url = new URL(asset.url);
		assert.equal(url.protocol, "https:");
		assert.equal(url.hostname, "api.nuget.org");
		assert.ok(url.pathname.includes(`/${asset.version}/`));
		assert.match(asset.sha256, /^[0-9a-f]{64}$/);
		assert.ok(asset.name.includes(asset.version));
	}
});

test("Windows takes the runtime and its provider library for its architecture", () => {
	assert.deepEqual(onnxRuntimeAsset("win32", "x86_64").libraries, [
		"runtimes/win-x64/native/onnxruntime.dll",
		"runtimes/win-x64/native/onnxruntime_providers_shared.dll",
	]);
	assert.deepEqual(onnxRuntimeAsset("win32", "aarch64").libraries, [
		"runtimes/win-arm64/native/onnxruntime.dll",
		"runtimes/win-arm64/native/onnxruntime_providers_shared.dll",
	]);
});

test("macOS takes the dylib for its architecture", () => {
	assert.deepEqual(onnxRuntimeAsset("darwin", "aarch64").libraries, [
		"runtimes/osx-arm64/native/libonnxruntime.dylib",
	]);
	assert.deepEqual(onnxRuntimeAsset("darwin", "x86_64").libraries, [
		"runtimes/osx-x64/native/libonnxruntime.dylib",
	]);
});

test("Intel macOS stays on the last release that has its dylib", () => {
	assert.equal(onnxRuntimeAsset("darwin", "aarch64").version, "1.24.2");
	assert.equal(onnxRuntimeAsset("darwin", "x86_64").version, "1.23.2");
});

test("an unsupported target is refused by name", () => {
	assert.throws(() => onnxRuntimeAsset("linux", "x86_64"), /linux x86_64/);
	assert.throws(() => onnxRuntimeAsset("win32", "riscv64"), /win32 riscv64/);
});
