#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
"""Reference model of Bryce 3D's CMath noise / random helpers (Axiom.dll).

This is an independent Python re-implementation written from the behaviour
described in ``docs/algorithms/noise.md``. It is meant as an executable
specification and as a test oracle for the later native code.

The only input that cannot be derived is a 256-byte constant table
(``CMath::noise32_``). It is *not* stored in this repository: the tool reads
it at run time from the user's own copy of ``Axiom.dll``, located through the
DLL's export directory ("bring your own CD").

Everything else (the 64-entry log2/exp2 tables, the four 256-entry noise
tables) is computed here exactly as the original computes or would compute
it, using IEEE double arithmetic. Python floats round every operation to a
53-bit mantissa, which matches the x87 FPU running with the Windows default
precision control (53 bit, round-to-nearest) for the value ranges used here.

Usage:
    bryce_math.py selftest AXIOM_DLL      # build tables, print digests + samples
    bryce_math.py vectors AXIOM_DLL       # print test vectors as JSON
"""

from __future__ import annotations

import argparse
import hashlib
import json
import math
import struct
import sys

NOISE32_EXPORT = "?noise32_@CMath@@0PBKB"

# Constants as stored in Axiom.dll (see docs/algorithms/noise.md).
RSCALE = 2.0 ** -32                      # float 0x2F800000
INV255_F32 = struct.unpack("<f", struct.pack("<f", 1.0 / 255.0))[0]  # 0x3B808081
BIAS_K = struct.unpack("<d", bytes.fromhex("fdffffffffffefbf"))[0]   # -0.9999999999999997
LOG2_TINY = 1e-307
LOG2_FLOOR = struct.unpack("<f", bytes.fromhex("9ec97fff"))[0]       # -3.4e38 (float)
# log2_ polynomial coefficients, pairs (c0, c1) used as (c0 - r*c1)
LOG2_A = (0.2883070248990067, 0.22949900023246153)
LOG2_B = (0.48089833404997356, 0.36067132973951144)
LOG2_C = (1.442695040888937, 0.7213475204127876)


def f32(x: float) -> float:
    """Round a double to IEEE single precision (what an x87 ``fstp dword`` does)."""
    return struct.unpack("<f", struct.pack("<f", x))[0]


# --------------------------------------------------------------------------
# Minimal PE reader: locate the noise32_ export in the user's Axiom.dll
# --------------------------------------------------------------------------

def read_noise32(path: str) -> bytes:
    d = open(path, "rb").read()
    pe = struct.unpack_from("<I", d, 0x3C)[0]
    if d[pe:pe + 4] != b"PE\0\0":
        raise ValueError("not a PE file")
    nsec = struct.unpack_from("<H", d, pe + 6)[0]
    optsz = struct.unpack_from("<H", d, pe + 20)[0]
    opt = pe + 24
    exp_rva = struct.unpack_from("<I", d, opt + 96)[0]
    secs = []
    o = opt + optsz
    for _ in range(nsec):
        vsize, va, rsize, rptr = struct.unpack_from("<IIII", d, o + 8)
        secs.append((va, max(vsize, rsize), rptr, rsize))
        o += 40

    def off(rva: int) -> int:
        for va, size, rptr, rsize in secs:
            if va <= rva < va + size and rva - va < rsize:
                return rptr + rva - va
        raise ValueError(f"RVA {rva:#x} not backed by file data")

    e = off(exp_rva)
    nfun, nnames, afun, aname, aord = struct.unpack_from("<IIIII", d, e + 20)
    for i in range(nnames):
        name_rva = struct.unpack_from("<I", d, off(aname) + 4 * i)[0]
        no = off(name_rva)
        name = d[no:d.index(b"\0", no)].decode("latin-1")
        if name == NOISE32_EXPORT:
            ordinal = struct.unpack_from("<H", d, off(aord) + 2 * i)[0]
            rva = struct.unpack_from("<I", d, off(afun) + 4 * ordinal)[0]
            return d[off(rva):off(rva) + 256]
    raise ValueError(f"export {NOISE32_EXPORT} not found")


# --------------------------------------------------------------------------
# Tables (CMath::Initialize)
# --------------------------------------------------------------------------

class Tables:
    def __init__(self, noise32: bytes):
        if len(noise32) != 256:
            raise ValueError("noise32 must be 256 bytes")
        self.noise32_bytes = noise32
        self.noise32 = struct.unpack("<64I", noise32)
        uiun = list(range(256))
        urgn = []
        for i in range(256):
            t = i * INV255_F32                    # exact in double
            urgn.append(f32((t + t) - (3.0 - (t + t)) * (t * t)))
        for i in range(256):                      # shuffle driven by noise32 bytes
            j = noise32[i]
            uiun[i], uiun[j] = uiun[j], uiun[i]
            urgn[i], urgn[j] = urgn[j], urgn[i]
        urun = [f32(u * INV255_F32) for u in uiun]
        srgn = [f32(g - 0.5) for g in urgn]
        # four wrap-around entries (index 256..259 == 0..3)
        self.uiun = uiun + uiun[:4]
        self.urun = urun + urun[:4]
        self.urgn = urgn + urgn[:4]
        self.srgn = srgn + srgn[:4]

    def digests(self) -> dict:
        h = lambda b: hashlib.sha256(b).hexdigest()
        return {
            "noise32": h(self.noise32_bytes),
            "uiun": h(bytes(self.uiun)),
            "urun": h(struct.pack("<260f", *self.urun)),
            "urgn": h(struct.pack("<260f", *self.urgn)),
            "srgn": h(struct.pack("<260f", *self.srgn)),
        }

    # --- lattice noise ----------------------------------------------------
    def _hash(self, *k: int) -> int:
        """Index chain: h = uiun[k0]; h = uiun[(h + k1) & 255]; ... (last step not looked up)."""
        h = k[0] & 0xFF
        for v in k[1:]:
            h = (self.uiun[h] + v) & 0xFF
        return h

    def uiunoise(self, *k: int) -> int:
        return self.uiun[self._hash(*k)]

    def urunoise(self, *k: int) -> float:
        return self.urun[self._hash(*k)]

    def urgnoise(self, *k: int) -> float:
        return self.urgn[self._hash(*k)]

    def srgnoise(self, *k: int) -> float:
        return self.srgn[self._hash(*k)]


# --------------------------------------------------------------------------
# CRndSeed and the random_ family
# --------------------------------------------------------------------------

class RndSeed:
    def __init__(self, seed: int = 0):
        self.set(seed)

    def set(self, seed: int) -> None:
        self.s, self.i, self.j = seed & 0xFFFFFFFF, 1, 1

    def step(self, t: Tables) -> int:
        s = self.s ^ t.noise32[self.i & 31] ^ t.noise32[32 + (self.j & 31)]
        self.s = s
        self.i = (self.i + 1) & 0xFFFFFFFF
        self.j = (self.j + s) & 0xFFFFFFFF
        return s


def _signed(u: int) -> int:
    return u - (1 << 32) if u & 0x80000000 else u


def randomu(t, r):            # also random_ (same code)
    return r.step(t)

def randomr(t, r):            # value in [-0.5, 0.5), returned unrounded in ST0
    return RSCALE * _signed(r.step(t))

def randomlr(t, r):           # value in [0, 1)
    return r.step(t) * RSCALE

def uniform1(t, a, r):        # [0, a)
    return (r.step(t) * RSCALE) * a

def uniform2(t, a, b, r):     # [a, b)
    k = RSCALE * (b - a)
    return r.step(t) * k + a

def dice(t, n, r):
    return r.step(t) % n

def cointoss(t, p, r):        # note: compares a *signed* value in [-0.5, 0.5)
    return 1 if RSCALE * _signed(r.step(t)) < f32(p) else 0


# --------------------------------------------------------------------------
# Shaping functions
# --------------------------------------------------------------------------

def step(x, e, a, b):
    return a if x < e else b

def pulse(x, e0, e1, a, b):
    f = a if x < e0 else b
    return f - a if x < e1 else f - b

def bstep(x, e0, e1, lo, hi):
    v = (x - e0) / (e1 - e0)
    if v < lo:
        return lo
    return v if v <= hi else hi

def smoothstep(x, e0, e1):
    if x < e0:
        return 0.0
    if not x < e1:
        return 1.0
    v = (x - e0) / (e1 - e0)
    return ((3.0 - (v + v)) * v) * v

def log2_(x: float) -> float:
    """Bit-level model of CMath::log2_ (table + polynomial)."""
    if not x > LOG2_TINY:                 # also NaN
        return LOG2_FLOOR
    bits = struct.unpack("<Q", struct.pack("<d", x))[0]
    hi = bits >> 32
    k = (hi >> 14) & 63                   # top 6 mantissa bits
    e = ((hi >> 20) & 0x7FF) - 1023
    m = struct.unpack("<d", struct.pack("<Q", (bits & 0x000FFFFFFFFFFFFF) | 0x3FF0000000000000))[0]
    recip = 1.0 / (1.0 + k / 64.0)        # == table entry (verified bit-exact)
    lg = math.log2(1.0 + k / 64.0)        # == table entry (correctly rounded)
    r = recip * m - 1.0
    r2 = r * r
    el = e + lg
    p = (LOG2_C[0] - r * LOG2_C[1]) + ((LOG2_B[0] - r * LOG2_B[1]) + (LOG2_A[0] - r * LOG2_A[1]) * r2) * r2
    return r * p + el

def bias(x, b):
    return math.pow(x, log2_(b) * BIAS_K)   # original: msvcrt _CIpow (x87)

def gain(x, g):
    k = log2_(1.0 - g) * BIAS_K
    if x < 0.5:
        return math.pow(x + x, k) * 0.5
    return 1.0 - math.pow(2.0 - (x + x), k) * 0.5

def gamma(x, g):
    return math.pow(x, 1.0 / g)

def sinc(x, k=1.0):
    a = k * x * math.pi
    return math.sin(a) / a                  # original: x87 fsin; x == 0 gives NaN


# --------------------------------------------------------------------------

def vectors(t: Tables) -> dict:
    r = RndSeed(0)
    rnd = [randomu(t, r) for _ in range(8)]
    r = RndSeed(12345)
    rr = [randomr(t, r) for _ in range(4)]
    return {
        "tables": t.digests(),
        "uiunoise1": [t.uiunoise(k) for k in range(8)],
        "urunoise2": [t.urunoise(k, 7) for k in range(4)],
        "urgnoise3": [t.urgnoise(k, 3, 9) for k in range(4)],
        "srgnoise3": [t.srgnoise(1, k, 2) for k in range(4)],
        "random_seed0": rnd,
        "randomr_seed12345": rr,
        "log2_": {str(x): log2_(x) for x in (0.1, 0.5, 1.0, 3.0, 1000.0)},
        "bias_0.3": [bias(x, 0.3) for x in (0.1, 0.5, 0.9)],
        "gain_0.7": [gain(x, 0.7) for x in (0.1, 0.5, 0.9)],
    }


def main(argv=None) -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("command", choices=["selftest", "vectors"])
    ap.add_argument("axiom_dll")
    a = ap.parse_args(argv)
    t = Tables(read_noise32(a.axiom_dll))
    if a.command == "vectors":
        json.dump(vectors(t), sys.stdout, indent=1)
        print()
        return 0
    # selftest: structural invariants
    assert sorted(t.uiun[:256]) == list(range(256)), "uiun must be a permutation"
    # 1/255 is stored rounded *up* as a float, so the top urgn entry is
    # 0x3F800001 (1.0000001) and the top srgn entry 0.5000001.
    assert min(t.urgn) == 0.0 and max(t.urgn) == f32(1.0 + 2.0 ** -23)
    assert min(t.srgn) == -0.5 and max(t.srgn) == f32(0.5 + 2.0 ** -23)
    assert min(t.urun) == 0.0 and max(t.urun) == 1.0
    for x in (0.001, 0.37, 1.0, 2.0, 7.5, 1e6):
        assert abs(log2_(x) - math.log2(x)) < 1e-12, x
    for k, v in t.digests().items():
        print(f"{k:8s} {v}")
    print("selftest ok")
    return 0


if __name__ == "__main__":
    sys.exit(main())
