# SLICE-9 — First DKR character

Project: N64 Kart Racing (N64KR)
Date: 2026-11-01
Status: IN PROGRESS. Step 1 (art pipeline): Diddy renders with driving pose and front-wheel steering, in-game check pending; engine slot for a 9th character not started.

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

### Front-wheel steering (user feedback: "back tires shouldn't pivot like the front")
- [VERIFIED: decomp] DKR cars steer only attach points 2 and 3: `obj[2]/obj[3]->trans.rotation.y_rotation = gCurrentStickX * 100` (`src/racer.c:6215-6218`); 0 and 1 stay straight. For Diddy's car, 2 and 3 are the `fronttyre` parts.
- [VERIFIED: setup .blend] Frames 0–188 are 9 blocks of 21: within a block `Kart Root` yaws the kart 3°/frame relative to the camera (frame 0: 180°, frame 20: 120°) and Mario's arm pose steers (pose period 21 frames). Frames 189–288 keep him straight; 289–320 tumble.
- Renderer: front wheels get a steering yaw that grows with the frame's position in its block, `steer = MAX × (frame % 21) / 20`, MAX = stick 70 × 100 = 7000/65536 turn (~38°), applied by rotating the camera offset into the wheel's yaw (as DKR does) before picking the angle frame. Rear wheels follow the body only. Frame 20 preview: rear wheels at the body's angle, front-left wheel turned further into the turn.
- Steering sign fix (user feedback: "the rear tires are turning"): per-wheel angle debug showed the front wheels turning *against* the body in frames 0–20 (front 50–57° vs rear 59–65° at frame 20), so the straight rear wheels looked like the turning ones. The steering rotation sign was changed. [VERIFIED: debug output]
- In game the user then saw "the front stays straight while the rear turns": the change had turned the fronts the wrong way. Debug at frame 20 (kart turned left, camera at its left-rear): rears 59–65°, fronts 12–19° (turned back toward the camera, reading as straight). Fixed by yawing the wheel by +steer (camera offset rotated by −steer): fronts now 80–81° vs rears 59–65°, 41–50° vs 26–37° at frame 10. [VERIFIED: debug output; in-game check pending]

### Driver pose (user feedback: "the character is doing a T-pose")
- [VERIFIED: decomp] The model's base vertices are a T-pose; DKR always draws racers through vertex animation. Driving uses animation 0 with frame `40 - steerAngle / 2`, clamped 0..73 (`src/racer.c:1794-1805`); steerAngle follows stick x, so 40 is straight, lower frames steer right, higher frames steer left.
- [VERIFIED: decomp asm + ROM probe] Vertex animation format, read from `obj_animate` (`src/hasm/obj_animate.s`) and `model_anim_init` (`src/object_models.c:892`):
  - `ASSET_ANIMATION_IDS` (section 30): s16 per object model, its range in `ASSET_OBJECT_ANIMATIONS` (31 table / 32 data). Each animation is DKR-compressed; s32 keyframe count first.
  - Keyframe 0: 12-byte header, then s16 xyz offset per animated vertex, added to the base vertex. Keyframes 1..: stride `12 + 3 × animated count`, 12-byte header then s8 xyz deltas, applied cumulatively. A frame is 1/16 keyframe; the fraction scales the next keyframe's delta (`delta × t >> 4`). The keyframe headers (s16s read at bytes 0/2/4/10) are not used yet.
  - Model 0x4A = animated vertex count, 0x4C = s16 animated slot per vertex (−1 static); several vertices can share a slot.
  - Diddy (model 19): 325 vertices, 232 animated using 111 slots; 5 animations; animation 0 has 6 keyframes = 80 frames.
- Exporter writes animation 0's frames as glTF morph targets (Blender shape keys `anim0_fNN`); the renderer picks the pose per sprite frame: frames 0–188 go from 40 (straight) to 73 (full left lock) over each 21-frame block, alongside the front-wheel steer; other frames stay at 40. MK64 mirrors these sprites for right turns.
- [VERIFIED: render] Front views of poses 0 / 40 / 73: leaning to Diddy's right / straight with both hands on the wheel / leaning to his left. The sprite previews show no T-pose.
- Next in-game report: "front is fixed but the rears are turning again". Cause [VERIFIED: debug output + renders]: each wheel picked its sprite angle from its own position, and the sprite camera is close (≈358 DKR units behind, wheels ±40 apart), so at frame 6 (body turned 18°) the rear-right tyre got 25° and the rear-left 12°. The far tyre's disc then faced sideways like a steered wheel; a 3D tyre would hide that inner face behind the body. Fix: all wheels take the view direction from the vehicle centre (rears now 17°/17° at frame 6, 58°/58° at frame 20; fronts add the steer: 29°, 79°).
- Also matched DKR's flip [VERIFIED: decomp src/camera.c:1097-1118]: on the negative side DKR turns the sprite 180° in its plane (`tiltAngle += 0x8000`), not a one-axis mirror; the side is the sign of the lateral offset, negated when the camera is in front. No visible change for Diddy's tyre (its sprite is vertically symmetric). DKR's small camera-height tilt is not applied.
