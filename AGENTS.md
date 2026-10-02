# BaoBoard

A sticker keyboard for macOS and Windows, modeled on WeChat stickers. A global shortcut opens a popup grid, and picking a sticker pastes it into the app that had focus. Tauri v2 app: Rust backend in `src-tauri/`, vanilla TypeScript UI in `src/`, with no UI framework on purpose.

## Verify

`mise run check` is the gate, and CI runs exactly that on macOS and Windows. In a fresh checkout, run `npm ci` first.

Code behind `#[cfg(windows)]` doesn't compile on a Mac, so CI is its only check. Keep platform modules small, and in the PR's test plan, name any platform path you couldn't run.

## Workflow

- Branch from `main` as `mxbch-<kebab-target>`. Each change gets its own PR. Main is protected, PRs are squash-merged once CI is green, and the branch is deleted on merge.
- PR bodies follow `.github/pull_request_template.md`.
- A PR with a single commit uses a Conventional Commit message, so it fills in the PR title. Otherwise, write plain, short commit messages.
- Ideas we aren't building yet are issues labeled `discovery`. Check there before starting a feature.

## Gotchas

- Tauri is pinned to v2, and v3 is still in alpha. Training data mixes v1 and v2 APIs, so look up current v2 docs before using a Tauri or plugin API.
- On macOS, the simulated paste and the caret lookup need Accessibility permission. The grant is tied to the binary's signature, or to the terminal app under `mise run dev`, so an unsigned rebuild can silently lose it.
- User data lives in the OS app-data dir for `io.github.hansmissenheim.baoboard`. Tests work in temp dirs.

## Principles

- Make the smallest change that solves the task, in the style of the code around it.
- Prefer the standard library, a Tauri API, or a dependency we already have before adding a new crate or npm package.
- Logic that has branches gets a unit test: a Rust `#[test]` beside the code, or a `src/*.test.ts` file run by `node --test`.
