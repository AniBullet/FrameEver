import assert from "node:assert/strict";
import test from "node:test";
import { shouldBuildGpui, shouldBundleGpui } from "./run-gpui-build.mjs";

test("GPUI release builds run on every supported desktop platform", () => {
	assert.equal(shouldBuildGpui("darwin", {}, "release"), true);
	assert.equal(shouldBuildGpui("win32", {}, "release"), true);
	assert.equal(shouldBuildGpui("linux", {}, "release"), false);
	assert.equal(shouldBuildGpui("freebsd", {}, "release"), false);
});

test("GPUI development builds honor their platform and opt-out guards", () => {
	assert.equal(shouldBuildGpui("darwin", {}, "debug"), true);
	assert.equal(
		shouldBuildGpui("darwin", { FRAMEEVER_GPUI_DEV: "0" }, "debug"),
		false,
	);
	assert.equal(shouldBuildGpui("darwin", {}, "debug", false), false);
	assert.equal(shouldBuildGpui("win32", {}, "debug"), false);
});

test("the development opt-out never suppresses supported release builds", () => {
	for (const platform of ["darwin", "win32"]) {
		assert.equal(
			shouldBuildGpui(platform, { FRAMEEVER_GPUI_DEV: "0" }, "release"),
			true,
		);
	}
});

test("macOS development only bundles an enabled and available GPUI sidecar", () => {
	assert.equal(shouldBundleGpui("darwin", {}, "debug", true), true);
	assert.equal(shouldBundleGpui("darwin", {}, "debug", false), false);
	assert.equal(shouldBundleGpui("darwin", {}, "debug", true, false), false);
	assert.equal(
		shouldBundleGpui("darwin", { FRAMEEVER_GPUI_DEV: "0" }, "debug", true),
		false,
	);
});

test("release bundling is mandatory on every supported desktop platform", () => {
	for (const platform of ["darwin", "win32"]) {
		assert.equal(
			shouldBundleGpui(platform, { FRAMEEVER_GPUI_DEV: "0" }, "release", false),
			true,
		);
	}
	assert.equal(shouldBundleGpui("freebsd", {}, "release", true), false);
});
