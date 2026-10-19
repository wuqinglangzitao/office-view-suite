# Repository Guidelines

## Project Structure & Module Organization

This pnpm workspace contains browser-based Word, Excel, PowerPoint, and PDF viewers.

- `packages/viewer/`: public `createOfficeViewer()` facade; `viewer-core/`: shared contracts and lifecycle; `viewer-{word,excel,ppt,pdf}/`: renderers, Workers, format adapters, and Rust crates under `rs/`.
- `third_party/pdfium/`: upstream prebuilt PDFium, provenance, and license materials.
- `apps/*-view/`: demo UI and sample documents under `public/samples/`; SDK builds must not depend on demo-owned engine sources or assets.
- `fixtures/vanilla-vite/`: consumer fixture using built packages, without source aliases.
- `tests/`: SDK regression tests; `scripts/`: builds, packaging, and browser verification.
- `docs/sdk.md`: integration and deployment contract; `output/playwright/`: ignored verification artifacts.

Keep SDK logic in packages and file-selection UI in apps. Preserve native DOM integration without iframe.

## Build, Test, and Development Commands

Use Node.js 22.15+, pnpm 7.33.7, Rust 1.89.0, and `wasm-pack`. Run commands from the repository root:

- `pnpm install`: install workspace dependencies.
- `pnpm wasm`: build WASM with locked dependencies and record source/output fingerprints on first checkout or after Rust/vendor/lockfile/build-script changes. SDK builds reject missing or stale outputs.
- `pnpm build`: build SDK packages, demos, and fixture in dependency order.
- `pnpm build:sdk`: refresh package outputs after SDK edits.
- `pnpm dev:sdk` / `pnpm dev:ppt`: build SDK and start the fixture or PPT demo; equivalent Word, Excel, and PDF commands exist.
- `pnpm typecheck` / `pnpm test`: run TypeScript checks and Node regression tests.
- `pnpm test:rust`: run native Word, Excel, and PPT crate tests; PDF glue is verified in the browser.
- `pnpm test:packages`: install packed tarballs into an independent temporary consumer and verify its build.

## Coding Style & Naming Conventions

Follow existing TypeScript ESM style: two-space indentation, single quotes, semicolons, and explicit type imports. Use camelCase for functions/variables, PascalCase for types, and kebab-case package names. TypeScript strictness and unused-symbol checks are enabled; no standalone formatter or linter is configured. Avoid unrelated formatting changes or vendored-source edits.

## Testing Guidelines

Use `node:test` and `node:assert/strict`; name tests `tests/sdk-*.test.mjs` or `tests/sdk-lifecycle-*.mjs`. Build before testing because several tests import `dist`.

Cover cancellation, replacement, disposal, asset URLs, and buffer ownership when affected. No numerical coverage threshold is configured. For rendering changes, run `pnpm test:packages`, then `pnpm test:browser` and `pnpm test:visual` with `PLAYWRIGHT_MODULE` pointing to an installed Playwright module. Include screenshots and inspect differences against the pre-SDK baseline.

## Commit & Pull Request Guidelines

Follow the baseline commit's Lore convention: an intent-focused subject explaining why, with relevant `Constraint:`, `Rejected:`, `Confidence:`, `Scope-risk:`, `Directive:`, `Tested:`, and `Not-tested:` trailers.

PRs should describe the problem, resulting behavior, affected formats, verification commands, and limitations; link related issues and attach screenshots for rendering changes. Exclude generated assets, `dist/`, `target/`, and `node_modules/`. Preserve bundled dependency notices when changing packaging.
