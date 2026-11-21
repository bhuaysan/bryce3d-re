# CLAUDE.md – bryce3d-re

Native Linux-Reimplementierung von **Bryce 3D** (MetaCreations, 1997) auf Basis von Reverse Engineering. Öffentliches Repo, GPL-3.0. Aktueller Stand und Phasen: [PLAN.md](PLAN.md). Befunde: [docs/findings.md](docs/findings.md).

## Sitzungsablauf (Übergabe)

- **Sitzungsbeginn:** zuerst [START.md](START.md) lesen (aktueller Stand und nächste Schritte).
- **Zuständigkeit, jeder Fakt hat genau einen Ort (sonst verlinken, nicht kopieren):**

| Datei | Besitzt |
|---|---|
| `START.md` | aktueller Stand und nächste Schritte, nichts anderes |
| `PLAN.md` | Phasen mit Checkboxen, Entscheidungen, Risiken. Keine Schrittlisten |
| `docs/findings.md` | Befunde mit Status beobachtet/abgeleitet/offen, Korrekturen |
| `docs/formats/`, `docs/algorithms/` | Spezifikationen |
| `CLAUDE.md` | Regeln, Layout, Befehle. Keinen Projektstand |

- **Sitzungsende:** `START.md` aktualisieren: was erledigt ist, neuer Stand, was als Nächstes ansteht. Knapp halten, **keine Historie** (die steht in `git log` und `docs/findings.md`), erledigte Punkte entfernen statt abhaken.

## Rechtliche Grenze (wichtigste Regel)

Das Repo ist öffentlich. Es darf **nur eigener Code und eigene Doku** hinein:

- **Nie committen:** ISO, entpackte Programmdateien/DLLs/Plugins, `.dat`/`.br3`/`.mat`/… von der CD, Texturen, Dekompilat, Disassembly-Auszüge, Ghidra-Projekte, REA-Evidence, Golden Renders. Alles unter `extracted/` ist tabu (siehe `.gitignore`).
- **Keine größeren Dekompilat-Auszüge** in Doku oder Code-Kommentaren. Verhalten in eigenen Worten beschreiben und eigenständig neu implementieren, nicht Ghidra-Ausgabe abschreiben.
- **Nicht ableitbare Konstanten** (z. B. `CMath::noise32_`, 256 Byte) kommen nicht ins Repo. Werkzeuge lesen sie zur Laufzeit aus der Kopie des Nutzers („bring your own CD“, wie OpenMW/ScummVM). Im Repo höchstens Hash-Präfixe zur Identifikation.
- Vor jedem Commit `git status` prüfen, ob versehentlich Originaldaten oder große Binärdateien gestaged sind.

## Layout

| Pfad | Inhalt |
|---|---|
| `PLAN.md` | Projektplan, Phasen 0–6, offene Entscheidungen, nächste Schritte (Deutsch) |
| `docs/findings.md` | Findings-Ledger. Jede Aussage markiert als **[beobachtet]**, **[abgeleitet]** oder **[offen]** |
| `docs/formats/axiom-cmf.md` | CCmF-Container, Axiom-Ressourcenschicht, Bryce-Datensätze (`.br3`, `.mat`, `.obp`, `.bsk`, `.brt`, `.tme`, `Data/*.dat`) |
| `docs/algorithms/noise.md` | `CMath`-Noise, RNG, Formfunktionen |
| `tools/is3extract.py` | InstallShield-3-Extraktor (`data.1`), inkl. PKWARE-DCL-Decoder |
| `tools/br3dump.py` | CCmF-/`.br3`-Dumper (Baum, `--json`, `--check`, `--entries`), nur lesend |
| `tools/bryce_math.py` | Ausführbares Referenzmodell der `CMath`-Funktionen (`selftest`, `vectors`) |
| `Bryce3DCD.iso` | Original-CD, lokal, git-ignoriert |
| `extracted/` | Lokal, git-ignoriert: `win/prog/` (entpackte Installation), `decomp/{axiom,exe}/` (Ghidra-Dekompilat mit Journal), `analysis/` (Export-Listen), `ghidra-checkpoints/` (REA-Checkpoints) |

Nativer Code (`src/`, `libbryce-io`) existiert noch nicht. Entschieden (2026-11-21): **Rust**, native Re-Implementierung, Ziel ist die volle App (die Renderer-CLI ist Zwischenstand und Testwerkzeug). Das UI-Toolkit ist noch offen – nicht eigenmächtig festlegen, sondern nachfragen.

## Befehle

Python 3 ohne Fremdabhängigkeiten. Tools immer mit `-I` aufrufen (Eingaben sind Originaldaten):

```bash
python3 -I tools/is3extract.py extract extracted/cd/data.1 extracted/win
python3 -I tools/br3dump.py --check extracted/win/prog/Presets/*
python3 -I tools/br3dump.py --json SZENE.br3
python3 -I tools/bryce_math.py selftest extracted/win/prog/Axiom.dll
```

Doku-Drift: `python3 -I tools/check_docs.py` am Sitzungsende ausführen, muss ohne `DRIFT` durchlaufen (prüft START.md-Form und -Aktualität, Schrittlisten in PLAN.md, Links).

Regressionstest für Formatänderungen: `br3dump.py --check` über alle 169 CCmF-Dateien (CD + Installation) muss ohne Warnung durchlaufen.

## Konventionen

- **Sprache:** Doku (`PLAN.md`, `docs/`) auf Deutsch, `README.md`, Code, Kommentare und Commit-Messages auf Englisch.
- Jede neue Python-Datei beginnt mit `#!/usr/bin/env python3` und `# SPDX-License-Identifier: GPL-3.0-or-later`, Docstring mit Zweck und Usage.
- Doku-Dateien beginnen mit „Stand: <Datum>“ und Quellenangabe (welche Binary, welches Werkzeug, REA-Evidence-ID gekürzt). Evidence-IDs sind lokal und nur Verweise.
- Befunde in `findings.md` sauber als beobachtet/abgeleitet/offen kennzeichnen. Widerlegte Annahmen ausdrücklich als **Korrektur** vermerken, nicht stillschweigend löschen.
- Nach Fortschritt Checkboxen in `PLAN.md` nachziehen. Nächste Schritte stehen **nur** in `START.md`.
- Ziel ist **Re-Implementierung**, kein Matching-Decomp. Erfolgskriterium: bitgenaue Formate und pixelnahe Renders gegenüber dem Original unter Wine.

## Technische Fakten, die man leicht falsch macht

- CCmF-Container und Ressourcenschicht sind **immer Big-Endian**; Bryce-Nutzdaten in der Bytereihenfolge des schreibenden Rechners (PC-Dateien: FourCC-Paare erscheinen „umgedreht“, z. B. `rmacesab`).
- `CCmSystem`/`CCFEntry` sind ein Nachrichtensystem, **nicht** das Dateiformat (das ist `CCmFile`).
- Die EXE hat keine RTTI; Klassennamen kommen aus Axiom-/Amber-Exporten (MSVC-dekoriert, mit `llvm-undname` auflösbar), Strings und Ressourcentypnamen.
- Numerik: x87 mit Windows-Standard (53-Bit-Präzision). Python-`float`/`double` reproduziert das für die bisher untersuchten Funktionen bitgenau. `float`-Konstanten aus dem Original exakt übernehmen (z. B. aufgerundetes `1/255`).
- Amber (`amber*.dll`) ist nur die Echtzeitvorschau; der Raytracer liegt in `Bryce 3D.exe`.

## Werkzeuge

- **REA** (MCP-Server) mit Ghidra **12.2** (`~/Tools/ghidra_12.2_PUBLIC`); der Symlink `~/Tools/ghidra` zeigt auf 12.1.4 und bleibt so (wird von einem anderen Projekt gebraucht). Checkpoint-Modus ist aktiv (`extracted/ghidra-checkpoints/`), Umbenennungen bleiben erhalten; Kommentare nur über `annotate_native_function`.
- Für Massendekompilierung alternativ die REA-CLI (`rea decompile-all`) mit `GHIDRA_INSTALL_DIR` auf 12.2. Vorhandenes Dekompilat in `extracted/decomp/` zuerst per `grep` nutzen, bevor neu dekompiliert wird.
- **Wine** 11.x (WoW64), Prefix `~/.local/share/wineprefixes/bryce3d`; die EXE läuft direkt aus `extracted/win/prog/` ohne Installer. Wine ist das Referenz-Orakel: Annahmen aus dem Dekompilat nach Möglichkeit dagegen verifizieren.
