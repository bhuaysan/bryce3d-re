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

**Format-Befund `.br3` (Stand 2026-11-21, Details in [docs/formats/axiom-cmf.md](docs/formats/axiom-cmf.md)):** Drei Schichten. (1) CCmF-Container: Big-Endian, Verzeichnis aus Einträgen, jeder Eintrag eine Kette von Blöcken. (2) Axiom-Ressourcenschicht: Typtabelle → Ressourcentabellen (Name, Identität, Eintragsreferenzen), ebenfalls Big-Endian. (3) Bryce-Nutzdaten in der Bytereihenfolge des schreibenden Rechners, je Ressource. Die „umgedrehten“ FourCCs (`rmacesab` = `camr`/`base` auf x86) stehen in den Objektlisten, aus denen sich der Szenenbaum rekonstruieren lässt. Ressourcentypen sind C++-Klassennamen (`C3dSphere`, `C3dBaseMaterial` …). Dasselbe Format nutzen `.mat`, `.obp`, `.bsk`, `.brt`, `.tme` und die UI-Dateien `Data/*.dat`.

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
- [x] Windows-Dateien entpacken: `data.1` ist InstallShield **3** (unshield kann das nicht) → eigenes Tool `python3 -I tools/is3extract.py extract data.1 extracted/win`.
- [ ] Optional Mac-Seite: HFS lesen (`hfsutils` oder Python `machfs`), Installer VISE entpacken (schwierig – Fallback: Installation in einem Mac-OS-9-Emulator wie SheepShaver/QEMU-PPC).
- [x] REA als MCP-Server verbunden; PE-Import/Export-Inventar erstellt (siehe [docs/findings.md](docs/findings.md)).
- [x] Ghidra 12.2 lokal installiert (`~/Tools/ghidra_12.2_PUBLIC`). Der MCP-Server der Desktop-Session nutzt aber noch den 12.1.4-Symlink (siehe findings.md, Werkzeug-Status). Übergangsweise läuft die Dekompilierung über die REA-CLI.
- [x] Volle Dekompilierung von `Axiom.dll` und `Bryce 3D.exe` (REA `decompile-all`, Ghidra 12.2) nach `extracted/decomp/` (lokal).
- [ ] Gleiches für `amberkrn.dll` und die Plugins. Persistentes Ghidra-Projekt (REA-Checkpoint-Modus) für Annotationen.

### Phase 1 – Baseline unter Wine (≈ 1 Woche)
- [ ] Bryce unter Wine (32-bit-Prefix, `WINEARCH=win32` bzw. WoW64) installieren und starten.
- [ ] Alle Beispielszenen von der CD rendern → **Golden Images** (PNG) + Renderzeiten festhalten.
- [ ] `WINEDEBUG=+relay,+file` Traces: welche Win32/GDI/DirectX/OpenGL-APIs werden tatsächlich genutzt, wann werden `.dat`-Dateien gelesen.
- Ergebnis: lauffähige Referenz + Testkorpus. (Nebenbei: falls das Ziel nur „läuft auf Fedora“ ist, endet das Projekt hier.)

### Phase 2 – Statische Kartierung mit REA/Ghidra (≈ 2–3 Wochen)
- [ ] REA `inspect_pe_imports` / `inspect_native_exports` / `inspect_pe_resources` für jede DLL/EXE → Modulkarte & Abhängigkeitsgraph.
- [ ] RTTI/vtables auswerten → **Klassenhierarchie** (`C3dBaseMaterial`, `CPictTexture`, …) als zentrales Dokument. Die EXE hat keine RTTI. Klassennamen liefern die Ressourcentypen der `.br3`-Dateien (siehe axiom-cmf.md) und die FourCC-Klassencodes der Objektlisten.
- [x] `CMath`-Noise/Zufall/Formfunktionen dokumentiert ([docs/algorithms/noise.md](docs/algorithms/noise.md)), Referenzmodell `tools/bryce_math.py`.
- [ ] Subsysteme verorten und benennen: Datei-I/O (Axiom), Szenengraph, Raytracer-Kern, Terrain (Heightfield), prozedurale Texturen (3D-Noise), Atmosphäre/Himmel/Wolken, Materialien, Animation, UI-Framework, Plugin-ABI.
- [ ] Mac-PPC-Binaries (falls entpackt) auf Traceback-Tabellen/Funktionsnamen prüfen und per Ghidra „Function ID“/BSim mit der x86-Version abgleichen.
- [ ] Findings-Ledger (`docs/findings.md`) mit REA-Evidence-IDs pflegen; Ghidra-Projekt regelmäßig sichern.

### Phase 3 – Dateiformate (≈ 3–4 Wochen) · *erster nativer Meilenstein*
- [x] Axiom-Containerformat und Ressourcenschicht spezifiziert → [docs/formats/axiom-cmf.md](docs/formats/axiom-cmf.md). Gilt für `.br3`, `.mat`, `.obp`, `.bsk`, `.brt`, `.tme` und `Data/*.dat`.
- [x] `tools/br3dump.py` (Python-Prototyp): Container, Ressourcenbaum, Szenenhierarchie, JSON-Ausgabe. Liest alle 169 CCmF-Dateien ohne Warnung.
- [x] Ladepfad und Byte-Order-Regel der Bryce-Datensätze; Layout von Basis-, Objekt-, Gruppen-, Torus- und Kameradatensätzen sowie Header und Keys der Animationskanäle (axiom-cmf.md §6). `br3dump.py` dekodiert sie.
- [x] Material (`C3dBaseMaterial`, 976 B) vollständig dekodiert (axiom-cmf.md §6.7), Referenzen laufen in `br3dump.py --check` über alle 169 Dateien auf.
- [ ] Textur (`C3dTexture`, 1588 B): Gliederung und Kopf dekodiert (§6.8), prozedurale Komponenten offen.
- [ ] Restliche Klassen: Lichter, Terrain, Dreiecksnetz, Pfad, Himmel, Sonne, `CPictTexture`, `C3dCImage`, `brycedocset`, Key-Wertformate.
- [ ] Parser-Bibliothek `libbryce-io` (C++20 oder Rust, Entscheidung offen) nach dem Vorbild von `br3dump.py`.
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

1. **REA der Desktop-Session** läuft seit 2026-11-21 mit Ghidra 12.2 (verifiziert). Checkpoint-Verzeichnis `extracted/ghidra-checkpoints/` ist eingetragen, damit Annotationen über Sitzungen erhalten bleiben (aktiv und mit Umbenennungs-Rundlauf verifiziert).
2. **Textur-Rest**: Doubles und Matrizen der Komponentenblöcke, Restbereich T+0x384–0x43C und die Bedeutung von `kind`, Winkeln und Modi. Am ehesten über die Auswertung im Renderer (Funktion, die `C3dTexture` beim Shading liest) statt über die Leser. Danach `CPictTexture`/`C3dCImage`, Terrain und Lichter. Hinweis: vtable-only-Methoden fehlen im Dekompilat (Typname → Getter → `search_bytes` → vtable, `objdump -d`).
3. **Laufzeit-Gegenprobe Noise**: Tabellen aus dem laufenden Bryce unter Wine auslesen (winedbg) und mit `tools/bryce_math.py` vergleichen.
4. **Höhere Noise-Funktionen** (Perlin/fBm/Turbulenz) in der EXE lokalisieren. Sie bauen auf den CMath-Bausteinen auf.
5. Phase 1 fortsetzen: Registrierungsdialog unter Wine durchklicken, Golden Renders der 54 Szenen.
6. Entscheidung **C++20 oder Rust** für `src/`.
