#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
"""Extract InstallShield 3 compressed archives (magic 0x8C655D13, e.g. ``data.1``).

These archives are what InstallShield 3 used on Windows 95-era CDs such as
Bryce 3D. ``unshield`` only handles InstallShield 5+ cabinets, so this small
standalone tool fills the gap.

Each member is stored with PKWARE Data Compression Library "implode"
compression. The decompressor below is an independent Python
implementation of the algorithm documented in Mark Adler's ``blast.c``.

Usage:
    is3extract.py list ARCHIVE
    is3extract.py extract ARCHIVE OUTDIR
"""

from __future__ import annotations

import argparse
import datetime as _dt
import os
import struct
import sys
from dataclasses import dataclass

SIGNATURE = 0x8C655D13


# --------------------------------------------------------------------------
# PKWARE DCL implode decompression ("blast")
# --------------------------------------------------------------------------

class BlastError(Exception):
    pass


class _Huffman:
    """Canonical Huffman table built from the compact DCL length encoding."""

    MAXBITS = 13

    def __init__(self, rep: list[int]):
        lengths: list[int] = []
        for byte in rep:
            count = (byte >> 4) + 1
            lengths.extend([byte & 15] * count)
        self.count = [0] * (self.MAXBITS + 1)
        for length in lengths:
            self.count[length] += 1
        offs = [0] * (self.MAXBITS + 2)
        for length in range(1, self.MAXBITS + 1):
            offs[length + 1] = offs[length] + self.count[length]
        self.symbol = [0] * len(lengths)
        for sym, length in enumerate(lengths):
            if length:
                self.symbol[offs[length]] = sym
                offs[length] += 1


_LITLEN = [
    11, 124, 8, 7, 28, 7, 188, 13, 76, 4, 10, 8, 12, 10, 12, 10, 8, 23, 8,
    9, 7, 6, 7, 8, 7, 6, 55, 8, 23, 24, 12, 11, 7, 9, 11, 12, 6, 7, 22, 5,
    7, 24, 6, 11, 9, 6, 7, 22, 7, 11, 38, 7, 9, 8, 25, 11, 8, 11, 9, 12,
    8, 12, 5, 38, 5, 38, 5, 11, 7, 5, 6, 21, 6, 10, 53, 8, 7, 24, 10, 27,
    44, 253, 253, 253, 252, 252, 252, 13, 12, 45, 12, 45, 12, 61, 12, 45,
    44, 173,
]
_LENLEN = [2, 35, 36, 53, 38, 23]
_DISTLEN = [2, 20, 53, 230, 247, 151, 248]
_BASE = [3, 2, 4, 5, 6, 7, 8, 9, 10, 12, 16, 24, 40, 72, 136, 264]
_EXTRA = [0, 0, 0, 0, 0, 0, 0, 0, 1, 2, 3, 4, 5, 6, 7, 8]

_LITCODE = _Huffman(_LITLEN)
_LENCODE = _Huffman(_LENLEN)
_DISTCODE = _Huffman(_DISTLEN)


def blast(data: bytes) -> bytes:
    """Decompress one PKWARE DCL implode stream."""
    pos = 0
    bitbuf = 0
    bitcnt = 0
    n = len(data)

    def need(k: int) -> None:
        nonlocal pos, bitbuf, bitcnt
        while bitcnt < k:
            if pos >= n:
                raise BlastError("unexpected end of compressed data")
            bitbuf |= data[pos] << bitcnt
            pos += 1
            bitcnt += 8

    def bits(k: int) -> int:
        nonlocal bitbuf, bitcnt
        if k == 0:
            return 0
        need(k)
        val = bitbuf & ((1 << k) - 1)
        bitbuf >>= k
        bitcnt -= k
        return val

    def decode(h: _Huffman) -> int:
        nonlocal bitbuf, bitcnt
        code = first = index = 0
        count_tbl = h.count
        for length in range(1, _Huffman.MAXBITS + 1):
            if bitcnt == 0:
                need(1)
            code |= (bitbuf & 1) ^ 1  # DCL stores Huffman codes bit-inverted
            bitbuf >>= 1
            bitcnt -= 1
            count = count_tbl[length]
            if code - count < first:
                return h.symbol[index + (code - first)]
            index += count
            first = (first + count) << 1
            code <<= 1
        raise BlastError("invalid Huffman code")

    lit = bits(8)
    if lit > 1:
        raise BlastError(f"invalid literal mode {lit}")
    dict_bits = bits(8)
    if not 4 <= dict_bits <= 6:
        raise BlastError(f"invalid dictionary size {dict_bits}")

    out = bytearray()
    while True:
        if bits(1):
            sym = decode(_LENCODE)
            length = _BASE[sym] + bits(_EXTRA[sym])
            if length == 519:  # end-of-stream marker
                break
            shift = 2 if length == 2 else dict_bits
            dist = (decode(_DISTCODE) << shift) + bits(shift) + 1
            if dist > len(out):
                raise BlastError("distance too far back")
            start = len(out) - dist
            if dist >= length:
                out += out[start:start + length]
            else:  # overlapping copy
                for i in range(length):
                    out.append(out[start + i])
        else:
            out.append(decode(_LITCODE) if lit else bits(8))
    return bytes(out)


# --------------------------------------------------------------------------
# InstallShield 3 archive layout
# --------------------------------------------------------------------------

@dataclass
class Member:
    directory: str
    name: str
    size: int
    compressed_size: int
    offset: int
    timestamp: _dt.datetime | None
    attributes: int

    @property
    def path(self) -> str:
        return f"{self.directory}\\{self.name}" if self.directory else self.name


def _dos_datetime(date: int, time: int) -> _dt.datetime | None:
    try:
        return _dt.datetime(
            1980 + (date >> 9), (date >> 5) & 15, date & 31,
            time >> 11, (time >> 5) & 63, (time & 31) * 2,
        )
    except ValueError:
        return None


def read_members(data: bytes) -> list[Member]:
    if len(data) < 51:
        raise ValueError("file too small for an InstallShield 3 archive")
    (sig,) = struct.unpack_from("<I", data, 0)
    if sig != SIGNATURE:
        raise ValueError(f"not an InstallShield 3 archive (magic {sig:#010x})")
    (file_count,) = struct.unpack_from("<H", data, 12)
    (archive_size,) = struct.unpack_from("<I", data, 18)
    (toc,) = struct.unpack_from("<I", data, 41)
    (dir_count,) = struct.unpack_from("<H", data, 49)
    if archive_size != len(data):
        print(f"warning: header says {archive_size} bytes, file has {len(data)}"
              " (multi-volume archives are not supported)", file=sys.stderr)

    pos = toc
    directories: list[tuple[str, int]] = []
    for _ in range(dir_count):
        count, chunk, name_len = struct.unpack_from("<HHH", data, pos)
        name = data[pos + 6:pos + 6 + name_len].decode("cp1252")
        directories.append((name, count))
        pos += chunk

    members: list[Member] = []
    for dir_name, count in directories:
        for _ in range(count):
            (dir_index,) = struct.unpack_from("<H", data, pos + 1)
            size, csize, offset = struct.unpack_from("<III", data, pos + 3)
            date, time, attrib = struct.unpack_from("<HHI", data, pos + 15)
            (chunk,) = struct.unpack_from("<H", data, pos + 23)
            name_len = data[pos + 29]
            name = data[pos + 30:pos + 30 + name_len].decode("cp1252")
            if directories[dir_index][0] != dir_name:
                print(f"warning: {name}: directory index {dir_index} does not"
                      f" match table order ({dir_name})", file=sys.stderr)
            members.append(Member(directories[dir_index][0], name, size, csize,
                                  offset, _dos_datetime(date, time), attrib))
            pos += chunk
    if len(members) != file_count:
        print(f"warning: header lists {file_count} files, table has"
              f" {len(members)}", file=sys.stderr)
    return members


def _safe_join(root: str, rel: str) -> str:
    parts = [p for p in rel.replace("\\", "/").split("/") if p not in ("", ".", "..")]
    path = os.path.join(root, *parts)
    if os.path.commonpath([os.path.abspath(root), os.path.abspath(path)]) != os.path.abspath(root):
        raise ValueError(f"refusing to write outside {root}: {rel}")
    return path


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    sub = ap.add_subparsers(dest="cmd", required=True)
    p_list = sub.add_parser("list", help="list archive members")
    p_list.add_argument("archive")
    p_ext = sub.add_parser("extract", help="extract all members")
    p_ext.add_argument("archive")
    p_ext.add_argument("outdir")
    args = ap.parse_args(argv)

    with open(args.archive, "rb") as fh:
        data = fh.read()
    members = read_members(data)

    if args.cmd == "list":
        for m in members:
            ts = m.timestamp.isoformat(sep=" ") if m.timestamp else "?"
            print(f"{m.size:>10} {m.compressed_size:>10}  {ts}  {m.path}")
        print(f"{len(members)} files")
        return 0

    errors = 0
    for m in members:
        target = _safe_join(args.outdir, m.path)
        os.makedirs(os.path.dirname(target), exist_ok=True)
        try:
            payload = blast(data[m.offset:m.offset + m.compressed_size])
        except BlastError as exc:
            print(f"error: {m.path}: {exc}", file=sys.stderr)
            errors += 1
            continue
        if len(payload) != m.size:
            print(f"error: {m.path}: expected {m.size} bytes, got {len(payload)}",
                  file=sys.stderr)
            errors += 1
        with open(target, "wb") as fh:
            fh.write(payload)
        if m.timestamp:
            stamp = m.timestamp.timestamp()
            os.utime(target, (stamp, stamp))
        print(f"{m.path}  ({m.size} bytes)")
    print(f"{len(members) - errors}/{len(members)} files extracted OK")
    return 1 if errors else 0


if __name__ == "__main__":
    sys.exit(main())
