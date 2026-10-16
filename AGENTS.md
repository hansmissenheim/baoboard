# BaoBoard

A sticker keyboard for macOS and Windows, modeled on WeChat stickers. A global shortcut opens a popup grid, and picking a sticker pastes it into the app that had focus. Tauri v2 app: Rust backend in `src-tauri/`, vanilla TypeScript UI in `src/`, with no UI framework on purpose.

## Verify

`mise run check` is the gate, and CI runs exactly that on macOS and Windows. In a fresh checkout, run `npm ci` first.

Code behind `#[cfg(windows)]` doesn't compile on a Mac, so CI is its only check. Keep platform modules small, and in the PR's test plan, name any platform path you couldn't run.

## Workflow

A feature or bug-fix request is the owner's go-ahead to ship it. Branch, commit, push, PR, `discovery` issues, and squash-merge are granted up front, so run this loop to a merged PR and ask only in step 1. Ask before anything outside the loop, such as running the Release workflow or changing repo settings.

1. **Scope.** Read the code the change touches and check the `discovery` issues for a matching idea. Ask the owner, in one batch, whatever changes what gets built: user-visible behavior with more than one reasonable answer, a new crate or npm package, or anything that deletes or migrates user data. Pick defaults for the rest and list them in the PR. Done when the only open questions are ones you can default.
2. **Build.** Branch from `origin/main` as `mxbch-<kebab-target>`, renaming an app-made worktree branch with `git branch -m`. Done when `mise run check` is green locally and the new branching logic has tests.
3. **Review.** Run the `code-review` skill on the branch diff, fix the confirmed findings, and rerun the gate. Done when a pass finds nothing to fix.
4. **PR.** Push and open it with `gh pr create`. The body follows `.github/pull_request_template.md` with its comments replaced, and the test plan names what you ran, what you clicked, and each platform path left unchecked. Add `closes #<n>` when an issue asked for the change.
5. **CI.** Run `gh pr checks <n> --watch` in the background. On red, read `gh run view <id> --log-failed`, fix, push, and watch again. Done when every check is green, Windows included.
6. **Merge.** `gh pr merge <n> --squash`, which also deletes the remote branch. Done when the PR shows merged.
7. **Report.** A few lines to the owner: what shipped, the PR link, the defaults you chose, and what still needs a hand check, such as mouse-driven UI or Windows behavior.

Ideas that turn up along the way and aren't part of the request become `discovery` issues. A PR with a single commit uses a Conventional Commit message, so it fills in the PR title. Otherwise, write plain, short commit messages.

## Gotchas

- Tauri is pinned to v2, and v3 is still in alpha. Training data mixes v1 and v2 APIs, so look up current v2 docs before using a Tauri or plugin API.
- On macOS, the simulated paste and the caret lookup need Accessibility permission. The grant is tied to the binary's signature, or to the terminal app under `mise run dev`, so an unsigned rebuild can silently lose it.
- On macOS, the popup only takes focus when the shortcut is a real keypress. A synthetic one (from `osascript` or System Events) shows the window but leaves the previous app active.
- User data lives in the OS app-data dir for `io.github.hansmissenheim.baoboard`. Tests work in temp dirs.

## Principles

- Make the smallest change that solves the task, in the style of the code around it.
- Prefer the standard library, a Tauri API, or a dependency we already have before adding a new crate or npm package.
- Logic that has branches gets a unit test: a Rust `#[test]` beside the code, or a `src/*.test.ts` file run by `node --test`.
