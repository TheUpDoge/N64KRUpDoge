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
