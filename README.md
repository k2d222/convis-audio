This module is written in Rust. We generate Node (N-API) bindings using [napi-rs](https://napi.rs).

It comprises a main module `@convis/audio` which contains the TypeScript code and glue code to load the native module (`dist/index.js`). It optionally depends on three modules `@convis/audio-<target>` which contain just the pre-compiled `.node` file. This is a common approach in Node.

## Local development (`pnpm dev` in current folder)

napi-rs compiles Rust and generates 3 files in `dist/`: `audio.<local target>.node`, `index.js`, and `index.d.ts`.
* The `.node` file is gitignored. It contains the native Rust module.
* `index.js` is the glue code to import the `.node` file. It finds it either in the same directory, or in one of the precompiled modules.
* `index.d.ts` contains automatically-generated typings for the exported Rust methods and structs.

## Cross-compilation (`pnpm build` in current folder)

This script regenerates the precompiled modules in `platforms/`.

Install Rust and the target toolchains:
- `rustup target add x86_64-unknwown-linux-gnu`
- `rustup target add x86_64-pc-windows-msvc`
- `rustup target add aarch64-apple-darwin`

Then run `pnpm build`. It will compile all targets and move the `.node` file in the right `plaforms/` module.

## macOS

Currently cross-compilation to macOS does not seem to work. It seems to require Xcode.
