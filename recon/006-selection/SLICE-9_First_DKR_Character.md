# SLICE-9 — First DKR character

Project: N64 Kart Racing (N64KR)
Date: 2026-11-01
Status: IN PROGRESS. Step 1 (art pipeline) rendering; engine slot for a 9th character not started.

## Plan
1. **Art pipeline**: DKR racer 3D model → MK64 kart sprite frames. Validate by replacing an existing character's sprites with a texture pack (supported by SpaghettiKart).
2. **Engine slot**: a 9th registry entry with its own sprite paths, portrait, name; GP points / no-character marker / time trial save bits (SLICE-8 list).
3. **Select screen**: grid larger than 8; CPU character choice.

Two long-term rendering routes for DKR racers (user note: DKR racers are 3D):
- **Pre-rendered sprites** (this slice): the DKR 3D model rendered from every MK64 sprite angle. Fits MK64's sprite-based racer engine as-is.
- **Real-time 3D racers** (later): a new racer renderer + DKR vertex animation. Same extracted models.

## MK64 kart sprites [VERIFIED: source + asset headers]
- 321 frames per character, `textures/karts/<slot>/<slot>_frameNNN_wheelW` for frames 0–288 × 4 wheel-spin variants (1156) and `<slot>_frameNNN` for tumble frames 289–320 (32), plus `<slot>_palette` (`include/assets/textures/karts/mario_kart.h`).
- Frames are grouped into 18 view groups (`DECLARE_KART_GROUP` / `DECLARE_KART_TABLE0/1` in `src/kart_dma.c`); the wheel variant is `tyreSpeed >> 8` (`kart_dma.c:607`).
- Texture packs replace textures by path with PNG (`docs/textures-pack.md`); SpaghettiKart's custom characters are texture packs over these frames (`docs/characters/characteroverview.md`).
- Render template: `docs/MK64_Spaghetti_Adjusted_Kart_Setup.blend`: EEVEE, 64×64, frames 0–320, fixed perspective camera behind the kart (y −3.29), kart turned by animated empties `Root` and `Kart Root`; Mario's armature animation (1–290) is only his steering lean. Stock wheel rims span x ±0.63; tyres' bottom z −0.26.

## DKR racers [VERIFIED: DKR decomp + ROM probe]
- Racer objects: behaviour `BHV_RACER` (1), model type 3D. Per character per vehicle: `diddycar`, `SWcar`, `KremCar`, `BadgerCar`, `TortCar`, `ConkaCar`, `TigerCar`, `BanjoCar`, `ChickenCar`, `MouseCar`, the `*hover` and `*plane` sets, plus bosses.
- Header lists 6 models = detail series; model 0 most detailed (`diddycar`: model 19, 257 triangles, 19 textures; 6th model is an 8-vertex placeholder).
- ObjectModel attach points: s16 vertex indices @0x14, count @0x18. Header vehiclePartIds @0x14 / vehiclePartIndices @0x18 (count @0x56): `diddycar` → `backtyre` ×2, `fronttyre` ×2, vehicle-part sprite objects (model type 2) with 4 sprite frames 179–182 (wheel spin, 12×14 px), part scale 2.9 / 2.4.
- Wheel positions (model units): back (±40, 0, +26), front (±38, −4, −42) → the model faces −Z, like MK64.

## Tools
- `tools/n64kr/dkr_racer_to_gltf.py`: racer body (model 0) → glTF + textures; wheel sprite frames → PNG; `<object>_racer.json` with wheel attach positions, part scales, sprite anchors. Shares `write_gltf()` with the track converter (refactored out of `convert()`; Ancient Lake output unchanged).
- `tools/n64kr/blender_dkr_racer_sprites.py`: opens the kart setup .blend, hides Mario + kart, imports the racer, fits wheel span to wheel span (DKR wheel centres ± half sprite width → stock rim span; wheel bottoms → stock tyre bottom), parents to `Kart Root`, unlit materials (vertex colour × texture), wheels as camera-facing quads with the 4 spin frames; renders 321 + 3 × 289 frames (8-bit RGBA) and packages them as a texture pack for a kart slot. `--preview` renders 12 sample frames.

## Results so far
- [VERIFIED: render] Diddy's car body renders correctly (textures, colours, cap, star shirt).
- [VERIFIED: preview frames] Fit scale 0.0109: Diddy's height 1.41 vs Mario's 1.49, wheels on the ground line; frames show rear views turning through the steering angles, front view at 288, tumble frames flipping the car.
- Bugs fixed on the way: fit by full bbox used the unsubdivided "Kart" cage (bottom −0.74, too low) and Diddy's arms (too small); preview PNGs were 16-bit (setup scene default), forced to 8-bit.
- **Wheels (fixed after the user's DKR screenshot)** [VERIFIED: decomp + render]: the user's screenshot of DKR shows Diddy's car with big red tyres. Each wheel sprite (179–182, one per spin state) has **16 frames**: the wheel seen from 0° (head-on, frame 0, a thin sliver) to 90° (side on, frame 15, round tyre with yellow hub). DKR picks the frame per draw from the camera position relative to the vehicle's vertical plane, `frame = floor(angle / 90° × 16)`, mirrored when the camera is on the vehicle's −X side (`render_sprite_billboard`, decomp `src/camera.c:1077-1112`); the spin state is the part's `modelIndex`. The exporter now writes all 4 × 16 frames; the renderer picks the frame per rendered frame. A second bug put all four wheels at one point (new objects' `matrix_world` is only valid after a scene update); fixed with `view_layer.update()` before fitting. Previews now show red tyres on both sides, changing with the angle.
