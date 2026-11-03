# Tutorial: add another DKR track

Ancient Lake and Fossil Canyon are converted and tested. This tutorial converts another DKR race track and puts it in a cup. You need the setup from [TUTORIAL_SETUP.md](TUTORIAL_SETUP.md), including Blender 4.3 with HarbourMasters' Fast64 enabled.

> Status: the pipeline is tested on Ancient Lake and Fossil Canyon. Other tracks should convert the same way, but each DKR track has its own quirks (water, moving objects, zippers, shortcuts), so expect to check and maybe fix things. Write down what you find.

## 1. Pick a track

```bash
$N64KR dkr-level "$DKR" --list
```

The race tracks: `ancient_lake`, `fossil_canyon`, `jungle_falls`, `hot_top_volcano` (Dino Domain); `whale_bay`, `crescent_island`, `pirate_lagoon`, `treasure_caves` (Sherbet Island); `windmill_plains`, `greenwood_village`, `boulder_canyon`, `haunted_woods` (Dragon Forest); `snowball_valley`, `everfrost_peak`, `walrus_cove`, `frosty_village` (Snowflake Mountain); `spaceport_alpha`, `spacedust_alley`, `darkmoon_caverns`, `star_city` (Future Fun Land). Hubs, boss races, menus and cutscenes are in the list too but aren't race tracks.

This tutorial uses **Whale Bay**.

## 2. Convert

```bash
$N64KR dkr-level "$DKR" whale_bay build/dkr_tracks --mk64
"$BLENDER" -b --python tools/n64kr/blender_dkr_track.py -- build/dkr_tracks/whale_bay_mk64 build/x64/mods whale_bay
$N64KR dkr-track-scene build/dkr_tracks/whale_bay_mk64 build/x64/mods whale_bay "Whale Bay"
```

What each step does:

1. **`n64kr dkr-level --mk64`** reads the level from your ROM and writes `build/dkr_tracks/whale_bay_mk64/`:
   - the track as glTF plus textures, scaled to MK64 size, with the start at the origin racing toward −Z;
   - each material tagged with MK64 surface, collision and draw layer;
   - `whale_bay_mk64.json` with the AI path, built from one vehicle's checkpoints, plus the start grid and item box positions.
2. **`blender_dkr_track.py`** builds the SpaghettiKart track with Fast64 and writes the mod `build/x64/mods/n64kr-dkr-whale_bay/`. It also saves a `.blend` next to the converted files, so you can fix things by hand and re-export.
3. **`n64kr dkr-track-scene`** writes `scene.json`: the name, resource id `n64kr:dkr_whale_bay`, item boxes, minimap and sky colours.

Read the tool output before going in game. The converter prints the AI path size, the checkpoint count and the start grid's range (`MK64 layout: ... start grid z a..b`); all grid values should be positive (behind the line).

## 3. Put it in a cup

MK64's menus only race tracks that are in a cup. N64KR cups are defined in `src/n64kr/cups.cpp`, four tracks each:

```cpp
const CupDef kCups[] = {
    { "n64kr:rare_cup", "rare cup", "diddy kong racing + goldeneye 007",
      { "n64kr:dkr_ancient_lake", "n64kr:dkr_fossil_canyon", "n64kr:ge_dam_speedway", "n64kr:ge_dam_speedway_b" } },
    { "n64kr:sherbet_cup", "sherbet cup", "diddy kong racing",
      { "n64kr:dkr_whale_bay", "n64kr:dkr_crescent_island", "n64kr:dkr_pirate_lagoon", "n64kr:dkr_treasure_caves" } },
};
```

- Names are lower case; MK64's menu font prints them in its own style.
- A cup only appears when **all four** of its tracks are installed. Until then, the game log says which one is missing.
- New cups appear after the Rare Cup in Grand Prix and VS. The cup select arrows show the way.
- To try one track quickly, temporarily swap it into the Rare Cup.

Build (`cmake --build build/x64 --config Release`) and race.

## 4. What to check in game

| Check | If it's wrong |
|---|---|
| The countdown starts and all racers sit behind the line | Start grid or road direction: check the converter's printed start grid range; open the saved `.blend` to look at the path. |
| Laps count on every lap and positions make sense | The AI path: the converter uses one vehicle's checkpoints (the car's if it has at least 3, else the vehicle with the most). DKR keeps a set per vehicle, and mixing them breaks laps. |
| You can't fall through the road or get stuck in walls | Surfaces and collision tags on materials; open the saved `.blend`. |
| Items don't show through hills | Fixed in the engine for every track (`n64kr_restore_track_state`); if it comes back, check the track's draw layers. |
| Trees and decals have clean edges | Transparency class (opaque / cutout / translucent) is picked per texture from its alpha. |
| The minimap matches | The minimap width is padded to a multiple of 16 (4-bit textures need it). |

## 5. Music

Converted DKR tracks have no music yet (`Sequence: -1` in `scene.json`, so the game plays nothing). The `n64kr` tool's sequence renderer was written for GoldenEye but reads the same libultra bank format DKR uses, so DKR music is a natural next step. See the GoldenEye tutorial for how songs are registered and assigned to a track.

## Limits to know

- Trophies won in N64KR cups aren't kept in the save file, which only has room for MK64's cups.
- Time trials don't list N64KR cups (MK64's records have no slots for them).
- DKR's moving scenery, animated water and zippers aren't converted yet.
