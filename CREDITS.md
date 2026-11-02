# Credits — N64KRUpDoge

This fork's code is overwhelmingly upstream work. This file names the
people and projects it stands on. There is no AUTHORS or CONTRIBUTORS
file upstream; this file fills that gap for this fork.

## Upstream: SpaghettiKart (HarbourMasters)

- **SpaghettiKart** — the native PC port of Mario Kart 64, by
  [HarbourMasters](https://github.com/HarbourMasters/SpaghettiKart).
- **Maintainers** (named in the upstream README): **MegaMech, Coco, Kirito**.
- **Powered by libultraship** — the porting framework SpaghettiKart builds on.
- Every commit on this fork's `main` branch is upstream work; this fork's
  `main` tracks upstream `main` exactly (see CHANGELOG.md).

## Original game

- **Mario Kart 64** — Nintendo. No copyrighted assets are included in this
  repo. The port generates its assets from the user's own US ROM dump
  (`.z64`, SHA-1 `579C48E211AE952530FFC8738709F078D5DD215E`).
  The same rule covers donor content: each game's content comes from the
  user's own ROM at runtime, never from the repo.

## Donor reference: Diddy Kong Racing

- **Diddy Kong Racing decomp** (DavidSM64/Diddy-Kong-Racing) — used as the
  reference for hovercraft and plane movement in the N64KR project work
  (see the `n64kr` branch). No DKR assets are committed.

## This fork

- **Fork steward:** Mitchel Friend (TheUpDoge).
- **Project work:** the N64KR program (modular character / vehicle /
  movement platform) lives on the fork's **`n64kr` branch**, not on `main`.
  Its slices, recon docs, and contributors are listed in CHANGELOG.md.
