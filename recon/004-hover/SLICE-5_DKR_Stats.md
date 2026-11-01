# SLICE-5 — Real DKR hovercraft stats + boost-ramp fix

Project: N64 Kart Racing (N64KR)
Date: 2026-11-01
Status: WORKS (user playtest); character comparison partly checked
Base: branch `n64kr`, on top of SLICE-4 (`2d7d005`)

## Where the stats live in the DKR ROM [VERIFIED: decomp source + SHA-1 match]
Source: DKR decomp `a5eb080` (`src/asset_loading.c:57-90`, `src/objects.c:752-757, 8197-8202`, `include/structs.h:841`, `tools/dkr_assets_tool_extract.json`).

- User ROM: DKR USA 1.1 (v80), SHA-1 `6d96743d…3755`, matches the decomp's `ver/verification/dkr.us.v80.sha1`.
- Asset lookup table at ROM offset 970976 (`inputs-supported` in the asset JSON): `u32 count` (50), then one `u32` start offset per section, relative to the end of the table (table size 208 bytes for v80).
- Section 15 = misc assets; section 16 = misc table (word offsets into section 15, `-1` terminated).
  - Misc 9 = `ASSET_MISC_RACER_WEIGHT`, misc 10 = `ASSET_MISC_RACER_HANDLING`: 10 floats each, DKR `Character` order (Krunch, Bumper, Tiptup, Conker, Timber, Banjo, Drumstick, Pipsy, T.T., Diddy). Both match the decomp's SHA-1s.
- Section 34 = object headers; section 33 = their offset table. A racer header's byte `0x5C` (`miscAssetIds[0]`) is its acceleration curve; `0x60` is the internal name.
  - Hovercraft headers: `diddyhover` → misc 34, `ticktockhover` → 35, `KremlinHover` → 37, `BadgerHover` → 38, `TortHover` → 39, `ConkaHover` → 40, `TigerHover` → 41, `BanjoHover` → 42, `ChickenHover` → 43, `MouseHover` → 44 (all `[1]` = 51, collision spheres). Curves are 16 floats.

## Pipeline
- `tools/n64kr/extract_dkr_hover_stats.py` (committed, holds no game data): checks the ROM SHA-1 and the weight/handling SHA-1s, finds the hovercraft headers by internal name, writes `build/x64/n64kr_dkr_hover.txt`.
- That file holds ROM data and is git-ignored (`build*/` and `n64kr_dkr_*.txt`).
- `src/n64kr/hover.c` loads it once at first use. Without it, the placeholders from SLICE-3 are used.
- Menu: **Enhancements → N64KR → Hovercraft DKR Stats** (CVar `gN64KR.HoverDkrCharacter`, default 9 = Diddy). Profile renamed **"Hovercraft (DKR)"**.

## How the stats are used
- Speed curve: `xVelTemp = lerp(curve, |forward velocity|)` clamped at 12 (DKR `racer.c:1807-1818`), × 1.7.
- Gravity: `weight · 0.45` (`racer.c:4233`) `· 0.75` (`:1819`), converted to MK64 units.
- Handling: loaded, **not used** yet (steering is still MK64's).
- DKR top speed per character = where `curve · 1.7` balances `0.004 · v²`. The first version searched only up to 12 and every character hit that cap; DKR clamps the curve lookup at 12, so the balance point can sit past it (12.0–13.5). Search now runs to 40.
- Unit scale uses **Diddy's** top speed for every character, so the 150cc calibration from SLICE-3 holds for Diddy and other characters keep DKR's differences [CONCLUSION, from the extracted curves]: T.T. ≈ +12%, Drumstick ≈ +10%, Krunch ≈ +7%, Banjo ≈ +3%, Bumper/Conker/Timber ≈ +1%, Tiptup/Pipsy = Diddy.
- Gravity in MK64 units comes out at 0.13–0.19 (placeholder was 0.3): floatier.

## Boost-ramp fix (bug found in playtest)
- **User playtest:** DK Jungle's acceleration ramp did nothing with the hovercraft.
- Cause [VERIFIED: source]: MK64 ramps work in the stage hover replaces: `func_80030150` returns `boostPower + kartPropulsionStrength` while `BOOST_RAMP_WOOD_EFFECT` / `BOOST_RAMP_ASPHALT_EFFECT` is set (`player_controller.c:2823-2828`), and `func_8002AB70` raises the vertical divisor `unk_DAC` to 20–25 with special `kartGravity` (`:1010-1017`). Ramp boost logic: `apply_boost_ramp_wood_effect` / `apply_boost_ramp_asphalt_effect` (`effects.c:1169-1230`).
- Fix: the hover velocity stage returns early while either ramp effect is set, leaving MK64's `newVelocity` untouched.
- **User playtest after fix: "Ramp works."** [VERIFIED]

## In-game test [VERIFIED: user playtest]
- Real stats loaded and driving: "seems to work".
- DK Jungle ramp with hovercraft: works.

## Open
- [ ] Character differences felt in game (T.T. faster, Krunch heavier).
- [ ] Tree pop-in on DK Jungle. **[HYPOTHESIS]** either MK64's own palm-tree distance culling (`src/actors/palm_tree/render.inc.c`, toggled by Enhancements → General → Disable Culling) or `render_track_sections` switching to `pathCounter` when the player is more than 30 units above the surface (`src/racing/render_courses.c:136`), which a floating hovercraft over the river hits more often. Needs the user's comparison with a normal kart.
- [ ] Only US v80 is supported by the extractor.
