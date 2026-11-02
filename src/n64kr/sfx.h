#ifndef N64KR_SFX_H
#define N64KR_SFX_H

// N64KR: positional sound effects from mods, e.g. GoldenEye's gunfire and
// explosions (sound/n64kr/ge/<name>.wav, tools/n64kr-rs ge-audio).
//
// MK64's sound banks can't play other games' sounds, and HMAS plays one sound
// per channel, which can't layer a burst of gunfire over an explosion. These
// clips get their own voices, mixed into the port's audio thread
// (GameEngine::HandleAudioThread, N64KR HOOK) next to HMAS.

#include <common_structs.h>

#ifdef __cplusplus
extern "C" {
#endif

// Plays `path` (a WAV in the mods, mono or stereo 16-bit) at world position
// `pos` (NULL: not positional), quieter with distance from the nearest camera
// and panned by its direction. `volume` 0..1, `pitch` 1 = as recorded.
void n64kr_sfx_play(const char* path, const f32* pos, f32 volume, f32 pitch);

// Audio thread: adds the playing voices to `stereo` (interleaved floats, -1..1).
void n64kr_sfx_mix(f32* stereo, u32 frames);

void n64kr_sfx_stop_all(void);

#ifdef __cplusplus
}
#endif

#endif // N64KR_SFX_H
