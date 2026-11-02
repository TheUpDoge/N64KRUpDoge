# Changelog — N64KRUpDoge

## Fork `main` branch

- **No fork-specific changes.** `main` tracks
  HarbourMasters/SpaghettiKart `main` exactly: 1,241 commits, HEAD
  `ce0f935` ("Allow Match Refresh Rate for every renderer (#769)"),
  0 behind / 0 ahead as of 2026-11-01. The upstream changelog is the
  upstream repo's commit history.
- **CI note:** GitHub disables workflow runs on forked repos by default,
  so this fork's Actions tab has no runs. That is the fork default, not a
  build failure. The upstream repo runs CI (Windows / Linux / macOS /
  Switch nightly builds).

## Project work — `n64kr` branch (N64KR program)

All entries below are this fork's `n64kr` branch, built on the SpaghettiKart
host. Character, vehicle, and movement model are separate and swappable;
N64KR code lives in `src/n64kr/` with thin `// N64KR HOOK` markers in
upstream files. Claims are labelled VERIFIED / CONCLUSION / HYPOTHESIS in
the recon docs; only VERIFIED items are called done here.

- **SLICE-0 — Baseline:** SpaghettiKart builds per upstream
  `docs/BUILDING.md`; `mk64.o2r` generated from the user's ROM; one race
  driven.
- **SLICE-1 — VehicleProfile seam:** `n64kr::VehicleProfile` (top speed,
  gravity, friction, acceleration curve, handling, bounding box, hop);
  per-player `vehicleProfileId`; physics reads routed through it.
- **SLICE-2 — Movement switch:** driving physics routed through a
  movement selector chosen by the profile.
- **SLICE-3 / 4 / 5 — Hover:** `n64kr_movement_hover` ported from DKR
  movement code, keeping MK64 terrain collision; works with real DKR stats.
- **SLICE-6 — First DKR track:** Ancient Lake converts from the DKR ROM
  and drives three laps in game. Pipeline: `tools/n64kr/` conversion
  scripts → Blender track build → mod output (never committed).
- **SLICE-7 — Vehicle select:** R on the character select screen toggles
  Kart / Hover per player.
- **SLICE-8 — Character registry:** `src/n64kr/character.{h,c}`;
  characterId indexes N64KR characters; MK64 lookups go through
  `n64kr_char_*` helpers. Known watch item: one unexplained intermittent
  crash (null I4 texture, Wario Stadium race start).
- **SLICE-9 — First DKR character:** Diddy rendered from his DKR model
  into kart sprites, registry id 9 (8 stays MK64's "no character" marker);
  L on the select screen seats him. Portrait, minimap icons, name, voice
  at real playback rate, engine slot — VERIFIED by user playtest.
- **SLICE-10 — 3D racers:** 3D wheels roll at half rate without wrap snaps.
- **RECON-008 — GoldenEye Dam track:** first Rust tool
  (`tools/n64kr-rs/`) extracts Dam rooms, textures, and level geometry to
  glTF; Dam Speedway: banked oval over GoldenEye Dam's reservoir.

## Unreleased / future fork changes

- Fork-level docs (this file, CREDITS.md, TOOLS.md) — the first
  fork-specific additions on `main`.
- Record future fork changes here, newest first, with the same
  VERIFIED / CONCLUSION / HYPOTHESIS labelling.
