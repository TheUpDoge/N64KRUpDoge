"""Extract a DKR racer's voice clips from the user's own DKR ROM as WAV files.

Usage (from the repo root):
    python -I tools/n64kr/dkr_voice_to_wav.py "<DKR .z64>" diddy build/x64/mods/n64kr-dkr-diddy

Writes <mod dir>/sound/n64kr/<racer>/<clip>.wav (16-bit mono PCM) and
<racer>_voice.txt (clip list). ROM data: keep under build/, never commit.

Format notes (DKR decomp a5eb080, read from source):
- ASSET_AUDIO (section 39) with its table ASSET_AUDIO_TABLE (38): entry 1 is
  the sound-effect bank file (libultra ALBankFile, include/PR/libaudio.h,
  pointers stored as offsets from the file start), entry 2 its wave data
  (.tbl; ALWaveTable.base is an offset into it), entry 6 the sound table
  (SoundData, 10 bytes per sound id: soundBite u16, volume, minVolume, pitch
  (100 = 1.0), unk, range u16, priority, unk) - src/audio.c audio_init.
- A sound id plays bank 0, instrument 0, sound soundBite - 1
  (sndp_play_with_priority, src/audiosfx.c).
- Waves are N64 VADPCM (9-byte frames, codebook per wave) or raw 16-bit.
  Playback ratio against the 22050 Hz synth output (OUTPUT_RATE, audiomgr.h)
  is pitch / 100 x slideMult, slideMult = cents2ratio(keyBase * 100 - 6000,
  + detune unless the key map's pitch-slide flag 0x20 in keyMax is set):
  DKR reuses the key map as sound parameters (src/audiosfx.c:10-15, 590-597).
  The bank's sampleRate field is not used for playback.
- Voice ids (include/sound_ids.h): POSITIVE1..8 and NEGATIVE1..8 are groups
  of 12 racers (Krunch, Bumper, Tiptup, Conker, Timber, Banjo, Drumstick,
  Pipsy, T.T., Diddy, unused, unused); a few named one-off lines per racer.
"""
import os
import struct
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))  # python -I drops the script dir
import dkr_level_to_gltf as dkr  # noqa: E402

dkr.SECTION_INDEX.update({"ASSET_AUDIO_TABLE": 38, "ASSET_AUDIO": 39})

OUTPUT_RATE = 22050
VOICE_GROUP = ["krunch", "bumper", "tiptup", "conker", "timber", "banjo", "drumstick", "pipsy", "tt", "diddy"]
POSITIVE1, NEGATIVE1, GROUP_STRIDE = 0x0162, 0x01C2, 12
SPECIAL = {  # one-off lines (include/sound_ids.h)
    "diddy": {"laugh": 0x0042, "laugh2": 0x0044, "woah": 0x009C, "select": 0x0090},
}


def u16(b, o):
    return struct.unpack_from(">H", b, o)[0]


def s16(b, o):
    return struct.unpack_from(">h", b, o)[0]


def u32(b, o):
    return struct.unpack_from(">I", b, o)[0]


def s32(b, o):
    return struct.unpack_from(">i", b, o)[0]


def codebook(ctl, off):
    """ALADPCMBook -> per predictor, 8 coefficient vectors of length order + 8."""
    order, npred = s32(ctl, off), s32(ctl, off + 4)
    raw = [s16(ctl, off + 8 + 2 * i) for i in range(order * npred * 8)]
    table = []
    for p in range(npred):
        t = [[0] * (order + 8) for _ in range(8)]
        for j in range(order):
            for k in range(8):
                t[k][j] = raw[p * order * 8 + j * 8 + k]
        for k in range(1, 8):
            t[k][order] = t[k - 1][order - 1]
        t[0][order] = 1 << 11
        for k in range(1, 8):
            for j in range(k, 8):
                t[j][k + order] = t[j - k][order]
        table.append(t)
    return order, table


def inner(length, v1, v2):
    out = sum(v1[j] * v2[j] for j in range(length))
    dout = int(out / 2048)  # C division truncates toward zero...
    if out - dout * 2048 < 0:
        dout -= 1           # ...then floors, as the N64 tools do
    return dout


def vadpcm_decode(data, order, table):
    state = [0] * 16
    out = []
    for f in range(0, len(data) - 8, 9):
        header = data[f]
        scale, pred = 1 << (header >> 4), header & 0xF
        ix = []
        for b in data[f + 1:f + 9]:
            for nib in (b >> 4, b & 0xF):
                ix.append((nib - 16 if nib >= 8 else nib) * scale)
        coef = table[min(pred, len(table) - 1)]
        for half in range(2):
            vec = [0] * (order + 8)
            for i in range(order):
                vec[i] = state[(16 if half == 0 else 8) - order + i]
            for i in range(8):
                ind = half * 8 + i
                vec[order + i] = ix[ind]
                state[ind] = max(-32768, min(32767, inner(order + i, coef[i], vec) + ix[ind]))
        out += state
    return out


def wav_bytes(rate, samples):
    pcm = struct.pack("<%dh" % len(samples), *samples)
    return (b"RIFF" + struct.pack("<I", 36 + len(pcm)) + b"WAVEfmt " +
            struct.pack("<IHHIIHH", 16, 1, 1, rate, rate * 2, 2, 16) + b"data" + struct.pack("<I", len(pcm)) + pcm)


def extract(rom, racer, out_root):
    if racer not in VOICE_GROUP:
        sys.exit(f"Unknown racer '{racer}'. Known: {', '.join(VOICE_GROUP)}")
    audio, table = rom.section("ASSET_AUDIO"), rom.table("ASSET_AUDIO_TABLE")
    ctl, tbl = audio[table[1]:table[2]], audio[table[2]:table[3]]
    sounds = audio[table[6]:table[7]]
    bank = u32(ctl, 4)                      # bankArray[0]
    rate = s32(ctl, bank + 4)
    inst = u32(ctl, bank + 12)              # instArray[0]
    sound_count = s16(ctl, inst + 14)

    clips = {}
    k = VOICE_GROUP.index(racer)
    for n in range(8):
        clips[f"positive{n + 1}"] = POSITIVE1 + k + n * GROUP_STRIDE
        clips[f"negative{n + 1}"] = NEGATIVE1 + k + n * GROUP_STRIDE
    clips.update(SPECIAL.get(racer, {}))

    out_dir = os.path.join(out_root, "sound", "n64kr", racer)
    os.makedirs(out_dir, exist_ok=True)
    lines = []
    for clip, sound_id in clips.items():
        bite, volume, pitch = u16(sounds, sound_id * 10), sounds[sound_id * 10 + 2], sounds[sound_id * 10 + 4]
        if not (1 <= bite <= sound_count):
            print(f"  {clip}: sound id 0x{sound_id:03X} has no bank sound ({bite}), skipped")
            continue
        snd = u32(ctl, inst + 16 + 4 * (bite - 1))
        keymap = u32(ctl, snd + 4)
        key_max, key_base, detune = ctl[keymap + 3], ctl[keymap + 4], struct.unpack_from(">b", ctl, keymap + 5)[0]
        next_sound = ctl[keymap] + (ctl[keymap + 2] & 0xC0) * 4
        cents = key_base * 100 - 6000 + (0 if key_max & 0x20 else detune)
        wave = u32(ctl, snd + 8)
        base, length, kind = u32(ctl, wave), s32(ctl, wave + 4), ctl[wave + 8]
        data = tbl[base:base + length]
        if kind == 0:  # AL_ADPCM_WAVE
            order, coefs = codebook(ctl, u32(ctl, wave + 16))
            pcm = vadpcm_decode(data, order, coefs)
        else:          # AL_RAW16_WAVE
            pcm = [s16(data, i) for i in range(0, len(data) - 1, 2)]
        play_rate = max(1, round(OUTPUT_RATE * (pitch or 100) / 100 * 2 ** (cents / 1200)))
        with open(os.path.join(out_dir, f"{clip}.wav"), "wb") as fh:
            fh.write(wav_bytes(play_rate, pcm))
        lines.append(f"{clip} 0x{sound_id:03X} bite {bite} {len(pcm) / play_rate:.2f}s {play_rate} Hz "
                     f"(keyBase {key_base} detune {detune} pitch {pitch}) vol {volume} next {next_sound}")
    with open(os.path.join(out_dir, f"{racer}_voice.txt"), "w") as fh:
        fh.write("\n".join(lines) + "\n")
    print(f"Voice '{racer}': {len(lines)} clips in {out_dir} (bank header rate {rate} Hz, unused)")
    print("\n".join("  " + line for line in lines))


def main():
    if len(sys.argv) != 4:
        sys.exit(__doc__)
    extract(dkr.Rom(sys.argv[1]), sys.argv[2], sys.argv[3])


if __name__ == "__main__":
    main()
