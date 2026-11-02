# Reference: Kaze Emanuar (N64 optimization, SM64)

Channel: https://www.youtube.com/channel/UCuvSqzfO_LV_QzHdmEj84SQ (suggested by the user, 2026-11-01).
Read: the Software Engineering Daily interview transcript (SED1666) and press coverage. Videos themselves were not watched (no audio/video access).

## What his work is [VERIFIED: sources below]
- Rewrote and optimized the Super Mario 64 engine from the decomp to run well on real N64 hardware (60 FPS on hardware), fixing bugs and slow code across the whole source; uses it for his own game, Return to Yoshi's Island.
- Central theme: the N64's CPU and graphics coprocessor share one memory bus, so CPU work slows rendering ("whenever you use the CPU, you also slow down the rendering"); cache-line alignment, keeping working data inside cache, cutting memory traffic (Rambus).
- Graphics: works with the F3DEX3 microcode (Sauraen) and N64-friendly textures/models.
- Approach to decomp code: matching C (byte-identical builds) makes large changes practical; reads and fixes the whole codebase rather than patching around it.

## What it means for N64KR [CONCLUSION]
- N64KR runs on PC through SpaghettiKart/libultraship (display lists interpreted by a PC renderer), so N64 bus/cache/fill-rate limits don't bind us; his hardware optimizations aren't needed for the port itself.
- Directly useful: his knowledge of N64 display lists, microcode, textures and asset pipelines (Fast64), relevant as we convert models and tracks between N64 games, and if N64KR content ever targets real hardware.
- His "read and understand the whole system before changing it" approach matches this project's recon-first rule (CLAUDE.md).

Sources: https://softwareengineeringdaily.com/wp-content/uploads/2024/03/SED1666-Kaze_Emanuar.txt · https://en.wikipedia.org/wiki/Kaze_Emanuar · https://hackaday.com/2022/04/21/ · https://timeextension.com/news/2024/03/super-mario-sunshine-on-n64-looks-better-than-you-might-expect

## GitHub (https://github.com/KazeEmanuar) [VERIFIED: profile page, 2026-11-01]
- SM64-but-some-optimizations-are-removed (C, 58★): a demo of SM64 running better, with some of his optimizations removed (non-functional).
- HackerSM64 (C, fork of HackerN64/HackerSM64): the SM64 decomp base for ROM hacks his community builds on.
- splitscreen-multiplayer (Assembly), Peachfury (C), sm64 (fork of n64decomp/sm64), Chaos3 (C, a decomp version of Chaos Edition).
- [CONCLUSION] None of these are model, track or format tools we can reuse directly; HackerSM64 is the place to study his N64 rendering and optimization practice if N64KR ever targets real hardware.
