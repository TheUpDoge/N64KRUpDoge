# RECON-007 — 3D racers: where the models come from

Project: N64 Kart Racing (N64KR)
Date: 2026-11-01
Status: RECON (no code yet)

## Goal (user, 2026-11-01)
Racers drawn as real-time 3D models, as DKR does, instead of MK64's pre-rendered sprites; SpaghettiKart's sprites stay as the fallback for any character without a model. Other assets may need the same treatment later.

## Facts
- [VERIFIED: MK64 decomp + o2r] MK64 has no 3D character models: racers are 321 pre-rendered sprite frames each (SLICE-9 record).
- [VERIFIED: blend inspection] `docs/MK64_Spaghetti_Adjusted_Kart_Setup.blend` (SpaghettiKart docs, Overkart community) has a rigged 3D Mario (37-bone armature: head, cap, moustache, overalls, hands, shoes) and a 3D MK64 kart (body, seat, steering, wheels), plus Shy Guy / Toadette / Hammer Bro portrait models. No other MK64 racer.
- [VERIFIED: DKR ROM] DKR racers are 3D models with vertex animation, already decoded (`tools/n64kr/dkr_racer_to_gltf.py`).

## ROMs added by the user (2026-11-01)
Unpacked to `E:\Claude\roms\` (outside the repo; never commit). SHA-1 prefixes:

| Game | SHA-1 | MK64 cast in 3D |
|---|---|---|
| Mario Party (USA) | 1159bd567300 | all 8 (Toad host, Bowser villain) |
| Mario Party 2 (USA) | 166eda1c0567 | all 8 |
| Mario Party 3 (USA) | 6beb80ff822b | all 8 |
| Mario Tennis (USA) | 999047f07cec | all 8 playable |
| Mario Golf (USA) | e2c4e7a905b2 | Mario, Luigi, Peach, Yoshi, Wario, DK |
| Super Smash Bros. (USA) | e2929e10fccc | Mario, Luigi, Yoshi, DK |
| Donkey Kong 64 (USA) | cf806ff26036 | DK (and Diddy) |
| Paper Mario (USA) | 3837f44cda78 | 2D sprites, not useful for 3D racers |

Also: GoldenEye, Perfect Dark, Star Fox 64, Kirby 64, Blast Corps, Wave Race 64, Pilotwings 64, Beetle Adventure Racing, Top Gear (4), Rush 2, MRC, Off Road Challenge, Road Rash 64, Destruction Derby 64, Vigilante 8 (2), BattleTanx, Castlevania, Mega Man 64, Quest 64, Duke Nukem 64, Fighting Force 64, Dr. Mario 64 (future content: tracks, vehicles, music).

## Reverse-engineering status of the model sources [HYPOTHESIS until probed]
- Super Smash Bros.: mature decomp, ssb-decomp-re (`make extract` pulls assets from the ROM); a PC port (BattleShip) builds on it via OpenSmash. Fighter model format not documented in what was read.
- Mario Party 3: decomp at zestydevy/marioparty3 (83 commits; completeness unknown). Mario Party 1: MaikelChan/MarioPartyCompression extracts and decompresses the main data block (fonts, textures; model format unknown).
- Mario Tennis: decomp at dellm-79/mariotennisn64, very early (12 commits). Model format unknown.
- Donkey Kong 64: decomp at gitlab.com/dk64_decomp/dk64; dk64_lib (Python) exports geometry/textures to glTF.

## Plan
1. **Engine first, with models we have:** real-time 3D racer drawing (display list from a model, posed by steering), Diddy (ROM) then Mario (blend), sprite fallback kept, per-character switch.
2. **Model sources in parallel:** probe SSB64 (Mario, Luigi, Yoshi, DK) and Mario Party 1–3 (Peach, Wario, Toad, Bowser; all 8 in one format if it can be decoded). Pick one format that covers the most of the cast.
3. DKR music after (user's order).

Sources: github.com/dellm-79/mariotennisn64, github.com/zestydevy/marioparty3, github.com/MaikelChan/MarioPartyCompression, github.com/VetriTheRetri/ssb-decomp-re, github.com/turtlesoupy/opensmash, github.com/ThomasJRyan/dk64_lib.

## SLICE-10 v1: Diddy as a real-time 3D racer (built)
- Tool `tools/n64kr/dkr_racer_to_model.py`: writes libultraship XML display lists + vertex buffers + RGBA16 texture resources into the character mod (`racers/diddy/`): body (253 triangles, DKR textures, vertex colours) and four wheels as 10-sided cylinders (discs textured with the sprite's side-on frame, tread coloured from the head-on frame), 9 driving poses (DKR animation 0 frames 0, 10, 20, 30, 40, 48, 56, 64, 73), 21 materials. Model space: DKR units, turned to face +Z, tyres on y = 0. Racer parsing shared with the glTF exporter (`load_racer` in `dkr_racer_to_gltf.py`, which also fixes the JSON's drive_animation frame count, overwritten by the wheel loop before).
- [VERIFIED: source] MK64 picks sprite frames from the camera angle relative to the kart's visual heading `rotation[1] + unk_0C0` (`func_8002934C`, `player_controller.c`); `unk_0C0` is the visual turn (`unk_078 * 9` / `* 0x14`, clamped ±0x1C70). Forward is (−sin heading, cos heading) (frame 0 = camera behind; `func_8002AE38`). `mtxf_translate_rotate` maps local +Z to (sin y, cos y), so the model is drawn with y = −heading. The sprite quad is 18 × 18 units for the 64 × 64 frame (`gPlayerOneVtx`); model scale 0.12 world units per DKR unit (from the SLICE-9 sprite fit).
- Engine: `src/n64kr/racer3d.{h,c}` + one hook at the top of the draw part of `render_kart` (`render_player.c`): characters with a registry `model3d` are drawn as the pose display list (`ResourceGetDataByName`, as custom track meshes are) under the kart matrix; pose from `unk_0C0`. Falls back to sprites when the model isn't installed or CVar `gN64KR.Racers3D` is 0.
- [VERIFIED: build] Debug: 0 errors. In-game check pending. Not yet: steering front wheels, wheel spin, slope tilt (pitch/roll), boo/star/flash effects on the model.
- [VERIFIED: user playtest + screenshots] 3D Diddy works in a race: faces forward, right size overall, sits on the road, leans into turns ("phenomenal"). Feedback: tyres too large. Fix: wheel cylinders at 0.75× radius, 0.6× width of the sprite size (DKR's flat billboards read smaller than solid cylinders); ground re-derived from the smaller tyres (−46.4 → −34.8 DKR units). Mod-only change (no rebuild). Recheck pending.

## SLICE-10 v2: effects and steering (built)
User playtest of v1: banana spin works (it rotates the kart); Boo did nothing; turbo drew a translucent black square; front tyres didn't turn.
- [VERIFIED: source] The square: while boosting MK64 draws a faint copy of the kart sprite behind it (`func_80025DE8`, `render_player.c`, `boostPower >= 2`), loading `sKartTexture`, which the 3D path never sets. Now hooked: the afterimage is the 3D model at alpha 0x40 (`n64kr_racer3d_draw_boost`, effect matrix as the original).
- Boo / Lakitu fizzle / battle bomb or loss: the model's materials take alpha = texel × primitive alpha and set no render mode; the engine sets primitive alpha and render mode as `render_kart` does for sprites (Boo: `player->alpha` on the player's own screen, `gPlayerOtherScreensAlpha` on others; MK64's translucent mode).
- Front wheels: exported as separate display lists around their centres (`wheel2`, `wheel3`), listed in `model.json` with their centres; the engine multiplies a steering matrix onto the kart matrix (y rotation, up to 7000/65536 of a turn at full visual turn, DKR's stick × 100). `model.json` is read at runtime (`racer3d_model.cpp`, nlohmann json), so scale and pose count are no longer hard-coded in C.
- [VERIFIED: build] Debug: 0 errors. In-game check pending. Not yet: star / lightning colour effects on the model, slope tilt, wheel spin.

## SLICE-10 v3 (built)
User playtest of v2 (screenshots): turbo afterimage works; front wheels turn only slightly; tyres still large, don't roll; Diddy about twice MK64 racer size; star made sparks but no colour flash; dust puffs sit inside the oversized tyres.
- Scale: 0.12 → 0.06 world units per DKR unit. Tyres: 0.6× radius, 0.45× width of the sprite size.
- All four wheels are separate display lists around their centres (`model.json` `wheels` with `steers` flags); the engine draws each with v · roll(x) · steer(y) + centre. Roll angle = `tyreSpeed << 6` (tyreSpeed is MK64's sprite wheel counter, 0..0x3FF per cycle, `kart_dma.c`). Steering reaches DKR's full lock (7000/65536 turn) at a third of the visual-turn clamp (the visual turn stays small in normal driving).
- Colour effects: materials are 2-cycle; cycle 2 is MK64's racer colour formula (1 − env) × colour + prim (`func_8004B614`), with the engine setting prim = gPlayerRed/Green/BlueEffect, env = gPlayerCyan/Magenta/YellowEffect, prim alpha = alpha. Render modes G_RM_PASS + TEX_EDGE2 (opaque) or + render_kart's translucent mode.
- [VERIFIED: build] Debug: 0 errors. In-game check pending.
