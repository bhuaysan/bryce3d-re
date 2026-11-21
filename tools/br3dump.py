#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
"""Dump the structure of Bryce 3D scene files (``.br3``) and other Axiom
"CCmF" composite files.

Format description: ``docs/formats/axiom-cmf.md``. Three layers are decoded:

1. CCmF container  - header, directory, free list, chained blocks (always big-endian)
2. Resource layer  - master type table -> per-type resource tables -> entries
                     (Axiom ``CResourceEnv``; always big-endian)
3. Payload         - bytes written by Bryce itself, in the byte order of the
                     machine that saved the file. Only lightly probed here
                     (byte-order detection, embedded FourCC tag pairs).

The tool is read-only and never modifies its input.

Usage:
    br3dump.py FILE...                 # human-readable tree
    br3dump.py --json FILE...          # machine-readable
    br3dump.py --check FILE...         # only validate, one line per file
    br3dump.py --entries FILE          # also list every directory entry
"""

from __future__ import annotations

import argparse
import json
import re
import struct
import sys
from dataclasses import dataclass, field

HEADER_SIZE = 0x248          # identification (512) + format name (32) + 2 x u32 + 32 reserved
DIRDATA_OFF = 0x248
DIRDATA_SIZE = 0x48
TAG_SIZE = 12                # chunk tag: capacity, length, next
DIR_ENTRY_SIZE = 12
IDENT_PREFIX = b"CCmF - Universal - Axiom - AGP - Composite File Management System"
MASTER_REF = (16, 17)        # fixed CSmRef of the master table
TABLE_HEADER_SIZE = 32       # elemSize, capacity, growBy, count, 16 reserved
MASTER_ELEM = 32             # type name[16], PersR, DataR, 8 bytes runtime
RESENTRY_ELEM = 60           # identity, PersR, DataR, name[32], user0..3

FOURCC_RE = re.compile(rb"[a-z0-9]{4}[a-z0-9]{4}")


class CmfError(Exception):
    pass


def u32be(d: bytes, o: int) -> int:
    return struct.unpack_from(">I", d, o)[0]


def cstr(b: bytes) -> str:
    return b.split(b"\0", 1)[0].decode("mac_roman", "replace")


@dataclass
class Chunk:
    offset: int
    capacity: int      # bytes reserved incl. 12-byte tag
    length: int        # payload bytes in this chunk


@dataclass
class Entry:
    index: int
    used: bool
    size: int
    first: int
    chunks: list = field(default_factory=list)

    @property
    def fragmented(self) -> bool:
        return len(self.chunks) > 1 or (len(self.chunks) == 1 and self.chunks[0].capacity != self.chunks[0].length + TAG_SIZE)


class CmfFile:
    def __init__(self, path: str):
        self.path = path
        self.d = open(path, "rb").read()
        self.warnings: list[str] = []
        self._parse()

    # ---------------- layer 1: container ----------------
    def _chain(self, first: int, expect: int | None = None, what: str = "") -> tuple[bytes, list[Chunk]]:
        d, out, chunks, pos, seen = self.d, bytearray(), [], first, set()
        while pos:
            if pos in seen:
                raise CmfError(f"{what}: chunk loop at {pos:#x}")
            seen.add(pos)
            if pos < HEADER_SIZE + DIRDATA_SIZE or pos + TAG_SIZE > len(d):
                raise CmfError(f"{what}: chunk offset {pos:#x} out of range")
            cap, ln, nxt = struct.unpack_from(">III", d, pos)
            if ln + TAG_SIZE > cap:
                raise CmfError(f"{what}: chunk at {pos:#x} length {ln} exceeds capacity {cap}")
            if pos + TAG_SIZE + ln > len(d):
                raise CmfError(f"{what}: chunk at {pos:#x} runs past end of file")
            chunks.append(Chunk(pos, cap, ln))
            out += d[pos + TAG_SIZE: pos + TAG_SIZE + ln]
            pos = nxt
        if expect is not None and len(out) != expect:
            raise CmfError(f"{what}: chain holds {len(out)} bytes, directory says {expect}")
        return bytes(out), chunks

    def _parse(self) -> None:
        d = self.d
        if len(d) < 0x291:
            raise CmfError("file too small for a CCmF header")
        ident = d[:512]
        if not ident.startswith(IDENT_PREFIX):
            raise CmfError("not a CCmF composite file (identification string mismatch)")
        self.ident = cstr(ident)
        m = re.search(r"\(vers\. (\d+)\)", self.ident)
        self.version = int(m.group(1)) if m else None
        self.format = cstr(d[0x200:0x220])
        self.hdr_reserved = (u32be(d, 0x220), u32be(d, 0x224))
        if self.hdr_reserved != (0, 0):
            self.warnings.append(f"header words at 0x220/0x224 not zero: {self.hdr_reserved}")
        o = DIRDATA_OFF
        self.dir_used = d[o]
        self.dir_size, self.dir_first = u32be(d, o + 4), u32be(d, o + 8)
        self.free_used = d[o + 12]
        self.free_bytes, self.free_first = u32be(d, o + 16), u32be(d, o + 20)
        self.capacity, self.used_count, self.grow_by = u32be(d, o + 24), u32be(d, o + 28), u32be(d, o + 32)
        if not (self.capacity and self.used_count <= self.capacity and self.dir_used and self.free_used):
            raise CmfError("directory descriptor fails the original validity checks")
        if self.dir_size != self.capacity * DIR_ENTRY_SIZE:
            self.warnings.append(f"directory size {self.dir_size} != capacity*12")
        raw, self.dir_chunks = self._chain(self.dir_first, self.dir_size, "directory")
        self.entries: list[Entry] = []
        for i in range(self.capacity):
            used = raw[i * 12]
            size, first = struct.unpack_from(">II", raw, i * 12 + 4)
            e = Entry(i, bool(used), size, first)
            if used and first:
                _, e.chunks = self._chain(first, size, f"entry {i}")
            elif used and size:
                raise CmfError(f"entry {i}: size {size} but no first chunk")
            self.entries.append(e)
        n_used = sum(e.used for e in self.entries)
        if n_used != self.used_count:
            self.warnings.append(f"used flag count {n_used} != directory used count {self.used_count}")
        self.free_chunks = []
        if self.free_first:
            pos, total, seen = self.free_first, 0, set()
            while pos and pos not in seen:
                seen.add(pos)
                cap, ln, nxt = struct.unpack_from(">III", d, pos)
                self.free_chunks.append(Chunk(pos, cap, ln))
                total += cap - TAG_SIZE
                pos = nxt
            if total != self.free_bytes:
                self.warnings.append(f"free list holds {total} bytes, descriptor says {self.free_bytes}")
        self._check_coverage()
        self._parse_resources()

    def _check_coverage(self) -> None:
        """Every byte after the directory descriptor should belong to exactly one chunk."""
        spans = [(c.offset, c.capacity) for c in self.dir_chunks + self.free_chunks]
        for e in self.entries:
            spans += [(c.offset, c.capacity) for c in e.chunks]
        spans.sort()
        pos = HEADER_SIZE + DIRDATA_SIZE
        gaps = overlaps = 0
        for off, cap in spans:
            if off > pos:
                gaps += off - pos
            elif off < pos:
                overlaps += 1
            pos = max(pos, off + cap)
        self.coverage = {"gap_bytes": gaps, "overlaps": overlaps, "end": pos, "file_size": len(self.d)}
        if overlaps:
            self.warnings.append(f"{overlaps} overlapping chunks")
        if pos != len(self.d):
            self.warnings.append(f"chunks end at {pos:#x}, file size {len(self.d):#x}")

    def entry_bytes(self, i: int) -> bytes:
        e = self.entries[i]
        if not e.used or not e.first:
            return b""
        return self._chain(e.first, e.size, f"entry {i}")[0]

    # ---------------- layer 2: resource tables ----------------
    def _table(self, pers: int, data: int, elem: int, what: str):
        hdr = self.entry_bytes(data)
        if len(hdr) != TABLE_HEADER_SIZE:
            raise CmfError(f"{what}: table header entry {data} has {len(hdr)} bytes, expected 32")
        esz, cap, grow, count = struct.unpack_from(">IIII", hdr, 0)
        if esz != elem or count > cap:
            raise CmfError(f"{what}: bad table header elemSize={esz} capacity={cap} count={count}")
        body = self.entry_bytes(pers)
        if len(body) < count * elem:
            raise CmfError(f"{what}: table body entry {pers} too short ({len(body)} < {count * elem})")
        return {"elem_size": esz, "capacity": cap, "grow_by": grow, "count": count}, body

    def _parse_resources(self) -> None:
        self.types = []
        if self.format != "Resource Composite File":
            return
        try:
            info, body = self._table(*MASTER_REF, MASTER_ELEM, "master table")
        except (CmfError, IndexError) as ex:
            self.warnings.append(f"no resource layer: {ex}")
            return
        self.master_info = info
        for k in range(info["count"]):
            rec = body[k * MASTER_ELEM:(k + 1) * MASTER_ELEM]
            tname = cstr(rec[:16])
            pers, data = struct.unpack_from(">II", rec, 16)
            t = {"type": tname, "table_ref": [pers, data], "resources": []}
            try:
                tinfo, tbody = self._table(pers, data, RESENTRY_ELEM, f"type {tname}")
                t["table"] = tinfo
                for r in range(tinfo["count"]):
                    rr = tbody[r * RESENTRY_ELEM:(r + 1) * RESENTRY_ELEM]
                    ident, rp, rd = struct.unpack_from(">III", rr, 0)
                    users = list(struct.unpack_from(">IIII", rr, 0x2C))
                    payload = self.entry_bytes(rp) if rp < len(self.entries) else b""
                    aux = self.entry_bytes(rd) if rd < len(self.entries) else b""
                    res = {
                        "identity": ident, "name": cstr(rr[12:44]), "ref": [rp, rd],
                        "size": len(payload), "aux_size": len(aux), "user": users,
                        "probe": probe_payload(payload),
                    }
                    rec = decode_object_record(tname, payload)
                    if rec is None and "_e" in tname:
                        rec = decode_channel_header(payload)
                    if rec is not None:
                        res["record"] = rec
                    t["resources"].append(res)
            except CmfError as ex:
                self.warnings.append(str(ex))
            self.types.append(t)

    # ---------------- layer 3 (partial): object lists ----------------
    def _objlist_resources(self):
        for t in self.types:
            if t["type"] in ("C3dRootObjList", "C3dObjList"):
                for r in t["resources"]:
                    yield t["type"], r

    def _objlist(self, r: dict):
        """Decode one object list. Header: u32 1, u32 order flag (0 = written
        big-endian / Mac, 1 = little-endian / PC), u32 count; then count x 16-byte
        records. Byte order is decided per resource: preset libraries mix both."""
        d = self.entry_bytes(r["ref"][0])
        if len(d) < 12:
            return None
        for bo in (">", "<"):
            w0, w1, n = struct.unpack_from(bo + "III", d, 0)
            if w0 == 1 and w1 == (0 if bo == ">" else 1) and len(d) == 12 + 16 * n:
                break
        else:
            self.warnings.append(f"object list {r['identity']:#x}: unrecognised header")
            return None
        recs = []
        for k in range(n):
            o = 12 + 16 * k
            cls, sub = d[o:o + 4], d[o + 4:o + 8]
            if bo == "<":                      # FourCCs stored as native uint32 on x86
                cls, sub = cls[::-1], sub[::-1]
            ident, child = struct.unpack_from(bo + "II", d, o + 8)
            recs.append((cls.decode("latin-1"), sub.decode("latin-1"), ident, child))
        return ("big" if bo == ">" else "little"), recs

    def _payload_order_from_objlists(self) -> str | None:
        orders = set()
        for _, r in self._objlist_resources():
            x = self._objlist(r)
            if x:
                orders.add(x[0])
        if not orders:
            return None
        return orders.pop() if len(orders) == 1 else "mixed"

    def scene_tree(self) -> list:
        """Decode C3dRootObjList / C3dObjList into nested object trees
        (one tree per root list; preset libraries hold one root per preset)."""
        by_id = {}
        for t in self.types:
            for r in t["resources"]:
                by_id.setdefault(r["identity"], []).append((t["type"], r))
        lists = {}
        roots = []
        for tname, r in self._objlist_resources():
            x = self._objlist(r)
            if not x:
                continue
            if tname == "C3dRootObjList":
                roots.append((r, x[1]))
            else:
                lists[r["identity"]] = x[1]

        def build(recs, depth=0):
            out = []
            for cls, sub, ident, child in recs:
                target = [x for x in by_id.get(ident, []) if x[0] != "C3dObjList"]
                node = {"class": cls, "subclass": sub, "identity": ident,
                        "type": target[0][0] if target else None,
                        "name": target[0][1]["name"] if target else None}
                if child and child in lists and depth < 32:
                    node["children"] = build(lists[child], depth + 1)
                out.append(node)
            return out

        if len(roots) == 1:
            return build(roots[0][1])
        return [{"class": "root", "subclass": "", "identity": r["identity"], "type": "C3dRootObjList",
                 "name": f"root list {i}", "children": build(recs)} for i, (r, recs) in enumerate(roots)]

    # ---------------- summary ----------------
    def byte_order(self) -> str:
        """Byte order of the *payload* layer."""
        o = self._payload_order_from_objlists()
        if o:
            return {"big": "big (Mac)", "little": "little (PC)"}.get(o, "mixed")
        votes = {"big": 0, "little": 0}
        for t in self.types:
            for r in t["resources"]:
                bo = r["probe"].get("byte_order")
                if bo in votes:
                    votes[bo] += 1
        if votes["big"] == votes["little"]:
            return "unknown"
        return "big (Mac)" if votes["big"] > votes["little"] else "little (PC)"

    def as_dict(self, with_entries: bool = False) -> dict:
        out = {
            "file": self.path, "size": len(self.d), "version": self.version, "format": self.format,
            "directory": {"capacity": self.capacity, "used": self.used_count, "grow_by": self.grow_by,
                          "chunks": [[c.offset, c.capacity, c.length] for c in self.dir_chunks]},
            "free": {"bytes": self.free_bytes, "chunks": len(self.free_chunks)},
            "coverage": self.coverage, "payload_byte_order": self.byte_order(),
            "types": self.types, "scene": self.scene_tree(), "warnings": self.warnings,
        }
        if with_entries:
            out["entries"] = [{"index": e.index, "used": e.used, "size": e.size,
                               "chunks": [[c.offset, c.capacity, c.length] for c in e.chunks]}
                              for e in self.entries if e.used]
        return out


# ---------------- layer 3: decoded Bryce records (docs/formats/axiom-cmf.md §6.4-6.6) ----------------

OBJ_RECORD_TYPES = {
    # type name -> extra bytes after the common 640-byte object part
    "C3dSphere": 0, "C3dCube": 0, "C3dCone": 0, "C3dCylinder": 0, "C3dPlane": 0, "C3dSlab": 0,
    "C3dSquare": 0, "C3dCircle": 0, "C3dPyramid": 0, "C3dGroupType": 4, "C3dTorusType": 28,
    "C3dBaseCamera": 168,
}
OBJ_CHANNELS = ("Obj_eUnused", "Obj_ePathT", "Obj_ePosition", "Obj_eOrient", "Obj_eScale", "Obj_eShear", "Obj_eOrigin")
CAM_CHANNELS = ("Cam_eCraOrient", "Cam_eCorpOrient/Bank", "Cam_eFOV", "Cam_ePanX", "Cam_ePanY", "Cam_eZoomX", "Cam_eZoomY")


def decode_object_record(tname: str, d: bytes) -> dict | None:
    """Common object record: 0x108-byte base + 0x178-byte object part (+ subclass tail).
    The u32 PC flag at offset 0 selects the byte order (1 = little-endian)."""
    extra = OBJ_RECORD_TYPES.get(tname)
    if extra is None or len(d) != 0x280 + extra:
        return None
    flag = struct.unpack_from("<I", d, 0)[0]
    if flag not in (0, 1):
        return None
    bo = "<" if flag == 1 else ">"
    o = 0x108
    hdr = struct.unpack_from(bo + "5I", d, o)
    rec = {
        "pc_flag": flag,
        "name": cstr(d[8:0x108]),
        "material": hdr[4] or None,
        "matrix_a": [round(x, 9) for x in struct.unpack_from(bo + "12d", d, o + 0x14)],
        "matrix_b": [round(x, 9) for x in struct.unpack_from(bo + "12d", d, o + 0x74)],
        "channels": {n: c for n, c in zip(OBJ_CHANNELS, struct.unpack_from(bo + "7I", d, o + 0xDC)) if c},
    }
    t = 0x280
    if tname == "C3dGroupType":
        rec["group_ref"] = struct.unpack_from(bo + "I", d, t)[0]
    elif tname == "C3dTorusType":
        u0, a, b2 = struct.unpack_from(bo + "Idd", d, t)
        rec["torus"] = {"u0": u0, "d0": a, "d1": b2, "u1": list(struct.unpack_from(bo + "2I", d, t + 20))}
    elif tname == "C3dBaseCamera":
        rec["camera"] = {
            "v0": list(struct.unpack_from(bo + "3d", d, t + 4)),
            "flags": list(struct.unpack_from(bo + "2I", d, t + 0x1C)),
            "d1": struct.unpack_from(bo + "d", d, t + 0x4C)[0],
            "state": list(struct.unpack_from(bo + "7d", d, t + 0x54)),
            "channels": {n: c for n, c in zip(CAM_CHANNELS, struct.unpack_from(bo + "7I", d, t + 0x8C)) if c},
        }
    return rec


def decode_channel_header(d: bytes) -> dict | None:
    """Animation channel: u32 1, u32 PC flag, u32 key count n, u32, u32 extra, ...;
    keys of K bytes start at 0x94; size == 0x94 + n*K + extra."""
    if len(d) < 0x94:
        return None
    for bo in (">", "<"):
        v, flag, n, _, extra = struct.unpack_from(bo + "5I", d, 0)
        if v == 1 and flag == (1 if bo == "<" else 0):
            body = len(d) - 0x94 - extra
            if n and body >= 0 and body % n == 0:
                return {"pc_flag": flag, "keys": n, "key_size": body // n, "extra": extra}
            return None
    return None


def probe_payload(b: bytes) -> dict:
    """Light, non-authoritative look at a payload: FourCC tag pairs and their order."""
    res: dict = {}
    be_tags, le_tags = [], []
    for m in FOURCC_RE.finditer(b):
        if m.start() % 4:
            continue
        s = m.group().decode()
        a, c = s[:4], s[4:]
        # Bryce writes (class, subclass) FourCCs as native uint32 values: a
        # little-endian writer therefore produces each 4-char code reversed.
        le_tags.append(a[::-1] + c[::-1])
        be_tags.append(s)
    # class/subclass codes observed in the corpus (see docs/formats/axiom-cmf.md)
    known = {"camr", "3dob", "lght", "brtx", "base", "sphr", "plne", "path", "cube", "cone", "cylr",
             "grou", "hfld", "slab", "sqre", "tmsh", "toru"}
    score_be = sum(t[:4] in known or t[4:] in known for t in be_tags)
    score_le = sum(t[:4] in known or t[4:] in known for t in le_tags)
    if score_be or score_le:
        res["byte_order"] = "big" if score_be >= score_le else "little"
        tags = be_tags if score_be >= score_le else le_tags
        seen = []
        for t in tags:
            if t not in seen:
                seen.append(t)
        res["tags"] = seen[:16]
    return res


def print_tree(f: CmfFile, with_entries: bool) -> None:
    print(f"{f.path}")
    print(f"  CCmF version {f.version}, format \"{f.format}\", {len(f.d)} bytes")
    print(f"  directory: {f.used_count}/{f.capacity} entries used, grows by {f.grow_by}, "
          f"{len(f.dir_chunks)} chunk(s) at {', '.join(hex(c.offset) for c in f.dir_chunks)}")
    print(f"  free list: {f.free_bytes} bytes in {len(f.free_chunks)} chunk(s); "
          f"gaps {f.coverage['gap_bytes']} bytes")
    print(f"  payload byte order: {f.byte_order()}")
    for t in f.types:
        tab = t.get("table", {})
        print(f"  type {t['type']!r:24s} table={t['table_ref']} count={tab.get('count', '?')}")
        for r in t["resources"]:
            tags = " ".join(r["probe"].get("tags", [])[:4])
            print(f"    - {r['name']!r:34s} id={r['identity']:#010x} ref={r['ref']} "
                  f"{r['size']:7d} B  aux {r['aux_size']:5d} B  user={r['user']}"
                  + (f"  tags: {tags}" if tags else ""))
    tree = f.scene_tree()
    if tree:
        print("  scene objects:")

        recs = {r["identity"]: r for t in f.types for r in t["resources"]}

        def show(nodes, ind):
            for n in nodes:
                extra = ""
                rec = recs.get(n["identity"], {}).get("record")
                if rec and "matrix_b" in rec:
                    pos = rec["matrix_b"][9:12]
                    mat = recs.get(rec["material"], {}).get("name") if rec["material"] else None
                    extra = (f"  pos=({pos[0]:.3f}, {pos[1]:.3f}, {pos[2]:.3f})"
                             + (f" material={mat!r}" if mat is not None else "")
                             + (f" anim={','.join(k.split('_e')[1] for k in rec['channels'])}" if rec["channels"] else ""))
                print(f"{'    ' * ind}  {n['class']}/{n['subclass']}  {n['name']!r} ({n['type']}){extra}")
                show(n.get("children", []), ind + 1)
        show(tree, 1)
    if with_entries:
        for e in f.entries:
            if e.used:
                ch = ",".join(f"{c.offset:#x}+{c.length}" for c in e.chunks)
                print(f"  entry {e.index:4d}: {e.size:7d} B {'frag ' if e.fragmented else ''}{ch}")
    for w in f.warnings:
        print(f"  WARNING: {w}")


def main(argv=None) -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("files", nargs="+")
    g = ap.add_mutually_exclusive_group()
    g.add_argument("--json", action="store_true")
    g.add_argument("--check", action="store_true")
    ap.add_argument("--entries", action="store_true", help="list every used directory entry")
    a = ap.parse_args(argv)
    rc = 0
    results = []
    for path in a.files:
        try:
            f = CmfFile(path)
        except (CmfError, OSError, struct.error) as ex:
            print(f"FAIL {path}: {ex}", file=sys.stderr if not a.check else sys.stdout)
            rc = 1
            continue
        if a.check:
            nres = sum(len(t["resources"]) for t in f.types)
            status = "OK  " if not f.warnings else "WARN"
            print(f"{status} {path}: v{f.version} {f.used_count} entries, {len(f.types)} types, "
                  f"{nres} resources, payload {f.byte_order()}"
                  + ("" if not f.warnings else f" ({'; '.join(f.warnings)})"))
            if f.warnings:
                rc = rc or 2
        elif a.json:
            results.append(f.as_dict(a.entries))
        else:
            print_tree(f, a.entries)
    if a.json:
        json.dump(results, sys.stdout, indent=1)
        print()
    return rc


if __name__ == "__main__":
    sys.exit(main())
