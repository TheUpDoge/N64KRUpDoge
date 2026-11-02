//! GoldenEye 007 (N64, US) content. Formats read from the n64decomp/007
//! decompilation (commit c435646).

pub mod bg;

/// SHA-1 of the US ROM the tables below belong to (decomp ge007.u.sha1).
pub const US_SHA1: &str = "abe01e4aeb033b6c0836819f549c791b26cfde83";

/// Level background ("bg") files in the US ROM: (name, ROM offset, size)
/// (decomp scripts/filelist.u.csv). Stored uncompressed; the rooms inside
/// are 1172-compressed.
pub const BG_FILES: &[(&str, usize, usize)] = &[
    ("bg_sev_all_p", 4425312, 69104),
    ("bg_silo_all_p", 4494416, 331584),
    ("bg_stat_all_p", 4826000, 139472),
    ("bg_arec_all_p", 4965472, 189312),
    ("bg_arch_all_p", 5154784, 154352),
    ("bg_tra_all_p", 5309136, 132464),
    ("bg_dest_all_p", 5441600, 186816),
    ("bg_sevb_all_p", 5628416, 109984),
    ("bg_azt_all_p", 5738400, 137808),
    ("bg_pete_all_p", 5876208, 105520),
    ("bg_depo_all_p", 5981728, 182640),
    ("bg_ref_all_p", 6164368, 38416),
    ("bg_cryp_all_p", 6202784, 87728),
    ("bg_dam_all_p", 6290512, 197024),
    ("bg_ark_all_p", 6487536, 200576),
    ("bg_run_all_p", 6688112, 41936),
    ("bg_sevx_all_p", 6730048, 116176),
    ("bg_jun_all_p", 6846224, 86352),
    ("bg_dish_all_p", 6932576, 18544),
    ("bg_cave_all_p", 6951120, 148720),
    ("bg_cat_all_p", 7099840, 21808),
    ("bg_crad_all_p", 7121648, 66384),
    ("bg_ame_all_p", 7188032, 40800),
    ("bg_oat_all_p", 7228832, 28240),
    ("bg_len_all_p", 7257072, 4000),
];

/// Level short names (as players know them) -> bg file (decomp bg.c levelinfotable).
pub const LEVELS: &[(&str, &str, f32)] = &[
    // (name, bg file, levelscale)
    ("dam", "bg_dam_all_p", 0.23363999),
    ("facility", "bg_ark_all_p", 1.20648),
    ("runway", "bg_run_all_p", 0.089571431),
    ("surface", "bg_sevx_all_p", 0.45445713),
    ("bunker", "bg_sev_all_p", 0.53931433),
    ("silo", "bg_silo_all_p", 0.47256002),
    ("frigate", "bg_dest_all_p", 0.44757429),
    ("statue", "bg_stat_all_p", 0.107202865),
    ("archives", "bg_arch_all_p", 0.50678575),
    ("streets", "bg_pete_all_p", 0.34187999),
    ("depot", "bg_depo_all_p", 0.21847887),
    ("train", "bg_tra_all_p", 0.15019713),
    ("jungle", "bg_jun_all_p", 0.094662853),
    ("control", "bg_arec_all_p", 0.49886572),
    ("caverns", "bg_cave_all_p", 0.26824287),
    ("cradle", "bg_crad_all_p", 0.23571429),
    ("aztec", "bg_azt_all_p", 0.35300568),
    ("egypt", "bg_cryp_all_p", 0.25608),
];

pub fn bg_file(name: &str) -> Option<(usize, usize)> {
    BG_FILES.iter().find(|(n, _, _)| *n == name).map(|(_, o, s)| (*o, *s))
}

pub fn level(name: &str) -> Option<(&'static str, f32)> {
    LEVELS.iter().find(|(n, _, _)| *n == name).map(|(_, f, s)| (*f, *s))
}
