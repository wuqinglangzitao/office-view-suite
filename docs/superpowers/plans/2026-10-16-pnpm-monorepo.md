# pnpm Monorepo Migration Plan

1. Add the root pnpm workspace manifest, scripts, documentation, and ignore rules.
2. Add a consistent `typecheck` script to each viewer package.
3. Move the four viewer directories intact under `apps/`.
4. Remove the obsolete per-application npm lockfiles.
5. Install from the root and create one `pnpm-lock.yaml`.
6. Initialize the root Git repository on the `main` branch.
7. Verify workspace discovery, application type checks, production builds, WASM paths, and Git root detection.

The migration deliberately excludes shared-package extraction, dependency upgrades, a Cargo workspace, and rendering changes.
