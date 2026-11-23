# Turns the WOFF (version 1) files of @fontsource/onest into plain TrueType fonts, which the
# native build's text renderer reads. WOFF 1 is the same tables, each zlib-compressed; this
# puts them back into an sfnt file. Run from the repository root after `pnpm install`:
#
#   python native/fonts/woff2ttf.py node_modules/@fontsource/onest/files native/fonts
import os
import struct
import sys
import zlib

WEIGHTS = ["400", "500", "600", "800"]
SUBSETS = ["latin", "latin-ext", "cyrillic", "cyrillic-ext"]


def woff_to_sfnt(data: bytes) -> bytes:
    sig, flavor, _length, num, _res, _total, _maj, _min, _mo, _ml, _mol, _po, _pl = struct.unpack(">4sIIHHIHHIIIII", data[:44])
    assert sig == b"wOFF", "not a WOFF 1 file"
    tables = []
    for i in range(num):
        tag, off, comp, orig, checksum = struct.unpack(">4sIIII", data[44 + 20 * i: 64 + 20 * i])
        raw = data[off:off + comp]
        body = zlib.decompress(raw) if comp < orig else raw
        assert len(body) == orig
        tables.append((tag, checksum, body))
    tables.sort(key=lambda t: t[0])
    entry_selector = max(num.bit_length() - 1, 0)
    search_range = (1 << entry_selector) * 16
    out = bytearray(struct.pack(">IHHHH", flavor, num, search_range, entry_selector, num * 16 - search_range))
    offset = 12 + 16 * num
    records = bytearray()
    bodies = bytearray()
    for tag, checksum, body in tables:
        records += struct.pack(">4sIII", tag, checksum, offset + len(bodies), len(body))
        bodies += body
        bodies += b"\0" * ((4 - len(body) % 4) % 4)
    return bytes(out + records + bodies)


def main(src: str, dst: str) -> None:
    os.makedirs(dst, exist_ok=True)
    for w in WEIGHTS:
        for s in SUBSETS:
            name = f"onest-{s}-{w}-normal"
            with open(os.path.join(src, name + ".woff"), "rb") as f:
                ttf = woff_to_sfnt(f.read())
            with open(os.path.join(dst, name + ".ttf"), "wb") as f:
                f.write(ttf)
            print(name, len(ttf))


if __name__ == "__main__":
    main(sys.argv[1], sys.argv[2])
