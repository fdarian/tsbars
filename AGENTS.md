# tsbars

Typed Handlebars imports: `import t from './x.hbs'` gets a call signature inferred from the template. Works as a TypeScript 7.1 _content mapper_ (the compiler hands the template to an external process, which returns a virtual `.ts` module plus a source map), with Vite and Bun plugins for the runtime side. Modelled on reylo by Colin McDonnell.

## Stack

- pnpm (package manager), Bun (test runner only: `bun test`), Node 26 (CI)
- TypeScript `7.1.0-beta` (native compiler), tsdown (bundling + `.d.ts`), Biome (lint/format), Changesets
- Rust (edition 2024) for the mapper, built to `wasm32-wasip1`; toolchain pinned in `rust-toolchain.toml`

## Repo map

- `crates/tsbars/`: library. Handlebars parser, context inference, TS emit with source mappings. Rust tests in `tests/`.
- `crates/tsbars-mapper/`: JSON-RPC content mapper server over stdio (LSP-style framing). Thin wrapper around the lib.
- `bin/mapper.js`: WASI host. Runs `wasm/tsbars-mapper.wasm` under `node:wasi`; this is what `package.json#typescript.contentMapper.exec` points tsc at.
- `src/`: runtime plugins (`compile.ts`, `vite.ts`, `bun.ts`) that precompile `.hbs` with Handlebars. Built to `dist/`.
- `tests/`: `runtime.test.ts` plus `e2e.test.ts`, which type-checks `tests/e2e/*` fixtures and `examples/basic` with the real `tsc` and asserts on diagnostics.
- `examples/basic/`: minimal working consumer project.
- `.github/`: `ci.yml` (format auto-commit, then `pnpm run check`), `release.yml` (Changesets), `actions/setup` (Node, pnpm, Bun, Rust).

`dist/`, `wasm/`, `target/` are gitignored build output.

## Commands

- `pnpm install`
- `pnpm run build`: tsdown, then `build:wasm` (cargo build for wasm32-wasip1 into `wasm/`)
- `pnpm run check`: `tsc --noEmit`, Biome lint, clippy (native and wasm), `bun test`, `cargo test`
- `pnpm run format`: Biome + `cargo fmt`; CI auto-commits this on PRs, so run it before pushing
- `pnpm exec changeset`: add a release note; merging the generated "Version Packages" PR publishes

## Gotchas

- TypeScript 7 has no JS compiler API, so tsdown generates declarations through `isolatedDeclarations`. Exported APIs need explicit types.
- Panics abort under `wasm32-wasip1`: `catch_unwind` in the mapper only helps the native build (`cargo test`). Don't rely on it for wasm; return errors instead of panicking.
- tsc resolves the `tsbars` mapper package only from a `node_modules` directory, not by self-reference. The e2e fixtures and `examples/basic` symlink the repo root into their own `node_modules/tsbars`.
- Content mappers are external code: tsc needs `--runExternalCode`, or nothing runs.
- `e2e.test.ts` rebuilds the wasm in `beforeAll`; edits to the Rust crates are picked up by `bun test`.
- Mapper stdout is the protocol channel. `bin/mapper.js` never touches `process.stdin` so fd 0 stays a blocking pipe for WASI; don't log to stdout.
- The first npm publish must be manual. OIDC trusted publishing (configured on npmjs.com for this repo and `release.yml`) only works for later releases.
