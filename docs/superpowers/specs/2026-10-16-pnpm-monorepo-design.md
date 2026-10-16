# Office View Suite pnpm Monorepo Design

## Goal

Convert the existing four standalone viewers into one pnpm workspace while preserving each application's current behavior, build pipeline, Rust crate, vendored dependencies, and generated WASM layout.

## Scope

The workspace will contain these applications:

- `ppt-view`
- `word-view`
- `excel-view`
- `pdf-view`

This migration does not extract shared packages, change viewer behavior, combine Rust crates, or rewrite application code.

## Target Layout

```text
office-view-suite/
|-- apps/
|   |-- ppt-view/
|   |-- word-view/
|   |-- excel-view/
|   `-- pdf-view/
|-- docs/
|-- package.json
|-- pnpm-workspace.yaml
|-- pnpm-lock.yaml
|-- .gitignore
`-- README.md
```

Each application moves as a complete directory. Its internal layout remains unchanged, including `src/`, `public/`, `rs/`, vendored Rust sources, generated WASM packages, and application-level configuration.

## Package Management

The repository will use pnpm workspaces with `apps/*` as the only package pattern. Generated package manifests below application directories, such as `src/wasm/package.json`, are not workspace packages.

The root `package.json` will:

- be private;
- declare the installed pnpm version through `packageManager`;
- expose per-application development commands;
- expose repository-wide build, typecheck, and WASM commands;
- avoid adding dependencies that are not already used by the applications.

The four npm lockfiles will be replaced by one root `pnpm-lock.yaml`. Application `package.json` files remain independently versioned and retain their existing dependencies and scripts.

## Commands

The root scripts will provide:

- `dev:ppt`, `dev:word`, `dev:excel`, and `dev:pdf` for targeted Vite development servers;
- `build` to build all workspace applications;
- `typecheck` to run TypeScript checking for all applications;
- `wasm` to rebuild all application WASM outputs;
- targeted `build:*`, `typecheck:*`, and `wasm:*` commands for diagnosis and incremental work.

Repository-wide scripts will run applications sequentially where deterministic output is more useful than parallel speed. Development servers remain targeted because each is a long-running process.

## Rust And WASM Boundary

Each application keeps its own Rust crate and `Cargo.lock`. A root Cargo workspace will not be introduced in this migration because the crates have distinct vendored dependencies and independent WASM targets.

Existing `wasm-pack` commands remain inside the application packages. Moving an entire application under `apps/` preserves the relative relationship between its `rs/` crate and generated output directory.

## Git Repository

The root will be initialized as the single Git repository. No nested Git repositories currently exist.

The root `.gitignore` will cover dependency directories, Vite output, Rust build output, local editor files, logs, and common OS metadata. Existing source and generated artifacts that are already part of the applications will not be deleted solely because of the migration.

## Migration Sequence

1. Create the root workspace configuration, documentation, and ignore rules.
2. Move the four complete application directories into `apps/`.
3. Remove the four npm lockfiles.
4. Install dependencies from the root to generate one pnpm lockfile.
5. Initialize the root Git repository.
6. Run targeted type checks and builds for all four applications.
7. Verify root scripts and the expected workspace package list.

## Verification

The migration is complete when:

- `pnpm list --depth -1 -r` reports exactly the root plus the four viewer applications;
- all four application type checks pass;
- all four Vite production builds pass;
- existing application-level WASM commands resolve their Rust crate and output paths correctly;
- no application npm lockfiles remain;
- a single root `pnpm-lock.yaml` exists;
- Git recognizes the root as the repository top level.

Rebuilding every WASM binary is optional if it would only regenerate already-present artifacts; at minimum, each WASM command and path will be checked, and any required rebuild will use the repository's established Rust toolchain.

## Risks And Controls

- Moving directories can break hard-coded paths. A repository-wide path search and all application builds will validate the move.
- pnpm 7 has stricter dependency isolation than npm. Each application currently declares its own direct build dependencies, so no undeclared cross-package dependency is expected.
- Long-running Vite servers may still point at pre-migration directories. Existing processes will not be terminated automatically; new root commands will start from the migrated locations.
- Generated and vendored trees contain their own manifests. Restricting the workspace pattern to `apps/*` prevents accidental enrollment.

## Deferred Work

The following improvements are intentionally excluded:

- shared TypeScript or UI packages;
- shared Vite configuration;
- dependency upgrades;
- a root Cargo workspace;
- CI/CD configuration;
- behavioral changes to document rendering.
