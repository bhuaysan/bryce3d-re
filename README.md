# bryce3d-re

A native Linux reimplementation of **Bryce 3D** (MetaCreations, 1997), based on reverse engineering.

> **Status:** planning or early research. Nothing runs yet. See [PLAN.md](PLAN.md) (in German).

## Goal

Run Bryce 3D scenes on modern Linux (Fedora first) natively. Approach, in order:

1. **Reference baseline:** run the original Windows release under Wine and record golden renders.
2. **File formats:** document and parse the Axiom container format (`.br3` scenes, material/object/sky libraries).
3. **Native renderer:** a command-line ray tracer (`bryce-render scene.br3 -o out.png`), validated against the golden renders.
4. **Native UI:** an editor front end, built step by step.

## You need your own copy

This repository contains **only original code and documentation**. It does not ship and never will ship:

- Bryce program files, DLLs, plugins or UI resources
- Scenes, libraries, textures or other assets from the CD
- Decompiled or disassembled original code

To use this project, you need a legally obtained Bryce 3D CD. Tools here read data from your own copy at runtime (the same model as OpenMW, OpenRCT2 and ScummVM).

## Legal

Bryce is a trademark of its respective owner (currently DAZ 3D). This project is not affiliated with, endorsed by or connected to DAZ 3D or the former MetaCreations. The reverse engineering is done for interoperability with lawfully owned copies.

## License

The code in this repository is licensed under the [GNU General Public License v3.0](LICENSE).
