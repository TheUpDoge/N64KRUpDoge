//! libultra audio, rendered offline: sound banks (.ctl + .tbl), VADPCM and
//! raw samples, compact MIDI sequences (ALCSeq) and a renderer that follows
//! the compact sequence player (ALCSPlayer). GoldenEye uses this library
//! (decomp src/libultra/audio: bnkf.c, cseq.c, csplayer.c, seqplayer.c);
//! Diddy Kong Racing does too.
//!
//! Bank file (big-endian, offsets from the start of the .ctl):
//! - ALBankFile: s16 revision (0x4231 "B1"), s16 bank count, s32 bank offsets.
//! - ALBank: s16 instrument count, u8 flags, pad, s32 sample rate, s32
//!   percussion*, s32 instrument offsets.
//! - ALInstrument: u8 volume, pan, priority, flags, tremolo type/rate/depth/
//!   delay, vibrato type/rate/depth/delay, s16 bend range (cents), s16 sound
//!   count, s32 sound offsets.
//! - ALSound: s32 envelope*, keymap*, wavetable*, u8 sample pan, sample volume, flags.
//! - ALEnvelope: s32 attack, decay, release time (microseconds), u8 attack volume, decay volume.
//! - ALKeyMap: u8 velocity min/max, key min/max, key base, s8 detune (cents).
//! - ALWaveTable: s32 base (offset in the .tbl), s32 length, u8 type (0 VADPCM,
//!   1 raw 16-bit), u8 flags, pad, then s32 loop*, s32 book* (VADPCM) or s32 loop* (raw).
//! - Loops: u32 start, end, count (-1 forever); VADPCM loops add 16 s16 of decoder state.
//! - ALADPCMBook: s32 order, s32 predictor count, s16 coefficients[order * predictors * 8].

use std::collections::HashMap;

use crate::rom::{s16_be, u16_be, u32_be};

#[derive(Clone, Copy)]
pub struct Envelope {
    pub attack_time: i32,
    pub decay_time: i32,
    pub release_time: i32,
    pub attack_volume: u8,
    pub decay_volume: u8,
}

#[derive(Clone, Copy)]
pub struct KeyMap {
    pub velocity_min: u8,
    pub velocity_max: u8,
    pub key_min: u8,
    pub key_max: u8,
    pub key_base: u8,
    pub detune: i8,
}

pub struct Wave {
    pub pcm: Vec<i16>,
    /// (start, end, count; count < 0 loops forever)
    pub looping: Option<(usize, usize, i32)>,
}

#[derive(Clone, Copy)]
pub struct Sound {
    pub envelope: Envelope,
    pub keymap: KeyMap,
    pub wave: usize,
    pub pan: u8,
    pub volume: u8,
}

pub struct Instrument {
    pub volume: u8,
    pub pan: u8,
    pub bend_range: i16,
    pub sounds: Vec<Sound>,
}

pub struct Bank {
    pub sample_rate: u32,
    pub instruments: Vec<Option<Instrument>>,
    pub percussion: Option<Instrument>,
    pub waves: Vec<Wave>,
}

fn s32_be(b: &[u8], o: usize) -> i32 {
    u32_be(b, o) as i32
}

fn check(b: &[u8], o: usize, len: usize, what: &str) -> Result<(), String> {
    if o + len > b.len() {
        return Err(format!("{what} at {o:#x} past the end of the bank file"));
    }
    Ok(())
}

/// Decodes VADPCM (9-byte frames: header = scale << 4 | predictor, 16 nibbles).
pub fn vadpcm_decode(data: &[u8], order: usize, book: &[i16]) -> Vec<i16> {
    let npred = if order == 0 { 0 } else { book.len() / (order * 8) };
    let mut out = Vec::with_capacity(data.len() / 9 * 16);
    let mut last = [0i32; 8];
    for frame in data.chunks_exact(9) {
        let scale = 1i32 << (frame[0] >> 4);
        let pred = ((frame[0] & 0xF) as usize).min(npred.saturating_sub(1));
        let coef = |o: usize, k: usize| -> i32 { book.get(pred * order * 8 + o * 8 + k).copied().unwrap_or(0) as i32 };
        let mut ins = [0i32; 16];
        for (i, b) in frame[1..].iter().enumerate() {
            for (j, nib) in [b >> 4, b & 0xF].into_iter().enumerate() {
                ins[i * 2 + j] = (((nib as i32) << 28) >> 28) * scale;
            }
        }
        for half in 0..2 {
            let x = &ins[half * 8..half * 8 + 8];
            let mut cur = [0i32; 8];
            for k in 0..8 {
                // Prediction from the previous `order` outputs plus the residuals so far.
                let mut acc: i64 = 0;
                for o in 0..order {
                    acc += coef(o, k) as i64 * last[8 - order + o] as i64;
                }
                for l in 0..k {
                    acc += coef(order - 1, k - l - 1) as i64 * x[l] as i64;
                }
                acc += (x[k] as i64) << 11;
                cur[k] = ((acc >> 11) as i32).clamp(-32768, 32767);
            }
            last = cur;
            out.extend(cur.iter().map(|&v| v as i16));
        }
    }
    out
}

fn parse_wave(ctl: &[u8], tbl: &[u8], o: usize) -> Result<Wave, String> {
    check(ctl, o, 20, "wavetable")?;
    let (base, len, kind) = (u32_be(ctl, o) as usize, u32_be(ctl, o + 4) as usize, ctl[o + 8]);
    let data = tbl.get(base..base + len).ok_or_else(|| format!("wave data {base:#x}+{len:#x} outside the .tbl"))?;
    let loop_off = u32_be(ctl, o + 12) as usize;
    let looping = if loop_off != 0 {
        check(ctl, loop_off, 12, "loop")?;
        let (start, end, count) = (u32_be(ctl, loop_off) as usize, u32_be(ctl, loop_off + 4) as usize, s32_be(ctl, loop_off + 8));
        (count != 0 && end > start).then_some((start, end, count))
    } else {
        None
    };
    let pcm = if kind == 0 {
        let book_off = u32_be(ctl, o + 16) as usize;
        check(ctl, book_off, 8, "codebook")?;
        let (order, npred) = (s32_be(ctl, book_off) as usize, s32_be(ctl, book_off + 4) as usize);
        check(ctl, book_off + 8, order * npred * 16, "codebook")?;
        let book: Vec<i16> = (0..order * npred * 8).map(|i| s16_be(ctl, book_off + 8 + i * 2)).collect();
        vadpcm_decode(data, order, &book)
    } else {
        data.chunks_exact(2).map(|c| i16::from_be_bytes([c[0], c[1]])).collect()
    };
    Ok(Wave { pcm, looping })
}

/// Parses bank `index` of a .ctl, decoding every wave it uses from the .tbl.
pub fn parse_bank(ctl: &[u8], tbl: &[u8], index: usize) -> Result<Bank, String> {
    check(ctl, 0, 4, "bank file header")?;
    if u16_be(ctl, 0) != 0x4231 {
        return Err(format!("not a libultra bank file (revision {:#06x})", u16_be(ctl, 0)));
    }
    let count = u16_be(ctl, 2) as usize;
    if index >= count {
        return Err(format!("bank {index} of {count}"));
    }
    let b = u32_be(ctl, 4 + index * 4) as usize;
    check(ctl, b, 12, "bank")?;
    let inst_count = u16_be(ctl, b) as usize;
    let sample_rate = u32_be(ctl, b + 4);
    let mut waves: Vec<Wave> = Vec::new();
    let mut wave_at: HashMap<usize, usize> = HashMap::new();
    let mut instrument = |o: usize| -> Result<Instrument, String> {
        check(ctl, o, 16, "instrument")?;
        let n = u16_be(ctl, o + 14) as usize;
        check(ctl, o + 16, n * 4, "instrument sounds")?;
        let mut sounds = Vec::with_capacity(n);
        for k in 0..n {
            let s = u32_be(ctl, o + 16 + k * 4) as usize;
            check(ctl, s, 15, "sound")?;
            let (e, km, w) = (u32_be(ctl, s) as usize, u32_be(ctl, s + 4) as usize, u32_be(ctl, s + 8) as usize);
            check(ctl, e, 14, "envelope")?;
            check(ctl, km, 6, "keymap")?;
            let wave = match wave_at.get(&w) {
                Some(&i) => i,
                None => {
                    waves.push(parse_wave(ctl, tbl, w)?);
                    wave_at.insert(w, waves.len() - 1);
                    waves.len() - 1
                }
            };
            sounds.push(Sound {
                envelope: Envelope {
                    attack_time: s32_be(ctl, e),
                    decay_time: s32_be(ctl, e + 4),
                    release_time: s32_be(ctl, e + 8),
                    attack_volume: ctl[e + 12],
                    decay_volume: ctl[e + 13],
                },
                keymap: KeyMap {
                    velocity_min: ctl[km],
                    velocity_max: ctl[km + 1],
                    key_min: ctl[km + 2],
                    key_max: ctl[km + 3],
                    key_base: ctl[km + 4],
                    detune: ctl[km + 5] as i8,
                },
                wave,
                pan: ctl[s + 12],
                volume: ctl[s + 13],
            });
        }
        Ok(Instrument { volume: ctl[o], pan: ctl[o + 1], bend_range: s16_be(ctl, o + 12), sounds })
    };
    let percussion = match u32_be(ctl, b + 8) as usize {
        0 => None,
        p => Some(instrument(p)?),
    };
    let mut instruments = Vec::with_capacity(inst_count);
    for i in 0..inst_count {
        instruments.push(match u32_be(ctl, b + 12 + i * 4) as usize {
            0 => None,
            o => Some(instrument(o)?),
        });
    }
    Ok(Bank { sample_rate, instruments, percussion, waves })
}

pub fn cents_to_ratio(cents: f64) -> f64 {
    (2.0f64).powf(cents / 1200.0)
}

/// One event of a compact sequence, at an absolute tick.
#[derive(Clone, Copy, Debug)]
pub enum Event {
    /// channel, key, velocity, duration in ticks
    Note(u8, u8, u8, u32),
    Control(u8, u8, u8),
    Program(u8, u8),
    PitchBend(u8, i32),
    /// microseconds per quarter note
    Tempo(u32),
}

/// A compact sequence played out: events by tick, the song loop if every
/// track loops forever back to the same point (events then run through a
/// second pass of the loop, to the tick 2 * end - start), and the end tick.
pub struct Song {
    pub division: u32,
    pub events: Vec<(u64, Event)>,
    /// (loop start tick, loop end tick): ticks after the end repeat from the start.
    pub song_loop: Option<(u64, u64)>,
    pub end: u64,
}

/// Reads a compact MIDI sequence (cseq.c): 16 track offsets + division, then
/// per-track running-status MIDI where note-ons carry a duration, 0xFE
/// back-references repeat earlier bytes, and meta events 0x2E / 0x2D mark
/// loops (0x2D: loop count, current count, 4-byte jump back; count 0xFF = forever).
pub fn parse_song(seq: &[u8]) -> Result<Song, String> {
    if seq.len() < 68 {
        return Err("sequence too short".into());
    }
    let division = u32_be(seq, 64).max(1);
    let mut data = seq.to_vec(); // loop counters are written back into the data, as cseq.c does
    let mut events = Vec::new();
    let mut end = 0u64;
    // (loop start tick, loop end tick) of each track's first loop-forever jump.
    let mut forever: Vec<Option<(u64, u64)>> = Vec::new();
    for t in 0..16 {
        let start = u32_be(seq, t * 4) as usize;
        if start == 0 {
            continue;
        }
        let mut r = TrackReader { pos: start, bu_pos: 0, bu_len: 0 };
        let mut tick = r.var_len(&data)? as u64;
        let mut status = 0u8;
        let mut loop_starts: Vec<u64> = Vec::new();
        let mut track_forever = None;
        let mut guard = 0usize;
        loop {
            guard += 1;
            if guard > 2_000_000 {
                return Err(format!("track {t} runs away"));
            }
            let b = r.byte(&data)?;
            if b == 0xFF {
                let kind = r.byte(&data)?;
                match kind {
                    0x51 => {
                        let v = ((r.byte(&data)? as u32) << 16) | ((r.byte(&data)? as u32) << 8) | r.byte(&data)? as u32;
                        events.push((tick, Event::Tempo(v)));
                    }
                    0x2F => break,
                    0x2E => {
                        r.byte(&data)?;
                        r.byte(&data)?;
                        loop_starts.push(tick);
                    }
                    0x2D => {
                        let p = r.pos;
                        if p + 6 > data.len() {
                            return Err("loop end past the data".into());
                        }
                        let (count, current) = (data[p], data[p + 1]);
                        if current == 0 {
                            data[p + 1] = count;
                            r.pos = p + 6;
                        } else {
                            if current == 0xFF {
                                // Loop forever: note where this track's song loop is, play it
                                // once more (so a renderer can cut out a seamless second pass), then stop.
                                if track_forever.is_some() {
                                    break;
                                }
                                track_forever = Some((loop_starts.last().copied().unwrap_or(0), tick));
                            } else {
                                data[p + 1] = current - 1;
                            }
                            let back = u32_be(&data, p + 2) as usize;
                            r.pos = (p + 6).checked_sub(back).ok_or("loop jumps before the data")?;
                            r.bu_len = 0;
                        }
                    }
                    other => return Err(format!("track {t}: unknown meta event {other:#04x}")),
                }
                status = 0;
            } else {
                let (st, b1) = if b & 0x80 != 0 { (b, r.byte(&data)?) } else { (status, b) };
                if st & 0x80 == 0 {
                    return Err(format!("track {t}: running status without a status byte"));
                }
                status = st;
                let ch = st & 0x0F;
                match st & 0xF0 {
                    0x90 => {
                        let vel = r.byte(&data)?;
                        let dur = r.var_len(&data)?;
                        events.push((tick, Event::Note(ch, b1, vel, dur)));
                    }
                    0x80 | 0xA0 => {
                        r.byte(&data)?;
                    }
                    0xB0 => {
                        let v = r.byte(&data)?;
                        events.push((tick, Event::Control(ch, b1, v)));
                    }
                    0xC0 => events.push((tick, Event::Program(ch, b1))),
                    0xD0 => {}
                    0xE0 => {
                        let b2 = r.byte(&data)?;
                        events.push((tick, Event::PitchBend(ch, ((b2 as i32) << 7) + b1 as i32 - 8192)));
                    }
                    _ => return Err(format!("track {t}: status {st:#04x}")),
                }
            }
            tick += r.var_len(&data)? as u64;
        }
        end = end.max(tick);
        forever.push(track_forever);
    }
    events.sort_by_key(|e| e.0);
    // The song loops if every track that loops forever shares start and end.
    let loops: Vec<(u64, u64)> = forever.iter().flatten().copied().collect();
    let song_loop = match loops.first() {
        Some(&first) if loops.iter().all(|&l| l == first) && loops.len() == forever.len() => Some(first),
        _ => None,
    };
    Ok(Song { division, events, song_loop, end })
}

struct TrackReader {
    pos: usize,
    bu_pos: usize,
    bu_len: usize,
}

impl TrackReader {
    /// __getTrackByte: 0xFE hi lo len = repeat `len` bytes from (hi lo + 4) back; 0xFE 0xFE = 0xFE.
    fn byte(&mut self, d: &[u8]) -> Result<u8, String> {
        let get = |i: usize| d.get(i).copied().ok_or_else(|| "sequence data ended".to_string());
        if self.bu_len > 0 {
            let b = get(self.bu_pos)?;
            self.bu_pos += 1;
            self.bu_len -= 1;
            return Ok(b);
        }
        let b = get(self.pos)?;
        self.pos += 1;
        if b != 0xFE {
            return Ok(b);
        }
        let next = get(self.pos)?;
        self.pos += 1;
        if next == 0xFE {
            return Ok(0xFE);
        }
        let back = ((next as usize) << 8) | get(self.pos)? as usize;
        let len = get(self.pos + 1)? as usize;
        self.pos += 2;
        self.bu_pos = self.pos.checked_sub(back + 4).ok_or("back-reference before the data")?;
        self.bu_len = len;
        let b = get(self.bu_pos)?;
        self.bu_pos += 1;
        self.bu_len = self.bu_len.saturating_sub(1);
        Ok(b)
    }

    fn var_len(&mut self, d: &[u8]) -> Result<u32, String> {
        let mut v = self.byte(d)? as u32;
        if v & 0x80 != 0 {
            v &= 0x7F;
            loop {
                let c = self.byte(d)? as u32;
                v = (v << 7) + (c & 0x7F);
                if c & 0x80 == 0 {
                    break;
                }
            }
        }
        Ok(v)
    }
}

struct Voice {
    wave: usize,
    pos: f64,
    pitch: f64,
    chan: u8,
    key: u8,
    velocity: u8,
    sound: Sound,
    /// envelope: current gain and a linear ramp to `target` ending at `ramp_end` (seconds)
    gain: f64,
    target: f64,
    ramp_end: f64,
    phase: u8, // 0 attack, 1 decay/hold, 2 release
    decay_at: f64,
    off_at: f64,
    released: bool,
    loops_left: i32,
    done: bool,
}

#[derive(Clone, Copy)]
struct Channel {
    program: usize,
    volume: u8,
    pan: u8,
    bend_range: i16,
    bend: f64,
    sustain: bool,
}

/// Renders a song with a bank at the bank's sample rate, interleaved stereo.
/// `until` is the tick to stop at; voices still sounding then are cut.
/// Returns the samples and (tick, sample frame) marks at each event tick.
pub fn render(song: &Song, bank: &Bank, until: u64, seq_volume: f64) -> (Vec<f32>, Vec<(u64, usize)>) {
    let rate = bank.sample_rate.max(8000) as f64;
    let mut out: Vec<f32> = Vec::new();
    let mut voices: Vec<Voice> = Vec::new();
    let first_inst = bank.instruments.iter().position(|i| i.is_some()).unwrap_or(0);
    let mut chans = [Channel { program: first_inst, volume: 127, pan: 64, bend_range: 200, bend: 1.0, sustain: false }; 16];
    for c in chans.iter_mut() {
        if let Some(Some(inst)) = bank.instruments.get(c.program) {
            c.volume = inst.volume;
            c.pan = inst.pan;
            c.bend_range = inst.bend_range;
        }
    }
    let mut uspt = 500000.0 / song.division as f64; // microseconds per tick
    let mut time = 0.0f64; // seconds
    let mut tick = 0u64;
    let mut marks: Vec<(u64, usize)> = vec![(0, 0)];
    // Note-offs by tick: (tick, channel, key)
    let mut offs: Vec<(u64, u8, u8)> = Vec::new();
    let mut ev = song.events.iter().peekable();

    let instrument_for = |chans: &[Channel; 16], ch: u8| -> Option<&Instrument> {
        if ch == 9 && bank.percussion.is_some() && chans[9].program == first_inst {
            return bank.percussion.as_ref();
        }
        bank.instruments.get(chans[ch as usize].program).and_then(|i| i.as_ref())
    };

    loop {
        // Next thing to happen: an event or a note-off, whichever tick is first.
        let next_ev = ev.peek().map(|e| e.0);
        let next_off = offs.iter().map(|o| o.0).min();
        let next = match (next_ev, next_off) {
            (Some(a), Some(b)) => a.min(b),
            (Some(a), None) => a,
            (None, Some(b)) => b,
            (None, None) => until,
        }
        .min(until);
        // Mix up to that tick.
        let t_next = time + (next - tick) as f64 * uspt / 1e6;
        let n = ((t_next * rate) as usize).saturating_sub(out.len() / 2);
        mix(&mut out, n, &mut voices, bank, &chans, rate, seq_volume);
        time = t_next;
        tick = next;
        if tick >= until {
            break;
        }
        // Note-offs due now.
        let due: Vec<(u64, u8, u8)> = offs.iter().filter(|o| o.0 <= tick).copied().collect();
        offs.retain(|o| o.0 > tick);
        for (_, ch, key) in due {
            for v in voices.iter_mut().filter(|v| v.chan == ch && v.key == key && !v.released && v.off_at <= 0.0) {
                v.off_at = time;
                if !chans[ch as usize].sustain {
                    release(v, time);
                }
                break;
            }
        }
        while let Some(&&(t, e)) = ev.peek() {
            if t > tick {
                break;
            }
            ev.next();
            match e {
                Event::Tempo(us) => uspt = us as f64 / song.division as f64,
                Event::Program(ch, p) => {
                    if let Some(Some(inst)) = bank.instruments.get(p as usize) {
                        let c = &mut chans[ch as usize];
                        c.program = p as usize;
                        c.volume = inst.volume;
                        c.pan = inst.pan;
                        c.bend_range = inst.bend_range;
                    }
                }
                Event::Control(ch, ctl, v) => {
                    let c = &mut chans[ch as usize];
                    match ctl {
                        7 => c.volume = v,
                        10 => c.pan = v,
                        64 => {
                            c.sustain = v > 64;
                            if !c.sustain {
                                for voice in voices.iter_mut().filter(|x| x.chan == ch && x.off_at > 0.0 && !x.released) {
                                    release(voice, time);
                                }
                            }
                        }
                        _ => {}
                    }
                }
                Event::PitchBend(ch, b) => {
                    let c = &mut chans[ch as usize];
                    c.bend = cents_to_ratio((c.bend_range as f64 * b as f64 / 8192.0).trunc());
                }
                Event::Note(ch, key, vel, dur) => {
                    let Some(inst) = instrument_for(&chans, ch) else { continue };
                    // __lookupSoundQuick's binary search, read as a linear scan.
                    let Some(sound) = inst.sounds.iter().find(|s| {
                        let k = s.keymap;
                        key >= k.key_min && key <= k.key_max && vel >= k.velocity_min && vel <= k.velocity_max
                    }) else {
                        continue;
                    };
                    let cents = (key as i32 - sound.keymap.key_base as i32) * 100 + sound.keymap.detune as i32;
                    let env = sound.envelope;
                    let attack = env.attack_time.max(0) as f64 / 1e6;
                    voices.push(Voice {
                        wave: sound.wave,
                        pos: 0.0,
                        pitch: cents_to_ratio(cents as f64),
                        chan: ch,
                        key,
                        velocity: vel,
                        sound: *sound,
                        gain: 0.0,
                        target: env.attack_volume as f64,
                        ramp_end: time + attack,
                        phase: 0,
                        decay_at: time + attack,
                        off_at: 0.0,
                        released: false,
                        loops_left: bank.waves[sound.wave].looping.map_or(0, |l| l.2),
                        done: false,
                    });
                    if dur > 0 {
                        offs.push((tick + dur as u64, ch, key));
                    }
                }
            }
        }
        voices.retain(|v| !v.done);
        if marks.last().map_or(true, |m| m.0 != tick) {
            marks.push((tick, out.len() / 2));
        }
    }
    marks.push((tick, out.len() / 2));
    (out, marks)
}

fn release(v: &mut Voice, now: f64) {
    v.released = true;
    v.phase = 2;
    v.target = 0.0;
    v.ramp_end = now + v.sound.envelope.release_time.max(0) as f64 / 1e6;
}

fn mix(out: &mut Vec<f32>, frames: usize, voices: &mut [Voice], bank: &Bank, chans: &[Channel; 16], rate: f64, seq_volume: f64) {
    let start = out.len();
    out.resize(start + frames * 2, 0.0);
    let t0 = start as f64 / 2.0 / rate;
    for v in voices.iter_mut() {
        if v.done {
            continue;
        }
        let wave = &bank.waves[v.wave];
        let c = &chans[v.chan as usize];
        let pan = (c.pan as i32 - 64 + v.sound.pan as i32).clamp(0, 127) as f64 / 127.0;
        let (pl, pr) = ((pan * std::f64::consts::FRAC_PI_2).cos(), (pan * std::f64::consts::FRAC_PI_2).sin());
        let level = (v.velocity as f64 / 127.0) * (v.sound.volume as f64 / 127.0) * (c.volume as f64 / 127.0) * seq_volume;
        let step = v.pitch * c.bend;
        let mut t = t0;
        let dt = 1.0 / rate;
        for f in 0..frames {
            // Envelope: linear ramps (alSynSetVol) attack -> decay -> hold, release to 0.
            if v.phase == 0 && t >= v.decay_at {
                v.phase = 1;
                v.gain = v.target;
                let d = v.sound.envelope.decay_time;
                v.target = v.sound.envelope.decay_volume as f64;
                v.ramp_end = if d < 0 { f64::MAX } else { t + d as f64 / 1e6 };
                if d < 0 {
                    v.gain = v.target;
                }
            }
            if t >= v.ramp_end {
                v.gain = v.target;
                if v.phase == 2 {
                    v.done = true;
                    break;
                }
            } else {
                let left = (v.ramp_end - t).max(dt);
                v.gain += (v.target - v.gain) * (dt / left).min(1.0);
            }
            // Sample (linear interpolation), with the wave's loop.
            if let Some((ls, le, _)) = wave.looping.filter(|_| v.loops_left != 0) {
                if v.pos as usize >= le {
                    v.pos -= (le - ls) as f64;
                    if v.loops_left > 0 {
                        v.loops_left -= 1;
                    }
                }
            }
            let i = v.pos as usize;
            if i + 1 >= wave.pcm.len() {
                v.done = true;
                break;
            }
            let frac = v.pos - i as f64;
            let s = wave.pcm[i] as f64 * (1.0 - frac) + wave.pcm[i + 1] as f64 * frac;
            let a = s / 32768.0 * level * (v.gain / 127.0);
            out[start + f * 2] += (a * pl) as f32;
            out[start + f * 2 + 1] += (a * pr) as f32;
            v.pos += step;
            t += dt;
        }
    }
}

/// 16-bit stereo WAV.
pub fn wav(rate: u32, samples: &[f32], gain: f32) -> Vec<u8> {
    let mut pcm = Vec::with_capacity(samples.len() * 2);
    for &s in samples {
        pcm.extend_from_slice(&((s * gain * 32767.0).clamp(-32768.0, 32767.0) as i16).to_le_bytes());
    }
    let mut w = Vec::with_capacity(44 + pcm.len());
    w.extend_from_slice(b"RIFF");
    w.extend_from_slice(&(36 + pcm.len() as u32).to_le_bytes());
    w.extend_from_slice(b"WAVEfmt ");
    w.extend_from_slice(&16u32.to_le_bytes());
    w.extend_from_slice(&1u16.to_le_bytes());
    w.extend_from_slice(&2u16.to_le_bytes());
    w.extend_from_slice(&rate.to_le_bytes());
    w.extend_from_slice(&(rate * 4).to_le_bytes());
    w.extend_from_slice(&4u16.to_le_bytes());
    w.extend_from_slice(&16u16.to_le_bytes());
    w.extend_from_slice(b"data");
    w.extend_from_slice(&(pcm.len() as u32).to_le_bytes());
    w.extend_from_slice(&pcm);
    w
}

/// Mono 16-bit WAV.
pub fn wav_mono(rate: u32, samples: &[i16]) -> Vec<u8> {
    let mut w = Vec::with_capacity(44 + samples.len() * 2);
    w.extend_from_slice(b"RIFF");
    w.extend_from_slice(&(36 + samples.len() as u32 * 2).to_le_bytes());
    w.extend_from_slice(b"WAVEfmt ");
    w.extend_from_slice(&16u32.to_le_bytes());
    w.extend_from_slice(&1u16.to_le_bytes());
    w.extend_from_slice(&1u16.to_le_bytes());
    w.extend_from_slice(&rate.to_le_bytes());
    w.extend_from_slice(&(rate * 2).to_le_bytes());
    w.extend_from_slice(&2u16.to_le_bytes());
    w.extend_from_slice(&16u16.to_le_bytes());
    w.extend_from_slice(b"data");
    w.extend_from_slice(&(samples.len() as u32 * 2).to_le_bytes());
    for s in samples {
        w.extend_from_slice(&s.to_le_bytes());
    }
    w
}
