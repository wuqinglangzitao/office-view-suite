# Office View Suite

Office View Suite is a pnpm monorepo containing browser-based viewers for PowerPoint, Word, Excel, and PDF documents.

## Requirements

- Node.js 22
- pnpm 7.33.7
- Rust 1.89.0 and `wasm-pack` when rebuilding WASM packages

## Applications

| Package | Directory | Development command |
| --- | --- | --- |
| `ppt-view` | `apps/ppt-view` | `pnpm dev:ppt` |
| `word-view` | `apps/word-view` | `pnpm dev:word` |
| `excel-view` | `apps/excel-view` | `pnpm dev:excel` |
| `pdf-view` | `apps/pdf-view` | `pnpm dev:pdf` |

## Commands

Install all workspace dependencies from the repository root:

```bash
pnpm install
```

Build or type-check every application:

```bash
pnpm build
pnpm typecheck
```

Rebuild all WASM packages:

```bash
pnpm wasm
```

Targeted `build:*`, `typecheck:*`, and `wasm:*` scripts are available for each application.
