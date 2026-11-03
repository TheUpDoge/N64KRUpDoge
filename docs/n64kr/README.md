# N64KR documentation

N64KR is a fork of [SpaghettiKart](https://github.com/HarbourMasters/SpaghettiKart) (the native PC port of Mario Kart 64) that brings in racers, tracks, physics, music and hazards from **Diddy Kong Racing** and **GoldenEye 007**, generated from ROMs you own. Character, vehicle and handling are separate, so any racer can drive any vehicle on any track.

| Read | For |
|---|---|
| [HOW_IT_WORKS.md](HOW_IT_WORKS.md) | What changed, why, and how each piece was done, plus the bugs that cost the most time and their causes |
| [TUTORIAL_SETUP.md](TUTORIAL_SETUP.md) | Build N64KR and generate all its content from your ROMs |
| [TUTORIAL_ADD_DKR_RACER.md](TUTORIAL_ADD_DKR_RACER.md) | Add another DKR racer (Krunch, Tiptup, Pipsy, ...) |
| [TUTORIAL_ADD_DKR_TRACK.md](TUTORIAL_ADD_DKR_TRACK.md) | Convert another DKR track and put it in a cup |
| [TUTORIAL_GOLDENEYE.md](TUTORIAL_GOLDENEYE.md) | GoldenEye guards, barrels and music on any track; build a new GoldenEye track |
| [../../N64KR.md](../../N64KR.md) | Feature list with test status, tool and module reference |
| [../../recon/](../../recon) | Step-by-step records with decomp file:line evidence |

## Ground rules

- **Your own ROMs only.** The tools read ROMs you own and write into `build/`, which git ignores. Never commit or share ROMs, extracted assets, `.o2r` files or generated mods.
- **Thin hooks.** N64KR code lives in `src/n64kr/`; SpaghettiKart's own files only get one-line hooks marked `// N64KR HOOK`.
- **Evidence labels.** Records mark claims VERIFIED (source, tool output or in-game test), CONCLUSION, or HYPOTHESIS.
