# Tools — N64KRUpDoge

Tools are designated here and kept isolated from game code. Nothing in
`tools/` ships in the game; tools are build-time and pipeline-time only.
Game code lives in `src/`; N64KR game code lives in `src/n64kr/`.

## 1. Upstream toolchain — `tools/` (on `main` and `n64kr`)

The SpaghettiKart decompilation / recompilation toolchain, inherited
unchanged from upstream. Subdirectories: `asm_processor` (MIPS assembly
processing), `blender` (Blender-side scripts), `ido-recomp` (IDO
recompilation), `stb` (stb libraries). Notable scripts at `tools/` root
include `bin2c.py` (binary → C), `n64cksum.c` / `.h` (ROM checksum),
`n64graphics.c` / `.h` (N64 graphics formats), `gfxdis_multi.py`
(display-list disassembly), `m2ctx`, `mkgccgen.py`, `format.py`,
`windows_build.py`, plus patch helpers (`apply_patch.sh`,
`create_patch.sh`, `revert_patch.sh`). See also the repo-root helpers
`extract_assets.py`, `python_convert.py`, `progress.py`.

Isolation: upstream tools stay in `tools/`; they are never mixed into
`src/` game code.

## 2. N64KR pipeline tools — `tools/n64kr/` (`n64kr` branch only)

The Diddy Kong Racing content pipeline. These do **not** exist on `main`.

- `dkr_level_to_gltf.py` — DKR level → glTF.
- `blender_dkr_track.py` — Blender DKR track build.
- `dkr_track_scene.py` — track scene assembly.
- `dkr_racer_to_gltf.py`, `dkr_racer_to_model.py` — DKR racer → model.
- `blender_dkr_racer_sprites.py`, `blender_dkr_racer_hud.py` — racer
  sprites and HUD art.
- `dkr_voice_to_wav.py` — DKR voice clips → WAV.
- `extract_dkr_hover_stats.py` — real DKR hover stats for SLICE-3/4/5.

Output goes to `build/x64/mods/` and is never committed, like all
generated assets.

## 3. N64KR Rust tool — `tools/n64kr-rs/` (`n64kr` branch only)

A Rust binary (with `Cargo.toml` / `Cargo.lock` for reproducible builds)
that extracts GoldenEye Dam rooms, textures, and level geometry to glTF
(RECON-008). Source under `src/ge/` (background, display lists, images,
textures, modules) plus `gltf.rs`, `track.rs`, `rom.rs`, `rarezip.rs`,
`png.rs`, `main.rs`.

## Isolation rules (from the project's CLAUDE.md, `n64kr` branch)

1. N64KR code goes in `src/n64kr/`; edits to upstream files are limited
   to thin hooks, each marked `// N64KR HOOK`, so upstream updates stay
   mergeable.
2. Never commit ROMs, extracted assets, `.o2r` files, or DKR misc-asset
   values (`*.z64`, `*.n64`, `*.v64`, `*.o2r`, `baseroms/`, `build/`).
3. Tools never ship in the game and never live in `src/`.
4. Don't open PRs to SpaghettiKart or the DKR decomp without the user's
   say-so; read their contribution rules first (some decomp communities
   ban AI-generated contributions).
