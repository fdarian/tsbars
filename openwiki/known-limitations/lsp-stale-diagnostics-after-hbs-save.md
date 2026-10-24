---
type: known-limitation
title: Stale diagnostics after saving an unopened .hbs file (TS 7.1 beta LSP)
description: In editors using the TypeScript 7.1 beta language server, saving a .hbs file that is not open in the server does not refresh diagnostics in importing .ts files. Upstream bug, not a tsbars bug; includes root cause, workarounds, and when to delete this note.
tags: [known-issue, upstream, typescript, lsp, editor]
verified:
  - by: openwiki/0.7.2
    at: 2026-10-24T09:10:05.197Z
sources:
  - id: openwiki-source-5b54a58d1b51cd490b0e7162
    resource: repo://package.json
generated: { by: "claude-code", at: "2026-10-24T09:10:05.197Z" }
---

# Stale diagnostics after saving an unopened .hbs file

**Symptom.** In an editor running the TS 7.1 beta language server (`tsc --lsp`), saving a `.hbs` file that is not open in the language server does not refresh diagnostics in the `.ts` files that import it. The old inferred types stick until the importing file is edited.

**Not a tsbars bug.** The mapper is registered through `package.json#typescript.contentMapper` and returns correct output; the editor just never re-pulls diagnostics. Tracked upstream:

- Issue: [microsoft/TypeScript#64943](https://github.com/microsoft/TypeScript/issues/64943) (open)
- Fix PR: [microsoft/TypeScript#64944](https://github.com/microsoft/TypeScript/pull/64944) (open as of 2026-10-24)

## Root cause (tag `v7.1-beta`)

`Session.DidChangeWatchedFiles` in `tsc/internal/project/session.go` (~L454) uses `PathKey.Extension()` (`TryGetExtensionFromPath`), which returns `""` for non-TS extensions such as `.hbs`. The path is then treated as a possible directory, the content-mapper-extension check in the `else` branch is never reached, and `ScheduleDiagnosticsRefresh()` never runs.

Server state does update: the transform cache is keyed by content hash and the file watchers cover `**/*`. Only the `workspace/diagnostic/refresh` push is missing. `.d.ts` files hit the same bug. It is a regression from [microsoft/TypeScript#64159](https://github.com/microsoft/TypeScript/pull/64159).

## Why the mapper cannot work around it

`openProject`'s `watchedFiles` (which needs `dynamicConfig: true`) is meant for config files, and the content-mapper spec says not to list transformed files there.

## Workarounds

- Attach `.hbs` buffers to the TS language server so edits arrive as `didOpen`/`didChange`. In Neovim, add `handlebars` to the `tsc` server's filetypes.
- Edit or re-open the importing `.ts` file to force diagnostics to be re-pulled.

## Revisit

When #64944 ships in a TypeScript release, delete this note.
