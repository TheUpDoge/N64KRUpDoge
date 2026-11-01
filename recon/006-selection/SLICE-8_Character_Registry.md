# SLICE-8 — Character registry

Project: N64 Kart Racing (N64KR)
Date: 2026-11-01
Status: PASSED (user playtest 2026-11-01); one unexplained intermittent crash on watch

## Goal
Stop MK64's 8 characters being hard-wired. A racer's `characterId` now indexes the N64KR character registry; upstream code reads MK64's per-character tables and sound banks through registry accessors. The registry holds exactly MK64's 8, each mapped to itself, so the game must be unchanged. New characters (SLICE-9) become new entries.

## Map of `characterId` uses [VERIFIED: grep at `1ba7d67`]
~195 uses in 22 files. Grouped:

| Category | Accessor | Uses routed | Where |
|---|---|---|---|
| Voice clips (`characterId * 0x10 + sound`) and engine sound (`characterId + SOUND_ARG_LOAD(0x01,0x04,0xFF,…)`, engine tuning `switch`es, `D_800EA154`) | `n64kr_char_voice` | 30 + 41 | `audio/external.c` (all 30), `effects.c` 18, `racing/actors.c` 12, `racing/actors_extended.c` 3, shell `update.inc.c` 5, `player_controller.c` 2, `code_80005FD0.c` 1, `code_80057C60.c` 1, `ending/ceremony_and_credits.c` 1 |
| Kart sprites: `gKartTextureTable0/1`, `gKartTextureTumbles`, `gKartPalettes`, `D_800DDEB0` (sizes); wheel palettes (`render_player.c` `character`) | `n64kr_char_sprites` | 18 + 1 | `kart_dma.c`, `render_player.c:1681` |
| HUD: `gPortraitTextures/TLUTs`, `common_texture_minimap_kart_character/progress`, names `D_800E76A8` | `n64kr_char_hud` | 8 + 3 | `render_objects.c`, `menu_items.c` |
| Body: `gCharacterSize`, particle `primColors`/`envColors`, local size tables `sp48`/`sp54`/`sp80`, racer-vs-racer bump weight `gFloatArray802B8790` | `n64kr_char_body` | 14 + 10 + 2 | `player_controller.c`, `render_player.c`, `code_80057C60.c`, `racing/race_logic.c` |
| CPU behaviour: heavyweight exceptions (`== WARIO/BOWSER/DK`), item-strategy `switch` | `n64kr_char_ai` | 4 | `code_80005FD0.c` |

Total: 133 routed (script counts per file matched the map). Each is a one-expression change; every touched file has `#include "n64kr/character.h" // N64KR HOOK` (13 files). `.inc.c` shell files are covered by `racing/actors_extended.c`.

### Left on raw `characterId` on purpose
- Bookkeeping keyed by the character itself: `gGPCurrentRaceCharacterIdByRank`, `gGetPlayerByCharacterId[10]`, `gGPPointsByCharacterId[8]`, spawn, replays.
- Functions that take a `characterId` parameter and now map it inside (`draw_minimap_character`, `func_8004FDB4`, `func_800A32B4`, `func_800A3ADC`).
- Interpolation tags (`& 0x7`), debug prints, freecam.

### Will need work in SLICE-9 (more than 8)
- `gGPPointsByCharacterId[8]` → `N64KR_MAX_CHARACTERS`.
- Time trial records pack the character in 3 bits (`save.c` `populate_time_trial_record`, `(characterId & 7) << 4`).
- MK64 uses 8 as a "no character" marker (`menu_items.c:8161`, `render_objects.c:2633`): accessors pass unknown ids through, but a 9th character can't use id 8 without moving that marker.
- Character select grid (8 cells, `gCharacterGridSelections`, `sCharacterGridOrder`) and CPU character choice (`chooseCPUPlayers`).
- Custom per-character assets (sprites, portrait, name) instead of borrowing a stock character's.

## Code
- `src/n64kr/character.{h,c}`: `N64krCharacter { name, voice, sprites, hud, body, ai }`, 8 stock entries, accessors `n64kr_char_voice/sprites/hud/body/ai` (unknown ids pass through).

## Build [VERIFIED: build output]
0 errors, 180 s.

## Pass test
Game identical to before: voices, kart sprites, portraits, minimap icons, names on results, sizes, CPU behaviour, for all 8 characters.

## Crash report: Wario Stadium (user, 2026-11-01)
- [VERIFIED: game log] Access violation 0.3 s after "Setup Race!" on Wario Stadium, in `Fast::Interpreter::ImportTextureI4` (`libultraship/src/fast/interpreter.cpp:878`) via `GfxSpTri1`: an I4 texture with a null data pointer. Two earlier races in the same session (same build) ran fine.
- [VERIFIED: diff audit] All 133 SLICE-8 accessor calls take clean arguments (`player->characterId`, `gPlayers[playerId].characterId`, ...), and every accessor returns its input for ids 0..7, so SLICE-8 is a runtime no-op for the stock roster.
- [HYPOTHESIS] Not caused by SLICE-8. Candidate: Enhancements → General → Disable Culling (enabled recently on advice for tree pop-in), which makes MK64 draw objects it normally skips; one may reference an unloaded I4 texture on Wario Stadium. Test: Wario Stadium with Disable Culling off, then on.
- [VERIFIED: user playtest] Follow-up: with no settings changed, the user raced Wario Stadium and other tracks several times without a crash. **Not reproducible; cause unknown.** Kept open as a watch item: if it recurs, save `build/x64/logs/Spaghetti Kart.log` right after, note the track, mode, characters and whether Disable Culling is on.

## In-game test [VERIFIED: user playtest]
- Several races on several tracks, game behaves as before ("seems good now").
