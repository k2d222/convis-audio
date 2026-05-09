This module is written in Rust. We generate Node (N-API) bindings using [napi-rs](https://napi.rs).

It comprises a main module `@convis/audio` which contains the TypeScript code and glue code to load the native module (`dist/index.js`). It optionally depends on three modules `@convis/audio-<target>` which contain just the pre-compiled `.node` file. This is a common approach in Node.

## Local development (`pnpm dev` in current folder)

napi-rs compiles Rust and generates 3 files in `dist/`: `audio.<local target>.node`, `index.js`, and `index.d.ts`.
* The `.node` file is gitignored. It contains the native Rust module.
* `index.js` is the glue code to import the `.node` file. It finds it either in the same directory, or in one of the precompiled modules.
* `index.d.ts` contains automatically-generated typings for the exported Rust methods and structs.

## Releases

* Trigger the GitHub workflow here: <https://github.com/k2d222/convis-audio/actions/workflows/build.yml>
* When completed: `pnpm script:download-artifacts <workflow run ID>`
* Check that the `.node` files were copied to `platforms/<target>/audio-<target>.node`
* Bump package versions and commit: `pnpm script:version patch` (for a patch version bump)
* `pnpm publish -r --access public`
