#ifndef N64KR_MUSIC_H
#define N64KR_MUSIC_H

// N64KR: music from other games, rendered from your ROM to WAV with loop
// points (music/n64kr/<name>.wav + .json, tools/n64kr-rs ge-audio) and played
// by the port's HMAS mixer under sequence ids MK64 never uses. A track picks
// one with its scene.json "Sequence" (e.g. 20032 = GoldenEye's Dam).

#include <common_structs.h>

#define N64KR_MUSIC_GE_DAM 0x4E40
#define N64KR_MUSIC_GE_DAMX 0x4E41
#define N64KR_MUSIC_GE_INTRO 0x4E42
#define N64KR_MUSIC_GE_RUNWAY 0x4E43
#define N64KR_MUSIC_GE_FACILITY 0x4E44

#ifdef __cplusplus
extern "C" {
#endif

// From play_sequence (src/audio/external.c, N64KR HOOK): registers song `id`
// with HMAS on first use, so the HMAS path there plays it. Other ids: no-op.
void n64kr_music_prepare(u16 id);

// Final lap (func_800CA49C, N64KR HOOK): if an N64KR song is playing, switch
// to its action version (GoldenEye's "x" songs) after the fanfare instead of
// MK64's speed-up, and return 1; otherwise 0.
s32 n64kr_music_final_lap(void);

#ifdef __cplusplus
}
#endif

#endif // N64KR_MUSIC_H
