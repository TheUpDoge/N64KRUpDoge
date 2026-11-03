//! A DKR racer's voice clips as WAV (`n64kr dkr-voice`), played by
//! src/n64kr/voice.cpp. Writes <mod dir>/sound/n64kr/<racer>/<clip>.wav
//! (16-bit mono PCM) and <racer>_voice.txt (clip list).
//!
//! Format (DKR decomp a5eb080):
//! - ASSET_AUDIO (section 39) with its table (38): entry 1 is the sound-effect
//!   bank (libultra ALBankFile, offsets from the file start), entry 2 its wave
//!   data (.tbl), entry 6 the sound table (10 bytes per sound id: soundBite
//!   u16, volume, minVolume, pitch (100 = 1.0), ...) - src/audio.c audio_init.
//! - A sound id plays bank 0, instrument 0, sound soundBite - 1
//!   (sndp_play_with_priority, src/audiosfx.c).
//! - Waves are N64 VADPCM (9-byte frames, codebook per wave) or raw 16-bit.
//!   Playback ratio against the 22050 Hz output is pitch / 100 x
//!   cents2ratio(keyBase x 100 - 6000, + detune unless the key map's flag
//!   0x20 in keyMax is set): DKR reuses the key map as sound parameters.
//! - Voice ids (include/sound_ids.h): POSITIVE1..8 and NEGATIVE1..8 are groups
//!   of 12 racers in Character enum order; a few one-off lines per racer.

use std::collections::HashSet;
use std::path::Path;

use super::{s16, s32, s8, u16, u32, Rom, AUDIO, AUDIO_TABLE};
use crate::n64audio;

const OUTPUT_RATE: i64 = 22050;
const VOICE_GROUP: [&str; 10] = ["krunch", "bumper", "tiptup", "conker", "timber", "banjo", "drumstick", "pipsy", "tt", "diddy"];
const POSITIVE1: usize = 0x0162;
const NEGATIVE1: usize = 0x01C2;
const GROUP_STRIDE: usize = 12;

/// One-off lines (include/sound_ids.h): laugh and laugh2 on a win, woah on a
/// fall, select when picked.
fn special(racer: &str) -> &'static [(&'static str, usize)] {
    match racer {
        "diddy" => &[("laugh", 0x0042), ("laugh2", 0x0044), ("woah", 0x009C), ("select", 0x0090)],
        "banjo" => &[("laugh", 0x0043), ("laugh2", 0x0078), ("woah", 0x0098), ("select", 0x008C)], // GUHHUH, YAHOO, WOAH, "I'm Banjo"
        "conker" => &[("laugh", 0x002E), ("woah", 0x0096), ("select", 0x008A)], // YEHAHA, AHH, "I'm Conker"
        _ => &[],
    }
}

/// libultra ALADPCMBook -> per predictor, 8 coefficient rows of length order + 8.
fn codebook(ctl: &[u8], off: usize) -> (usize, Vec<Vec<Vec<i64>>>) {
    let (order, npred) = (s32(ctl, off) as usize, s32(ctl, off + 4) as usize);
    let raw: Vec<i64> = (0..order * npred * 8).map(|i| s16(ctl, off + 8 + 2 * i) as i64).collect();
    let mut table = vec![];
    for p in 0..npred {
        let mut t = vec![vec![0i64; order + 8]; 8];
        for j in 0..order {
            for k in 0..8 {
                t[k][j] = raw[p * order * 8 + j * 8 + k];
            }
        }
        for k in 1..8 {
            t[k][order] = t[k - 1][order - 1];
        }
        t[0][order] = 1 << 11;
        for k in 1..8 {
            for j in k..8 {
                t[j][k + order] = t[j - k][order];
            }
        }
        table.push(t);
    }
    (order, table)
}

/// Dot product scaled down by 2048, rounded down (as the N64 tools do).
fn inner(length: usize, v1: &[i64], v2: &[i64]) -> i64 {
    (0..length).map(|j| v1[j] * v2[j]).sum::<i64>().div_euclid(2048)
}

fn vadpcm_decode(data: &[u8], order: usize, table: &[Vec<Vec<i64>>]) -> Vec<i16> {
    let mut state = [0i64; 16];
    let mut out = vec![];
    let mut f = 0;
    while f + 8 < data.len() {
        let header = data[f];
        let (scale, pred) = (1i64 << (header >> 4), (header & 0xF) as usize);
        let mut ix = [0i64; 16];
        for (i, b) in data[f + 1..f + 9].iter().enumerate() {
            for (h, nib) in [(b >> 4) as i64, (b & 0xF) as i64].into_iter().enumerate() {
                ix[i * 2 + h] = (if nib >= 8 { nib - 16 } else { nib }) * scale;
            }
        }
        let coef = &table[pred.min(table.len() - 1)];
        for half in 0..2 {
            let mut vec = vec![0i64; order + 8];
            for i in 0..order {
                vec[i] = state[(if half == 0 { 16 } else { 8 }) - order + i];
            }
            for i in 0..8 {
                let ind = half * 8 + i;
                vec[order + i] = ix[ind];
                state[ind] = (inner(order + i, &coef[i], &vec) + ix[ind]).clamp(-32768, 32767);
            }
        }
        out.extend(state.iter().map(|&s| s as i16));
        f += 9;
    }
    out
}

/// Clips to leave out: Banjo and Conker have fewer recordings than DKR's
/// 8 positive + 8 negative slots, and DKR fills the lists with repeats, some
/// across lists (Banjo's positive2 is his "woah", positive5 = negative4;
/// Conker's positive7 = negative2). N64KR plays the positive list when a
/// racer hits someone, so Banjo answered a shell hit with "woah" (user
/// playtest). Keeps each recording once per list, a negative one out of the
/// positive list (unless also a laugh line) and a laugh out of the negative list.
fn crossed_clips(bites: &[(String, u16)]) -> HashSet<String> {
    let get = |c: &str| bites.iter().find(|(n, _)| n == c).map(|(_, b)| *b);
    let cheer: HashSet<u16> = ["laugh", "laugh2"].iter().filter_map(|c| get(c)).collect();
    let negative: HashSet<u16> = bites.iter().filter(|(c, _)| c.starts_with("negative") || c == "woah").map(|(_, b)| *b).collect();
    let (mut drop, mut seen) = (HashSet::new(), HashSet::new());
    for (clip, bite) in bites {
        let group = &clip[..clip.len().min(8)];
        if group != "positive" && group != "negative" {
            continue;
        }
        if (group == "positive" && negative.contains(bite) && !cheer.contains(bite))
            || (group == "negative" && cheer.contains(bite))
            || seen.contains(&(group.to_string(), *bite))
        {
            drop.insert(clip.clone());
        }
        seen.insert((group.to_string(), *bite));
    }
    drop
}

/// `n64kr dkr-voice <rom> <racer> <mod dir>`
pub fn extract(rom: &Rom, racer: &str, out_root: &str) -> Result<(), String> {
    let k = VOICE_GROUP.iter().position(|g| *g == racer).ok_or_else(|| format!("unknown racer '{racer}'; known: {}", VOICE_GROUP.join(", ")))?;
    let (audio, table) = (rom.section(AUDIO), rom.table(AUDIO_TABLE));
    let (ctl, tbl, sounds) = (rom.entry(audio, &table, 1), rom.entry(audio, &table, 2), rom.entry(audio, &table, 6));
    let bank = u32(ctl, 4) as usize; // bankArray[0]
    let rate = s32(ctl, bank + 4);
    let inst = u32(ctl, bank + 12) as usize; // instArray[0]
    let sound_count = s16(ctl, inst + 14) as i64;

    let mut clips: Vec<(String, usize)> = vec![];
    for n in 0..8 {
        clips.push((format!("positive{}", n + 1), POSITIVE1 + k + n * GROUP_STRIDE));
        clips.push((format!("negative{}", n + 1), NEGATIVE1 + k + n * GROUP_STRIDE));
    }
    for &(name, id) in special(racer) {
        clips.push((name.to_string(), id));
    }

    let dir = Path::new(out_root).join("sound").join("n64kr").join(racer);
    std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let bites: Vec<(String, u16)> = clips.iter().map(|(c, id)| (c.clone(), u16(sounds, id * 10))).collect();
    let valid: Vec<(String, u16)> = bites.iter().filter(|(_, b)| (1..=sound_count).contains(&(*b as i64))).cloned().collect();
    let drop = crossed_clips(&valid);
    let mut lines = vec![];
    for ((clip, sound_id), (_, bite)) in clips.iter().zip(&bites) {
        let (volume, pitch) = (sounds[sound_id * 10 + 2], sounds[sound_id * 10 + 4]);
        if !(1..=sound_count).contains(&(*bite as i64)) {
            println!("  {clip}: sound id 0x{sound_id:03X} has no bank sound ({bite}), skipped");
            continue;
        }
        if drop.contains(clip) {
            let stale = dir.join(format!("{clip}.wav"));
            if stale.exists() {
                std::fs::remove_file(&stale).map_err(|e| e.to_string())?;
            }
            lines.push(format!("{clip} 0x{sound_id:03X} bite {bite} left out (repeat or crossed list)"));
            continue;
        }
        let snd = u32(ctl, inst + 16 + 4 * (*bite as usize - 1)) as usize;
        let keymap = u32(ctl, snd + 4) as usize;
        let (key_max, key_base, detune) = (ctl[keymap + 3], ctl[keymap + 4] as i64, s8(ctl, keymap + 5) as i64);
        let next_sound = ctl[keymap] as i64 + (ctl[keymap + 2] & 0xC0) as i64 * 4;
        let cents = key_base * 100 - 6000 + if key_max & 0x20 != 0 { 0 } else { detune };
        let wave = u32(ctl, snd + 8) as usize;
        let (base, length, kind) = (u32(ctl, wave) as usize, s32(ctl, wave + 4).max(0) as usize, ctl[wave + 8]);
        let data = &tbl[base..(base + length).min(tbl.len())];
        let pcm: Vec<i16> = if kind == 0 {
            // AL_ADPCM_WAVE
            let (order, coefs) = codebook(ctl, u32(ctl, wave + 16) as usize);
            vadpcm_decode(data, order, &coefs)
        } else {
            // AL_RAW16_WAVE
            (0..data.len().saturating_sub(1)).step_by(2).map(|i| s16(data, i)).collect()
        };
        let pitch_or = if pitch == 0 { 100 } else { pitch as i64 };
        let play_rate = super::round_half_even((OUTPUT_RATE * pitch_or) as f64 / 100.0 * 2f64.powf(cents as f64 / 1200.0)).max(1);
        std::fs::write(dir.join(format!("{clip}.wav")), n64audio::wav_mono(play_rate as u32, &pcm)).map_err(|e| e.to_string())?;
        lines.push(format!(
            "{clip} 0x{sound_id:03X} bite {bite} {:.2}s {play_rate} Hz (keyBase {key_base} detune {detune} pitch {pitch}) vol {volume} next {next_sound}",
            pcm.len() as f64 / play_rate as f64
        ));
    }
    std::fs::write(dir.join(format!("{racer}_voice.txt")), lines.join("\n") + "\n").map_err(|e| e.to_string())?;
    println!("Voice '{racer}': {} clips in {} (bank header rate {rate} Hz, unused)", lines.len() - drop.len(), dir.display());
    for line in &lines {
        println!("  {line}");
    }
    Ok(())
}
