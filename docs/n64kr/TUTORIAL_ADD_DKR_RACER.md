# Tutorial: add another DKR racer

Diddy, Banjo and Conker are in. This tutorial adds a fourth DKR racer, **Krunch**, end to end. The same steps work for Bumper, Tiptup, Timber, Drumstick, Pipsy and T.T. You need the setup from [TUTORIAL_SETUP.md](TUTORIAL_SETUP.md) done first.

> Status: Diddy, Banjo and Conker were made exactly this way and are tested in game. The other seven DKR racers haven't been converted yet, so you may be the first. Note anything that looks off.

## 1. Look up the racer

| Racer | DKR car object | voice group | select line id |
|---|---|---|---|
| Krunch | `KremCar` | `krunch` | `0x0087` |
| Bumper | `BadgerCar` | `bumper` | `0x0088` |
| Tiptup | `TortCar` | `tiptup` | `0x0089` |
| Timber | `TigerCar` | `timber` | `0x008B` |
| Drumstick | `ChickenCar` | `drumstick` | `0x008D` |
| Pipsy | `MouseCar` | `pipsy` | `0x008E` |
| T.T. | `SWcar` | `tt` | `0x008F` |

`python -I tools/n64kr/dkr_racer_to_gltf.py "$DKR" --list` prints every racer object in your ROM (cars, hovercraft, planes). The voice ids come from the DKR decomp's `include/sound_ids.h`.

Pick three short names for the racer, used everywhere below:

| Name | Krunch example | Used for |
|---|---|---|
| hud name | `krunch` | faces, portrait, minimap, nameplate (`textures/.../krunch_*`) |
| kart folder | `krunch_kart` | the 1188 kart sprites |
| voice name | `krunch` | `sound/n64kr/krunch/*.wav` |

## 2. Add the one-off voice lines (optional)

Every racer gets DKR's eight positive and eight negative lines automatically. The one-off lines ("My name's Krunch", a laugh, a surprised "woah") need an entry in `SPECIAL` in `tools/n64kr/dkr_voice_to_wav.py`, with ids from the decomp's `include/sound_ids.h`:

```python
SPECIAL = {
    ...
    "krunch": {"laugh": 0x0073, "laugh2": 0x0047, "woah": 0x0093, "select": 0x0087},  # LAUGH, HOHOO, EHH, "My name's Krunch"
}
```

`select` plays when a player picks the racer, `laugh` and `laugh2` on a win, and `woah` when the racer falls. Leave out any line the racer doesn't have; the engine picks from the clips that exist.

## 3. Generate the assets

```bash
python -I tools/n64kr/dkr_racer_to_gltf.py "$DKR" KremCar build/dkr_racers
"$BLENDER" -b docs/MK64_Spaghetti_Adjusted_Kart_Setup.blend --python tools/n64kr/blender_dkr_racer_sprites.py -- build/dkr_racers/KremCar build/x64/mods/n64kr-dkr-krunch krunch_kart
"$BLENDER" -b --python tools/n64kr/blender_dkr_racer_hud.py -- build/dkr_racers/KremCar build/x64/mods/n64kr-dkr-krunch krunch build/x64/mk64.o2r toad 60,170,60
"$BLENDER" -b --python tools/n64kr/blender_dkr_racer_faces.py -- build/dkr_racers/KremCar build/x64/mods/n64kr-dkr-krunch krunch
python -I tools/n64kr/dkr_voice_to_wav.py "$DKR" krunch build/x64/mods/n64kr-dkr-krunch
python -I tools/n64kr/dkr_racer_to_model.py "$DKR" KremCar krunch build/x64/mods/n64kr-dkr-krunch 1.2
```

- `60,170,60` is the kart colour on the minimap (any R,G,B).
- The last number is the 3D model's size. DKR's racers are close to the same size; Banjo uses 1.2 so he reads as a heavyweight next to MK64's cast. A big racer like Krunch probably wants about the same.
- Check the sprite render first with `--preview` after the kart folder name (12 frames instead of 1188).

Check the results before touching code:
- `build/dkr_racers/KremCar/frames/` has preview renders.
- `build/x64/mods/n64kr-dkr-krunch/textures/player_selection/krunch_face_00.png` to `_22.png` are the select faces: idle, blink, picked, lost.
- `build/x64/mods/n64kr-dkr-krunch/sound/n64kr/krunch/krunch_voice.txt` lists the clips, including any left out as repeats.

## 4. Register the racer (2 small code edits)

**`src/n64kr/character.h`**: give it the next free id (ids go up to 15; `N64KR_MAX_CHARACTERS` is 16):

```c
#define N64KR_CHAR_KRUNCH 12
```

**`src/n64kr/character.c`**: add a registry line at the end of `sCharacters`. Each column names the stock MK64 character to borrow from (`MARIO`, `LUIGI`, `YOSHI`, `TOAD`, `DK`, `WARIO`, `PEACH`, `BOWSER`):

```c
//  name      voice   sprites hud     body    ai      stats   kartFolder     hudName   voiceName  model3d
{ "Krunch",  BOWSER, MARIO,  TOAD,   BOWSER, BOWSER, BOWSER, "krunch_kart", "krunch", "krunch", "racers/krunch" },
```

| Column | Meaning | Keep as |
|---|---|---|
| voice | engine sound and fallback voice | pick by feel (a heavy kart sounds like DK or Bowser) |
| sprites | sprite layout of the kart folder | `MARIO` (the DKR sprites are rendered in Mario's layout) |
| hud | HUD layout of the portrait | `TOAD` (the portrait is styled on Toad's) |
| body | size, particle colours, bump weight | the weight class you want |
| ai | CPU item and driving behaviour | the weight class you want |
| stats | MK64 kart stats for the "own character" vehicle profile | the weight class you want |

**`src/n64kr/character_select.c`**: put it in a free cell of the DKR page:

```c
static const s32 sDkrPage[GRID_CELLS] = {
    N64KR_CHAR_DIDDY, N64KR_CHAR_BANJO, N64KR_CHAR_CONKER, N64KR_CHAR_KRUNCH, -1, -1, -1, -1,
};
```

That's all the code. Faces, nameplate, voice, sprites, portrait and 3D model are found from the registry names, and any DKR page racer nobody picks joins races as a CPU.

## 5. Build and test

```bash
cmake --build build/x64 --config Release
```

Checklist in game:
- The DKR page shows Krunch's face and name in the fourth cell; it blinks now and then.
- Picking him plays "My name's Krunch" and his picked animation.
- In a race: kart sprites or the 3D model look right from all sides, the portrait shows in the HUD, his voice plays when he hits or is hit.
- A Grand Prix win shows his face on the award screen.

## Limits to know

- **Menu seats.** MK64's menus track 8 stock ids, so a page character takes a stock character's seat in the race. The DKR page has 8 cells, the registry 16 ids.
- **Blinks** need a DKR eye texture with 2–4 frames. Most racers have one; if not, the face just doesn't blink.
- **Time trials** save a page character as the stock character of its cell.
