# Bryce 3D (1997) nativ auf Linux – Projektplan

Stand: 2026-11-21 · Ziel-Plattform: Fedora (aktuell 44), x86_64

## 1. Was auf der CD ist (Befund)

Die ISO `Bryce3DCD.iso` ist eine **Hybrid-CD** (Toast 3.5.1, 1997):

| Teil | Inhalt | Relevanz |
|---|---|---|
| ISO 9660 / Joliet | Windows-Version: InstallShield-Setup (`SETUP.EXE`, `_SETUP.LIB`, `setup.pkg`) + Cabinet `data.1` (28 MB, Magic `13 5D 65 8C` = InstallShield-5-CAB) | **Primäres RE-Ziel** |
| Apple Partition Map → **klassisches HFS** (nicht HFS+) | Mac-PPC-Version, verpackt mit *Installer VISE* (StuffIt-komprimiert), 7 PEF-Binaries (`Joy!peffpwpc`) | Sekundärquelle (Symbol-/Namens-Abgleich) |
| Gemeinsame Assets | ~20 `.br3`-Tutorial-/Beispielszenen, Material-/Objekt-/Sky-/Terrain-Bibliotheken, OBJ/DXF-Modelle, TIF-Texturen, 20 PDF-Handbuchkapitel | **Testkorpus + Spezifikation** |

Laut `setup.pkg` installiert das Windows-Setup u. a.:

- `Bryce 3D.exe` – Hauptprogramm (MSVC, Win95/NT, 32-bit PE)
- `Axiom.dll` – Dateisystem-/Container-Bibliothek (siehe `.br3` unten)
- `amberkrn.dll`, `amberd3d.dll`, `amberogl.dll`, `ambers3d.dll`, `ambersgi.dll` – „Amber“-3D-Abstraktion (Kernel + Backends Direct3D / OpenGL / Software / SGI-OpenGL) → nur für die **Echtzeit-Vorschau**, nicht für den eigentlichen Raytracer
- `msvcrt.dll`, `MSVCRT40.DLL` – C-Runtime
- ~50 `*.dat`-Dateien (`docwindow.dat`, `materialcomposer.dat`, `terraineditor.dat`, `menus.dat` …) – vermutlich die **komplett selbst gezeichnete UI** (Bitmaps + Layouts)
- Plugins: `OBJImport.bro`, `3DSImport.bro`, `DXFImport.bro`, `3DMFImport.bro`, `PS3Export.bri`, `BMPExport.bri`, `AVIExport.brm`
- Presets/Bibliotheken: `*.mat`, `*.obp`, `*.brt`, `*.bsk`, `*.tme` („Accel Engine“)

**Erster Format-Befund `.br3`:** Header `CCmF - Universal - Axiom - AGP - Composite File Management System (Universal)`. Danach Chunks mit umgedrehten FourCC-Tags (`rmacesab` = „basecamr“, `xtrb`, `bod3…`) und Klassennamen wie `CPictTexture`. → Szenen sind **plattformübergreifend** (Mac/PC) und vermutlich ein serialisierter Objektbaum aus C++-Klassen. Im Mac-HFS-Image taucht `tC3dBaseMaterial` hundertfach auf – gute Ankerpunkte.

## 2. Strategie

Eine vollständige 1:1-Dekompilierung zu nativem Code ist bei einer ~2–4 MB großen C++-Anwendung mit eigener UI ein Mehrjahresprojekt. Ich empfehle daher **schichtweise vorzugehen, vom Wertvollsten zum Aufwendigsten**, mit Wine als Referenz-Orakel:

```
 Wine (läuft, Referenz)  ──►  Formate (.br3/.mat/…)  ──►  nativer Renderer (CLI)  ──►  native UI
        ▲                               ▲                          ▲                       ▲
        └──── Golden-Renders / Traces ──┴────── Ghidra + REA ──────┴── Vergleichstests ────┘
```

Grundsatz: **Re-Implementierung** (sauberer, moderner C++/Rust-Code nach dekompilierter Logik), nicht Byte-genaue „Matching Decomp“. Erfolgskriterium ist pixelnahe Gleichheit der Renders gegenüber dem Original unter Wine.

## 3. Phasen

### Phase 0 – Setup & Extraktion (≈ 1 Abend)
- [x] Git-Repo `bryce3d-re` (öffentlich, GPL-3.0) mit `.gitignore` für ISO, extrahierte Binaries, Assets und Dekompilat (**nie committen/veröffentlichen** – urheberrechtlich geschützt).
- [ ] `sudo dnf install unshield wine` → `unshield -d extracted/win x data.1` (ggf. `data.1` → `data1.cab` umbenennen).
- [ ] Optional Mac-Seite: HFS lesen (`hfsutils` oder Python `machfs`), Installer VISE entpacken (schwierig – Fallback: Installation in einem Mac-OS-9-Emulator wie SheepShaver/QEMU-PPC).
- [ ] REA einrichten: `npx rea-agents setup` (MCP-Server für Claude Code registrieren), Ghidra-Provider auf `~/Tools/ghidra` zeigen lassen, `binary_session({})` zum Check.
- [ ] Ghidra-Projekt mit `Bryce 3D.exe` + allen DLLs anlegen; Analyzer „Microsoft RTTI“ / „Demangler“ aktivieren.

### Phase 1 – Baseline unter Wine (≈ 1 Woche)
- [ ] Bryce unter Wine (32-bit-Prefix, `WINEARCH=win32` bzw. WoW64) installieren und starten.
- [ ] Alle Beispielszenen von der CD rendern → **Golden Images** (PNG) + Renderzeiten festhalten.
- [ ] `WINEDEBUG=+relay,+file` Traces: welche Win32/GDI/DirectX/OpenGL-APIs werden tatsächlich genutzt, wann werden `.dat`-Dateien gelesen.
- Ergebnis: lauffähige Referenz + Testkorpus. (Nebenbei: falls das Ziel nur „läuft auf Fedora“ ist, endet das Projekt hier.)

### Phase 2 – Statische Kartierung mit REA/Ghidra (≈ 2–3 Wochen)
- [ ] REA `inspect_pe_imports` / `inspect_native_exports` / `inspect_pe_resources` für jede DLL/EXE → Modulkarte & Abhängigkeitsgraph.
- [ ] RTTI/vtables auswerten → **Klassenhierarchie** (`C3dBaseMaterial`, `CPictTexture`, …) als zentrales Dokument.
- [ ] Subsysteme verorten und benennen: Datei-I/O (Axiom), Szenengraph, Raytracer-Kern, Terrain (Heightfield), prozedurale Texturen (3D-Noise), Atmosphäre/Himmel/Wolken, Materialien, Animation, UI-Framework, Plugin-ABI.
- [ ] Mac-PPC-Binaries (falls entpackt) auf Traceback-Tabellen/Funktionsnamen prüfen und per Ghidra „Function ID“/BSim mit der x86-Version abgleichen.
- [ ] Findings-Ledger (`docs/findings.md`) mit REA-Evidence-IDs pflegen; Ghidra-Projekt regelmäßig sichern.

### Phase 3 – Dateiformate (≈ 3–4 Wochen) · *erster nativer Meilenstein*
- [ ] Axiom-Containerformat (`.br3` und vermutlich `.mat`, `.obp`, `.bsk`) spezifizieren → `docs/formats/axiom.md`.
- [ ] Parser-Bibliothek `libbryce-io` (C++20 oder Rust) + `br3dump` CLI, die eine Szene als JSON ausgibt.
- [ ] Round-Trip-Tests gegen **alle** `.br3`-Dateien der CD (Parse → Write → Bit-Vergleich).
- [ ] `.dat`-UI-Ressourcen und `.brt`-Texturpresets entschlüsseln.

### Phase 4 – Nativer Renderer (≈ 2–4 Monate) · *Kernwert*
- [ ] Raytracer-Kern nachbauen: Primitive (Kugel, Torus, Würfel, Kegel, Zylinder, Ebenen, Terrain-Heightfields, Symmetrische Lattice), Boolean-Objekte, Kameramodell.
- [ ] Prozedurale Texturen (Noise-Funktionen exakt nachbilden – hier ist Dekompilat-Genauigkeit entscheidend), Materialkanäle, Bump.
- [ ] Himmel/Atmosphäre: Sonne/Mond, Wolken, Nebel, Dunst, Volumenlicht.
- [ ] `bryce-render scene.br3 -o out.png` – Vergleich gegen Golden Images (PSNR/SSIM-Schwelle im CI).
- [ ] Danach: Multithreading, 64-bit, beliebige Auflösungen (echter Mehrwert gegenüber dem Original).

### Phase 5 – Native Oberfläche (≈ 6+ Monate, iterativ)
- [ ] Entscheidung UI-Toolkit: originalgetreue Nachbildung der Bryce-UI (SDL3 + OpenGL/Vulkan, Assets aus `.dat` zur Laufzeit vom Original-Install laden) **vs.** moderne Qt-Oberfläche.
- [ ] Echtzeit-Vorschau (Ersatz für Amber) mit OpenGL/Vulkan.
- [ ] Editoren in Reihenfolge des Nutzens: Szene/Kamera → Material Lab → Terrain Editor → Sky & Fog → Animation/Advanced Motion Lab.
- [ ] Import/Export-Plugins (OBJ, DXF, 3DS) nativ ersetzen.

### Phase 6 – Packaging
- [ ] RPM/Flatpak, das beim ersten Start die Original-CD/ISO verlangt und Assets daraus extrahiert (Modell wie OpenMW/OpenRCT2/ScummVM).

## 4. Werkzeuge

| Zweck | Tool |
|---|---|
| Extraktion | `7z`, `unshield`, `hfsutils`/`machfs`, ggf. SheepShaver |
| Disassembly/Dekompilierung | Ghidra (`~/Tools/ghidra`, vorhanden) |
| Agentische RE-Orchestrierung | REA (`rea-agents`, MCP-Server für Claude Code; Ghidra-Provider) |
| Referenz-Ausführung & Tracing | Wine (+relay), ggf. `winedbg`/gdb |
| Neu-Implementierung | C++20 + CMake (oder Rust), SDL3/Qt, OpenGL/Vulkan |
| Tests | Golden-Image-Vergleich (SSIM), Round-Trip-Tests der Formate |

## 5. Risiken

- **Umfang:** Phase 5 ist der größte Brocken; Phasen 3–4 liefern schon einen eigenständig nützlichen Wert (Bryce-Szenen nativ rendern).
- **Numerische Treue:** Noise/Terrain müssen sehr exakt nachgebaut werden, sonst sehen Szenen anders aus (x87-FPU-Rundung vs. SSE beachten).
- **Recht:** Bryce ist weiterhin urheberrechtlich geschützt (heute DAZ 3D). Reverse Engineering für Interoperabilität mit der eigenen Kopie ist in der EU (§ 69e UrhG) in engen Grenzen erlaubt; **keine Original-Binaries, Assets oder größere Dekompilat-Auszüge veröffentlichen**. Bei öffentlichem Repo: eigene Implementierung + „bring your own CD“-Modell.
- **REA-Reife:** REA ist ein Orchestrator; die eigentliche Arbeit macht Ghidra. Dekompilat bleibt Interpretation – jede Annahme gegen Wine-Verhalten verifizieren.

## 6. Offene Entscheidungen

1. **Ziel:** Reicht „läuft auf Fedora“ (→ Wine, fertig nach Phase 1) oder soll es wirklich eine native Re-Implementierung werden?
2. **Umfang:** Erst Renderer-CLI (Phasen 3–4) oder direkt volle App?
3. **Sprache:** C++20 (nah am Dekompilat) oder Rust?
4. ~~**Repo:** privat oder öffentlich?~~ → **Entschieden:** öffentlich als `bryce3d-re`, GPL-3.0. Konsequenz: Im Repo landen nur eigener Code und eigene Doku. Dekompilat, Ghidra-Projekte, REA-Evidence und Golden Renders bleiben lokal (siehe `.gitignore`).

## 7. Nächste konkrete Schritte

```bash
sudo dnf install unshield wine
```

```bash
mkdir -p extracted/win && 7z x Bryce3DCD.iso data.1 -oextracted && unshield -d extracted/win x extracted/data.1
```

```bash
npx rea-agents setup
```

Danach: Phase 1 (Wine-Baseline) und parallel Phase 2 mit `open_binary` auf `Bryce 3D.exe`.
