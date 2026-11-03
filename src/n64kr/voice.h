#ifndef N64KR_VOICE_H
#define N64KR_VOICE_H

// N64KR: own voice clips for characters MK64's sound banks don't have.
//
// MK64 voice lines are `voice * 0x10 + SOUND_ARG_LOAD(0x29, 0x00, 0x80, slot)`.
// A character with a registry voiceName gets its own id as voice index
// (n64kr_char_voice_line), so its lines arrive in play_sound with sound ids
// 0x90 and up, which MK64 never uses. Those are played here instead: WAV clips
// from a mod (sound/n64kr/<voiceName>/, `n64kr dkr-voice`),
// mixed by the port's HMAS mixer, quieter with distance from the listener.

#include <common_structs.h>

#ifdef __cplusplus
extern "C" {
#endif

// From play_sound (src/audio/external.c). Returns 1 if the sound was an own
// voice line (played or dropped here), 0 to let MK64 handle it. `position` is
// listener-relative, as MK64's sound requests are; may be NULL.
s32 n64kr_voice_play_sound(u32 soundBits, const f32* position, u8 cameraId);

#ifdef __cplusplus
}
#endif

#endif // N64KR_VOICE_H
