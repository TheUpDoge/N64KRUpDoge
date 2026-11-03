//! GoldenEye music and sound effects: where they are in the US ROM.
//!
//! (decomp scripts/filelist.u.csv; src/music.c, src/snd.c.)
//! - Music: instrument bank instruments.ctl/.tbl (bank 0), songs Mxxx.bin,
//!   each a 1172-compressed compact MIDI sequence (alCSeqNew after
//!   decompressdata), played at the track's default volume (g_musicDefaultTrackVolume).
//! - Sound effects: sfx.ctl/.tbl; effect n is instrument 0's sound n
//!   (sndPlaySfx: instArray[0]->soundArray[n]), pitched by
//!   alCents2Ratio(keyBase * 100 + detune - 6000) (sndSetupSound). A sound can
//!   chain another: next = velocityMin + (keyMin & 0xC0) * 4, started
//!   velocityMax * 33 ms later (Rare reuses the keymap fields).

pub const MUSIC_CTL: (usize, usize) = (3884112, 17312);
pub const MUSIC_TBL: (usize, usize) = (3901424, 397216);
pub const SFX_CTL: (usize, usize) = (3063264, 23488);
pub const SFX_TBL: (usize, usize) = (3086752, 797360);

/// Songs: (N64KR name, ROM offset, size, default volume / 0x7FFF).
pub const SONGS: &[(&str, usize, usize, f32)] = &[
    ("dam", 4321096, 3588, 0x6665 as f32 / 32767.0),
    ("damx", 4380980, 1312, 0x6665 as f32 / 32767.0),
    ("intro", 4299660, 2222, 0x7332 as f32 / 32767.0),
    ("runway", 4406354, 3358, 0x6665 as f32 / 32767.0),
    ("facility", 4315420, 2766, 0x6665 as f32 / 32767.0),
];

/// Sound effects: (N64KR name, SFX_ID) (decomp src/bondconstants.h SFX_ID_ToString).
pub const SFX: &[(&str, usize)] = &[
    ("kf7", 109),        // GUN_B4_BOLTACTION_SFX, the AK47 / KF7 Soviet
    ("explosion", 172),  // EXPLOSION_4A_SFX
    ("explosion2", 173), // EXPLOSION_4B_SFX
    ("rico1", 31),       // RICO_4_A_SFX
    ("rico2", 35),       // RICO_5_A_SFX
    ("whiz1", 164),      // RICO_EAR_WHISTLE1_SFX, a near miss
    ("whiz2", 166),      // RICO_EAR_WHISTLE3_SFX
    ("hit_metal", 72),   // HIT_BULLET_METAL_A_SFX
    ("alarm", 161),      // ALARM1_SFX
    ("cart", 122),       // CART_SPENT_SFX, a spent case
    ("rifle_big", 111),  // GUN_RIFLE7BIG_1_SFX, Bond's shot in the gun barrel intro
];

pub const SFX_PITCH_SHIFT_CENTS: i32 = -6000; // DEFAULT_SETUP_PITCH_SHIFT

