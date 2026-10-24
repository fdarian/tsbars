---
type: quickstart
title: OpenWiki quickstart
description: Task-routing map for the tsbars wiki, which holds only knowledge the code cannot say, such as upstream limitations and their workarounds.
tags: [quickstart, tsbars]
verified:
  - by: openwiki/0.7.2
    at: 2026-10-24T09:10:05.197Z
sources:
  - id: openwiki-source-5b54a58d1b51cd490b0e7162
    resource: repo://package.json
generated: { by: "claude-code", at: "2026-10-24T09:10:05.197Z" }
---

# OpenWiki quickstart

This wiki holds only what the code and `AGENTS.md` cannot say. For the repo map, commands, and build gotchas, read `AGENTS.md`. The mapper is wired into TypeScript via `package.json#typescript.contentMapper`.

| If you are working on... | Read |
| --- | --- |
| Editor diagnostics not updating after saving a `.hbs` file | [Stale diagnostics after saving an unopened .hbs file](known-limitations/lsp-stale-diagnostics-after-hbs-save.md) |
