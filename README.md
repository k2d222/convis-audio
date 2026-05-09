This module is written in Rust. We generate Node (N-API) bindings using [napi-rs](https://napi.rs).

It comprises a main module `@convis/audio` which contains the TypeScript code and glue code to load the native module (`out/bindings.js`). It optionally depends on three modules `@convis/audio-<target>` which contain just the pre-compiled `.node` file. This is a common approach in Node.

## Local development (`pnpm dev` in current folder)

First, napi-rs compiles Rust and generates 3 files in `out/`: `audio.<local target>.node`, `bindings.js`, and `bindings.d.ts`.
* The `.node` file is gitignored. It contains the native Rust module.
* `bindings.js` is the glue code to import the `.node` file. It finds it either in the same directory, or in one of the precompiled modules.
* `bindings.d.ts` contains automatically-generated typings for the exported Rust methods and structs.

Second, `tsdown` generates `dist/` containing the package entrypoint, and a copy of all `.node` files that it could resolve from `bindings.js`.

When cloning the repo, the `.node` file in `out/` does not exist yet, but the pre-compiled modules in `platforms/` do exist.
Run `pnpm tsdown` to compile just the TypeScript (not the Rust module), and it should work fine.

## Cross-compilation (`pnpm build` in current folder)

This script regenerates the precompiled modules in `platforms/`. 

Install Rust and the target toolchains:
- `rustup target add x86_64-unknwown-linux-gnu`
- `rustup target add x86_64-pc-windows-msvc`
- `rustup target add aarch64-apple-darwin`

Then run `pnpm build`. It will compile all targets and move the `.node` file in the right `plaforms/` module. There should not be any `.node` file left in `out/`, otherwise tsdown will package it.

## macOS

Currently cross-compilation to macOS does not seem to work. It seems to require Xcode.
