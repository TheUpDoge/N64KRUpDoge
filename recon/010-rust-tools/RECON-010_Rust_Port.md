# RECON-010 — DKR tools ported to Rust

Project: N64 Kart Racing (N64KR)
Date: 2026-11-02
Status: done; outputs verified identical to the Python tools

## Goal (user)
"Lets convert what we can that has been made into rust without breaking things."

## What moved
The six Python tools that read the DKR ROM became subcommands of the existing Rust program `n64kr` (`tools/n64kr-rs`), next to the GoldenEye commands:

| Python tool (removed) | Rust command | Module |
|---|---|---|
| `extract_dkr_hover_stats.py` | `dkr-hover-stats` | `src/dkr/hover.rs` |
| `dkr_voice_to_wav.py` | `dkr-voice` | `src/dkr/voice.rs` |
| `dkr_racer_to_gltf.py` | `dkr-racer` | `src/dkr/racer.rs` |
| `dkr_racer_to_model.py` | `dkr-racer-model` | `src/dkr/model.rs` |
| `dkr_level_to_gltf.py` | `dkr-level` | `src/dkr/level.rs`, `objects.rs`, `tex.rs`, `gltf.rs`, `mod.rs` (shared DKR reader) |
| `dkr_track_scene.py` | `dkr-track-scene` | `src/dkr/scene.rs` |

New shared pieces: `src/json.rs` (JSON parse and output formatted like Python's `json.dump(indent=n)`: key order kept, int / float kept apart, floats as Python's repr) and a greyscale mode in `src/png.rs`.

Not moved: the four Blender scripts (`tools/n64kr/blender_*.py`). Blender only runs Python (bpy). They didn't import the removed tools [VERIFIED: grep].

## Parity check [VERIFIED: tool runs on the user's DKR ROM, US v1.1]
Both versions were run on the same inputs and every output file compared: text, XML, textures, WAV and glTF buffers byte for byte; JSON and glTF by value (floats to 1e-9 relative; generator string ignored); PNGs by decoded pixels (the encoders compress differently).

| Command | Inputs | Files | Result |
|---|---|---|---|
| `dkr-hover-stats` | ROM | 1 | byte-identical |
| `dkr-voice` | diddy, banjo, conker | 50 | 47 WAVs byte-identical; the 3 clip lists differ only in line endings (Python wrote CRLF on Windows) |
| `dkr-racer` | diddycar, BanjoCar, ConkaCar | 474 | all identical (465 PNGs, 6 JSON/glTF, 3 buffers byte-identical) |
| `dkr-racer-model` | the three, Banjo at 1.2 | 609 | 606 byte-identical (display lists, vertices, textures), 3 JSON equal |
| `dkr-level` | ancient_lake, fossil_canyon, whale_bay (`--mk64`), whale_bay (plain) | 131 | all identical; the glTF buffers byte-identical (the layout transform, path and floor math give the same doubles as Python) |
| `dkr-track-scene` | Fossil Canyon (existing scene), Dam Speedway (GoldenEye actors, music), Whale Bay (new scene) | 6 | all identical |

End to end: `blender_dkr_racer_faces.py` on the Rust racer export rendered Banjo's faces identical to those from the Python export (4 frames compared pixel by pixel) [VERIFIED].

## Changes for users
- The content pipeline needs Rust (`cargo build --release` in `tools/n64kr-rs`); Python is only needed for SpaghettiKart's own build and inside Blender.
- Commands: `n64kr dkr-…` replace `python -I tools/n64kr/….py` with the same arguments, in the same order. Docs and tutorials updated.
- Adding a racer's one-off voice lines is now an entry in `special()` in `src/dkr/voice.rs`.
