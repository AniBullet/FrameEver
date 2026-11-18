const PACKAGE_SHA256 = {
	"1.24.2": "782564d3d68e87269ea1812cc94710ad06fe014faf093f01a6f89d0bd58719d3",
	"1.23.2": "25fe172f7fcdf34f2b5b02c8b997f6b88abc282956849d4af4335af23d0c4e4a",
};

const TARGETS = {
	win32: {
		x86_64: {
			version: "1.24.2",
			runtime: "win-x64",
			files: ["onnxruntime.dll", "onnxruntime_providers_shared.dll"],
		},
		aarch64: {
			version: "1.24.2",
			runtime: "win-arm64",
			files: ["onnxruntime.dll", "onnxruntime_providers_shared.dll"],
		},
	},
	darwin: {
		aarch64: {
			version: "1.24.2",
			runtime: "osx-arm64",
			files: ["libonnxruntime.dylib"],
		},
		x86_64: {
			version: "1.23.2",
			runtime: "osx-x64",
			files: ["libonnxruntime.dylib"],
		},
	},
};

export function onnxRuntimeAsset(platform, arch) {
	const target = TARGETS[platform]?.[arch];
	if (!target)
		throw new Error(`ONNX Runtime has no build for ${platform} ${arch}`);

	const { version, runtime, files } = target;
	return {
		version,
		name: `microsoft.ml.onnxruntime.${version}.zip`,
		url: `https://api.nuget.org/v3-flatcontainer/microsoft.ml.onnxruntime/${version}/microsoft.ml.onnxruntime.${version}.nupkg`,
		sha256: PACKAGE_SHA256[version],
		libraries: files.map((file) => `runtimes/${runtime}/native/${file}`),
	};
}
