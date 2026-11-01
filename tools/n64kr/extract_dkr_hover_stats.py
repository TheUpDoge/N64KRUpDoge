"""Extract DKR hovercraft stats from the user's own Diddy Kong Racing ROM.

Writes a small text file that src/n64kr/hover.c loads at runtime. The output
holds ROM data: it lives under build/ and must never be committed.

Usage (from the repo root):
    python -I tools/n64kr/extract_dkr_hover_stats.py "<path to DKR .z64>" build/x64/n64kr_dkr_hover.txt

Layout notes (DKR decomp a5eb080, recon/004-hover/SLICE-5_DKR_Stats.md):
- Asset lookup table at the ROM's assets offset: u32 count, then one u32
  start offset per section, relative to the end of the table.
- Section 15 = misc assets, 16 = misc table (word offsets, -1 terminated).
- Section 34 = object headers, 33 = their offset table. A racer object's
  header byte 0x5C (miscAssetIds[0]) is its acceleration curve.
"""
import hashlib
import struct
import sys

# DKR USA 1.1 (v80). Offsets from the decomp's dkr_assets_tool_extract.json.
SUPPORTED = {
    "6d96743d46f8c0cd0edb0ec5600b003c89b93755": {"name": "us.v80", "assets": 970976, "lut_size": 208},
}

SECTION_MISC = 15
SECTION_MISC_TABLE = 16
SECTION_OBJECT_TABLE = 33
SECTION_OBJECTS = 34

MISC_RACER_WEIGHT = 9
MISC_RACER_HANDLING = 10
# SHA-1 of the two stat tables in the decomp's asset list, to confirm the read.
MISC_SHA1 = {
    MISC_RACER_WEIGHT: "cd7675cbb4d678686cc7a6dd2c7dcf75034fbe7f",
    MISC_RACER_HANDLING: "0bd70b3abee14c180be4b21c120b506f09e839dc",
}

NUM_CHARACTERS = 10
CURVE_LENGTH = 16

# DKR Character enum order (include/enums.h) -> hovercraft object internal name.
HOVER_OBJECTS = [
    "KremlinHover",  # Krunch
    "BadgerHover",   # Bumper
    "TortHover",     # Tiptup
    "ConkaHover",    # Conker
    "TigerHover",    # Timber
    "BanjoHover",    # Banjo
    "ChickenHover",  # Drumstick
    "MouseHover",    # Pipsy
    "ticktockhover", # T.T.
    "diddyhover",    # Diddy
]


def main():
    if len(sys.argv) != 3:
        sys.exit(__doc__)
    rom = open(sys.argv[1], "rb").read()
    rom_sha1 = hashlib.sha1(rom).hexdigest()
    if rom_sha1 not in SUPPORTED:
        sys.exit(f"Unsupported DKR ROM (SHA-1 {rom_sha1}). Supported: USA 1.1 (v80), .z64 byte order.")
    info = SUPPORTED[rom_sha1]
    lut_start = info["assets"]
    lut_end = lut_start + info["lut_size"]

    def u32(off):
        return struct.unpack(">I", rom[off:off + 4])[0]

    def section(n):
        return rom[lut_end + u32(lut_start + 4 * (n + 1)):lut_end + u32(lut_start + 4 * (n + 2))]

    def table(n):
        raw = section(n)
        return [struct.unpack(">i", raw[i:i + 4])[0] for i in range(0, len(raw) - 3, 4)]

    misc = section(SECTION_MISC)
    misc_table = table(SECTION_MISC_TABLE)

    def misc_asset(index):
        return misc[misc_table[index] * 4:misc_table[index + 1] * 4]

    def floats(blob, count):
        return list(struct.unpack(f">{count}f", blob[:count * 4]))

    for index, sha in MISC_SHA1.items():
        if hashlib.sha1(misc_asset(index)).hexdigest() != sha:
            sys.exit(f"Misc asset {index} did not match the expected SHA-1; layout differs from this ROM.")

    objects = section(SECTION_OBJECTS)
    object_table = table(SECTION_OBJECT_TABLE)
    curve_ids = {}
    for i in range(len(object_table) - 1):
        header = objects[object_table[i]:object_table[i + 1]]
        if len(header) < 0x70:
            continue
        name = header[0x60:0x70].split(bytes(1))[0].decode("ascii", "replace")
        if name in HOVER_OBJECTS and name not in curve_ids:
            curve_ids[name] = header[0x5C]
    missing = [n for n in HOVER_OBJECTS if n not in curve_ids]
    if missing:
        sys.exit(f"Hovercraft objects not found: {missing}")

    weight = floats(misc_asset(MISC_RACER_WEIGHT), NUM_CHARACTERS)
    handling = floats(misc_asset(MISC_RACER_HANDLING), NUM_CHARACTERS)

    with open(sys.argv[2], "w", newline="\n") as out:
        out.write("# N64KR: DKR hovercraft stats extracted from your own ROM. Do not commit.\n")
        out.write(f"version {info['name']}\n")
        out.write("weight " + " ".join(f"{v:.9g}" for v in weight) + "\n")
        out.write("handling " + " ".join(f"{v:.9g}" for v in handling) + "\n")
        for character, name in enumerate(HOVER_OBJECTS):
            curve = floats(misc_asset(curve_ids[name]), CURVE_LENGTH)
            out.write(f"curve {character} " + " ".join(f"{v:.9g}" for v in curve) + "\n")
    print(f"Wrote {sys.argv[2]} ({info['name']}, {NUM_CHARACTERS} characters).")


if __name__ == "__main__":
    main()
