/** Load the native module for the current OS / architecture combo.
 * Modified from the auto-generated file by napi-rs.
 */

let nativeBinding = null;
const loadErrors = [];

function requireNative() {
	try {
		// the locally-build module overrides all others. (generated in dev mode)
		const module = require("./audio.node");
		console.log("!! Running @convis/audio-native in dev mode.");
		return module;
	} catch (e) {
		console.error(e);
		loadErrors.push(e);
	}
	if (process.env.NAPI_RS_NATIVE_LIBRARY_PATH) {
		try {
			return require(process.env.NAPI_RS_NATIVE_LIBRARY_PATH);
		} catch (err) {
			loadErrors.push(err);
		}
	} else if (process.platform === "win32") {
		if (process.arch === "x64") {
			try {
				return require("@convis/audio-x86_64-pc-windows-msvc");
			} catch (e) {
				loadErrors.push(e);
			}
		} else if (process.arch === "arm64") {
			try {
				return require("@convis/audio-aarch64-pc-windows-msvc");
			} catch (e) {
				loadErrors.push(e);
			}
		} else {
			loadErrors.push(
				new Error(`Unsupported architecture on Windows: ${process.arch}`),
			);
		}
	} else if (process.platform === "darwin") {
		if (process.arch === "x64") {
			try {
				return require("@convis/audio-x86_64-apple-darwin");
			} catch (e) {
				loadErrors.push(e);
			}
		} else if (process.arch === "arm64") {
			try {
				return require("@convis/audio-aarch64-apple-darwin");
			} catch (e) {
				loadErrors.push(e);
			}
		} else {
			loadErrors.push(
				new Error(`Unsupported architecture on macOS: ${process.arch}`),
			);
		}
	} else if (process.platform === "linux") {
		if (process.arch === "x64") {
			try {
				return require("@convis/audio-x86_64-unknown-linux-gnu");
			} catch (e) {
				loadErrors.push(e);
			}
		} else if (process.arch === "arm64") {
			try {
				return require("@convis/audio-aarch64-unknown-linux-gnu");
			} catch (e) {
				loadErrors.push(e);
			}
		} else {
			loadErrors.push(
				new Error(`Unsupported architecture on Linux: ${process.arch}`),
			);
		}
	} else {
		loadErrors.push(
			new Error(
				`Unsupported OS: ${process.platform}, architecture: ${process.arch}`,
			),
		);
	}
}

nativeBinding = requireNative();

if (!nativeBinding) {
	if (loadErrors.length > 0) {
		throw new Error(
			`Cannot find native binding. ` +
				`npm has a bug related to optional dependencies (https://github.com/npm/cli/issues/4828). ` +
				"Please try `npm i` again after removing both package-lock.json and node_modules directory.",
			{
				cause: loadErrors.reduce((err, cur) => {
					cur.cause = err;
					return cur;
				}),
			},
		);
	}
	throw new Error(`Failed to load native binding`);
}

module.exports = nativeBinding;
module.exports.AudioHardware = nativeBinding.AudioHardware;
