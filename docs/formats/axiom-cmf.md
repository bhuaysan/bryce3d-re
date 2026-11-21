# Axiom „CCmF“ Composite File (`.br3`, `.mat`, `.obp`, `.bsk`, `.brt`, `.tme`, `Data/*.dat`)

Stand: 2026-11-21 (Material und Textur ergänzt). Quellen: statische Analyse von `Axiom.dll` (`CCmFile`, `CResourceEnv`, `CMasterTable`, `CResTypeTable`, `CResEntry`, `CSpoolMem`) mit Ghidra 12.2 über REA (Evidence `ev_0ce71878fb83d786…`, lokal) sowie Auswertung aller 169 CCmF-Dateien der CD und der Windows-Installation.
Werkzeug: [`tools/br3dump.py`](../../tools/br3dump.py).

Die Beschreibung ist eine eigene Formulierung. Alle Offsets sind Byte-Offsets in der Datei bzw. im Datensatz.

## 0. Überblick

Ein `.br3` besteht aus drei Schichten:

| Schicht | Zuständig | Bytereihenfolge |
|---|---|---|
| 1. **Container** (CCmF): Header, Eintragsverzeichnis, Freiliste, verkettete Blöcke | `Axiom.dll` `CCmFile` | **immer Big-Endian** |
| 2. **Ressourcen**: Typtabelle → Ressourcentabellen → Einträge | `Axiom.dll` `CResourceEnv` | **immer Big-Endian** |
| 3. **Nutzdaten** einzelner Ressourcen (Objekte, Materialien, Animationskanäle …) | `Bryce 3D.exe` | **Bytereihenfolge des schreibenden Rechners**, Kennung je Ressource |

Schichten 1 und 2 sind Mac-nativ. Auf einem Little-Endian-Host tauscht Axiom beim Lesen und Schreiben die Bytes. Schicht 3 schreibt Bryce als native Speicherabbilder. Deshalb erscheinen die FourCC-Paare in PC-Dateien „umgedreht“ (`rmacesab` statt `camrbase`, siehe §5).

Alle 169 Dateien der CD/Installation tragen den Formatnamen `Resource Composite File` und die Container-Version 5.

## 1. Container-Header (0x000–0x247)

| Offset | Größe | Inhalt |
|---|---|---|
| 0x000 | 512 | Kennungs-String, NUL-terminiert, Rest NUL. Muss **exakt** gleich dem eingebauten String sein. Er beginnt mit `CCmF - Universal - Axiom - AGP - Composite File Management System (Universal) - Created by …` und enthält `(vers. 5)` |
| 0x200 | 32 | Formatname, NUL-terminiert (`Resource Composite File`; Vorgabe beim Anlegen ohne Namen: `Generic Composite File`) |
| 0x220 | 4 | u32 BE, muss 0 sein (sonst Fehler 4) |
| 0x224 | 4 | u32 BE, muss 0 sein (sonst Fehler 5) |
| 0x228 | 32 | reserviert, 0 |

Prüfung beim Öffnen (`Open` → `ValidateHeader`): Datei ≥ 0x291 Byte, Kennung gleich (sonst 100), optional Formatname gleich dem erwarteten (sonst 2), beide Reservewörter 0.

## 2. Verzeichnis-Deskriptor (0x248–0x28F, 0x48 Byte)

| Offset | Typ | Feld |
|---|---|---|
| 0x248 | u8 + 3 Pad | Verzeichnis-Eintrag „belegt“ (≠ 0 erforderlich) |
| 0x24C | u32 BE | Verzeichnisgröße in Byte (= Kapazität × 12) |
| 0x250 | u32 BE | Offset des ersten Blocks des Verzeichnisses |
| 0x254 | u8 + 3 Pad | Freilisten-Eintrag „belegt“ (≠ 0 erforderlich) |
| 0x258 | u32 BE | freie Nutzbytes gesamt |
| 0x25C | u32 BE | Offset des ersten freien Blocks (0 = keine) |
| 0x260 | u32 BE | Kapazität (Anzahl Verzeichniseinträge, > 0) |
| 0x264 | u32 BE | Anzahl belegter Einträge (≤ Kapazität) |
| 0x268 | u32 BE | Wachstumsschritt beim Vergrößern des Verzeichnisses (beobachtet 128) |
| 0x26C | 36 | reserviert (wird nicht getauscht), beobachtet 0 |

Die ersten zwei Felder sind selbst je ein **Verzeichniseintrag** (§3), einer für das Verzeichnis, einer für die Freiliste. Das Verzeichnis wird also wie jeder andere Eintrag gespeichert.

## 3. Verzeichniseintrag (12 Byte)

| Offset | Typ | Feld |
|---|---|---|
| 0 | u8 | belegt (0 = frei, 1 = belegt) |
| 1 | 3 | Pad |
| 4 | u32 BE | Nutzgröße des Eintrags in Byte |
| 8 | u32 BE | Dateioffset des ersten Blocks (0 = kein Inhalt) |

Ein Eintrag kann belegt **und** leer sein (Größe 0, Offset 0). In allen Szenen sind die Einträge 0–15 so reserviert: `CResourceEnv` belegt sie beim Anlegen einer Datei, nutzt sie aber nicht.

## 4. Blöcke und Ketten

Der Inhalt jedes Eintrags (auch des Verzeichnisses und der Freiliste) ist eine **einfach verkettete Liste von Blöcken**. Jeder Block beginnt mit einem 12-Byte-Tag:

| Offset | Typ | Feld |
|---|---|---|
| 0 | u32 BE | Kapazität des Blocks **inklusive** Tag |
| 4 | u32 BE | Nutzbytes in diesem Block (≤ Kapazität − 12) |
| 8 | u32 BE | Offset des nächsten Blocks, 0 = Ende |

Lesen eines Eintrags: ab dem ersten Offset jeweils `Nutzbytes` hinter dem Tag lesen, dem `next`-Zeiger folgen und aneinanderhängen. Die Summe muss der Nutzgröße im Verzeichnis entsprechen (sonst Fehler 8).
„Fragmentiert“ heißt im Original: mehr als ein Block, oder genau ein Block mit Kapazität ≠ Nutzbytes + 12.

Freie Blöcke stehen in der Freiliste. Sie haben `Nutzbytes = Kapazität − 12`, die Summe der Nutzbytes steht in 0x258. Beim Freigeben wird die Kette des Eintrags vorn an die Freiliste gehängt.

Beobachtet im Korpus: Die Blöcke aller Ketten (Verzeichnis, Freiliste, Einträge) überdecken den Bereich von 0x290 bis Dateiende **lückenlos und überlappungsfrei**. Das Verzeichnis ist oft fragmentiert (bis 78 Blöcke), weil es beim Wachsen um neue Blöcke verlängert wird. Nur `Presets/Volume.mat` hat eine nicht leere Freiliste (2 Blöcke, 17 765 Byte).

## 5. Ressourcenschicht (`CResourceEnv`)

### 5.1 Referenzen und Tabellen

Ressourcen verweisen auf Container-Einträge über ein Paar **(PersR, DataR)** von Eintragsnummern (`CSmRef`).

Tabellen sind dynamische Arrays aus zwei Einträgen:

- **DataR** → 32-Byte-Tabellenkopf: `u32 Elementgröße, u32 Kapazität, u32 Wachstumsschritt, u32 Anzahl`, dann 16 Byte reserviert (alle BE).
- **PersR** → Elementdaten, Kapazität × Elementgröße Byte. Die ersten *Anzahl* Elemente sind gültig.

### 5.2 Master-Tabelle (Typen)

Fest verlinkt mit **(PersR = 16, DataR = 17)**. Elementgröße 32 (geprüft):

| Offset | Typ | Feld |
|---|---|---|
| 0 | char[16] | Typname, NUL-terminiert (z. B. `C3dSphere`, `Obj_ePosition`, `canv`) |
| 16 | u32 BE | PersR der Ressourcentabelle dieses Typs |
| 20 | u32 BE | DataR der Ressourcentabelle dieses Typs |
| 24 | 8 | nicht getauscht, [abgeleitet] Laufzeitfelder, Inhalt ohne Bedeutung |

### 5.3 Ressourcentabelle eines Typs (`CResEntry`, 60 Byte)

| Offset | Typ | Feld |
|---|---|---|
| 0 | u32 BE | Identität (eindeutiger Schlüssel, siehe §6.1) |
| 4 | u32 BE | PersR: Eintrag mit den Nutzdaten |
| 8 | u32 BE | DataR: zweiter Eintrag (meist leer; bei `brycedocset` trägt er die Daten) |
| 12 | char[32] | Ressourcenname, NUL-terminiert (MacRoman), darf leer sein |
| 44 | 4 × u32 BE | User-Felder 0–3 (im Korpus 0) |

Typnamen sind entweder C++-Klassennamen von Bryce (`C3dBaseCamera`, `C3dBaseMaterial`, `C3dTexture`, `C3dHeightField`, `CPictTexture` …), Animationskanäle (`Obj_ePosition`, `Cam_eFOV`, `Sky_eHazeAmount`, `Mat_eDiffuseC` …), Vorschau- und Preset-Daten (`matprevcimage`, `CPresetDescr`) oder in den UI-Dateien kurze Kürzel (`canv`, `elem`, `menu`, `ImageIV32ARGB` …).

## 6. Nutzdaten (Schicht 3, teilweise)

### 6.1 Allgemein

- Geschrieben von `Bryce 3D.exe` als native Speicherabbilder. Mac-Dateien sind Big-Endian, PC-Dateien Little-Endian.
- **Die Bytereihenfolge gilt je Ressource, nicht je Datei.** `Presets/Paths.obp` enthält PC-Objektlisten neben zwei Mac-Objektlisten.
- **PC-Flag:** Jeder Datensatz trägt ein u32-Flag, 1 = auf dem PC geschrieben (Little-Endian), 0 = auf dem Mac geschrieben (Big-Endian). Der Leser der EXE tauscht jedes Feld mit seiner Breite (u32, double, float), wenn das nativ gelesene Flag ≠ 1 ist (beobachtet in allen Lesern, z. B. `FUN_0052cd90`, `FUN_00578390`, `FUN_005cc4c0`). Lage des Flags:
  - Klassendatensätze (`C3d*`, §6.4): Offset 0.
  - Objektlisten und Animationskanäle (§6.2, §6.6): Offset 4; Offset 0 enthält dort immer 1 (Versionsnummer).
  Im Korpus gilt das ausnahmslos (3279 Objekt-, 241 Listen-, 8854 Kanaldatensätze der Szenen). Ausnahme: `C3dCImage` beginnt mit dem FourCC `rle2` (komprimiertes Bild).
- Die Identität (§5.3) hat in Mac-Dateien Werte wie `0x05688430`, in PC-Dateien wie `0x00AEDFC8`. [abgeleitet] Das sind Speicheradressen der Objekte zum Speicherzeitpunkt, die als Schlüssel für Querverweise dienen. Ausnahmen: Root-Objektliste = 1, `brycedocset` = 0.
- FourCC-Codes speichert Bryce als `uint32`. Auf dem Mac stehen sie lesbar in der Datei (`camr`), auf dem PC umgekehrt (`rmac`). Ein Paar Klasse/Unterklasse ergibt also `camrbase` (Mac) bzw. `rmacesab` (PC).

### 6.2 Objektlisten (`C3dRootObjList`, `C3dObjList`)

Inhalt des PersR-Eintrags:

| Offset | Typ | Feld |
|---|---|---|
| 0 | u32 | immer 1 (Version) |
| 4 | u32 | PC-Flag: 0 in jeder Big-Endian-, 1 in jeder Little-Endian-Liste (alle 241 Listen der 54 Szenen sowie alle Listen der Presets, auch die gemischte `Paths.obp`) |
| 8 | u32 | Anzahl n |
| 12 | n × 16 | Records |

Record (16 Byte, in der Bytereihenfolge der Liste):

| Offset | Typ | Feld |
|---|---|---|
| 0 | FourCC (u32) | Klasse: `camr` Kamera, `3dob` 3D-Objekt, `lght` Licht |
| 4 | FourCC (u32) | Unterklasse, beobachtet: `base` (Kamera); `sphr`, `cube`, `cone`, `cylr`, `toru`, `plne`, `sqre`, `slab`, `hfld` (Terrain), `tmsh` (Dreiecksnetz), `path`, `grou` (Gruppe); bei Licht `sphr`/`cone`/`cube` |
| 8 | u32 | Identität des Objekts (→ `CResEntry` eines Objekttyps) |
| 12 | u32 | Identität der Kind-Objektliste (`C3dObjList`) bei Gruppen und Pfaden, sonst 0 |

Szenen haben genau eine `C3dRootObjList`. Bei Gruppen entsteht über die Kind-Listen ein Baum. Preset-Bibliotheken (`.obp`) haben eine Root-Liste pro Preset. Im Korpus lösen sich alle 3279 Records der 54 Szenen auf eine Ressource auf.

Szenen beginnen mit 16 Kameras (`perspective`, `top`, `right`, `front`, `left`, `back`, `bottom`, `director`, `dot0`–`dot7`), danach folgen die Objekte in Listenreihenfolge.

### 6.3 Ladepfad in der EXE (Übersicht)

Quelle: statische Analyse von `Bryce 3D.exe` (REA `decompile_all` `ev_cff33f09…` plus Disassembly). Adressen beziehen sich auf die EXE mit SHA-256 `7cceb0d7…99ebe7`.

1. Öffnen eines Dokuments (`FUN_00424de0`): Bei der Endung **`.brc`** geht es in einen separaten Konvertierungspfad (`FUN_00429680` → `FUN_0060c870`), der alle Felder unbedingt tauscht und andere Datensatz-Offsets verwendet. [abgeleitet] Ein älteres oder Mac-spezifisches Format. [offen] Welches. Bei allen anderen Endungen (`.br3`) folgt der native Lader `FUN_00429420` → `FUN_005296e0`.
2. Für jeden Record der Objektlisten wählt `FUN_00529bc0` anhand von (Klasse, Unterklasse) eine Fabrikfunktion. Die vollständige Liste der Codes steht im Code und enthält auch Unterklassen, die im Korpus fehlen: `3dob/` `cone circ cylr cube hfld grou plne path slab pyra sqre sphr symh tmsh toru`; `lght/` `cube cone pyrm cyln sphr sunl`; `camr/base`.
3. `FUN_00529f40` erzeugt das Objekt und ruft dessen virtuelle Lademethode (vtable +0x30). Die holt über den Klassennamen (vtable +0x18, z. B. `C3dSphere`) und die gespeicherte Identität die Ressource (`FUN_0052ce60`) und übergibt die Rohdaten dem klassenspezifischen **Leser auf vtable +0x28**.
4. Die Leser sind geschichtet wie die Klassen. Jeder liest seinen Abschnitt ab einem Cursor, tauscht ihn bei Bedarf und rückt den Cursor um seine feste Größe weiter. Danach lädt die Basisklasse die Animationskanäle (`FUN_00578ea0`).

| Klasse | Leser (vtable +0x28) | Größe |
|---|---|---|
| Basis aller Szenenobjekte | `FUN_0052cd90` | 0x108 |
| Primitiv (Kugel, Würfel, Kegel, Zylinder, Ebene, Platte, Quadrat, Kreis, Pyramide) | `FUN_00578390` (ruft Basis) | + 0x178 = **640** |
| `C3dGroupType` | `FUN_005501f0` | 640 + 4 = **644** |
| `C3dTorusType` | `0x5b8760` | 640 + 28 = **668** |
| `C3dBaseCamera` | `FUN_005311b0` | 640 + 168 = **808** |
| Material (`C3dBaseMaterial`) | `0x563e60` | 0x108 + 0x2c8 = **976** (§6.7) |
| Textur (`C3dTexture`) | `0x5a1120` (ruft `0x5c48b0`) | 0x108 + 0xEC + 0x440 = **1588** (§6.8) |
| Lichter | `0x546850` (Würfel), `0x53beb0` (Kegel), `0x58dd40` (Pyramide), `0x54cae0` (Zylinder), `0x55df80` (Kugel), `0x5a0550` (Sonne) | [offen] |

Alle Größen stimmen mit den Ressourcengrößen im Korpus überein.

### 6.4 Klassendatensatz: Basisteil (0x000–0x107)

| Offset | Typ | Feld |
|---|---|---|
| 0x000 | u32 | PC-Flag (§6.1) |
| 0x004 | u32 | im Korpus immer 1. [abgeleitet] Version |
| 0x008 | char[256] | Objektname, NUL-terminiert (= Ressourcenname) |

### 6.5 Klassendatensatz: Objektteil (0x108–0x27F, gemeinsam für Primitive, Gruppe, Torus, Kamera)

Offsets relativ zu 0x108:

| Offset | Typ | Feld |
|---|---|---|
| 0x00 | u32 | im Korpus immer 1 |
| 0x04 | u32 | [offen] |
| 0x08 | u32 | [offen] |
| 0x0C | u32 | [offen] |
| 0x10 | u32 | Identität des **Materials** (`C3dBaseMaterial`), 0 = keins (2080 von 3279 Objekten haben eins) |
| 0x14 | 12 × double | Matrix A (3×4). In 2783 von 3570 Objektdatensätzen (Szenen + `.obp`) die Einheitsmatrix mit Translation 0. [abgeleitet] Rotation/Ausrichtung |
| 0x74 | 12 × double | Matrix B (3×4). In Stichproben: Diagonale = Skalierung, Elemente 9–11 = Position (Kameras `top`/`left` … liegen bei ±500 auf ihrer Achse). [abgeleitet] Objekttransformation |
| 0xD4 | u32 | [offen] |
| 0xD8 | u32 | [offen] |
| 0xDC | 7 × u32 | Identitäten der Animationskanäle, Reihenfolge `Obj_eUnused`, `Obj_ePathT`, `Obj_ePosition`, `Obj_eOrient`, `Obj_eScale`, `Obj_eShear`, `Obj_eOrigin` (Namenstabelle in der EXE bei 0x629dc0). 0 = kein Kanal |
| 0xF8 | u32 | [offen] |
| 0xFC | 124 Byte | vom Leser nicht ausgewertet |

Unterklassen-Anhänge ab 0x280:

- **Gruppe** (+4): u32. Im Korpus die Identität einer `C3dRootObjList` (105×) oder `C3dObjTrackList` (18×). [offen] Bedeutung.
- **Torus** (+28): u32, double, double, u32[2]. [abgeleitet] Die doubles sind die Radien bzw. ihr Verhältnis.
- **Kamera** (+168):

| Offset ab 0x280 | Typ | Feld |
|---|---|---|
| 0x00 | u32 | [offen] |
| 0x04 | 3 × double | [abgeleitet] Kamerawinkel/-vektor |
| 0x1C | u32 | Flag (beim Laden „== 0“ ausgewertet) |
| 0x20 | u32 | Flag (dito) |
| 0x24 | 40 Byte | nicht ausgewertet |
| 0x4C | double | [offen] |
| 0x54 | 7 × double | [offen] (Kamerazustand, gespeichert in Objektfeldern 0xB30–0xB64) |
| 0x8C | 7 × u32 | Kanal-Identitäten: `Cam_eCraOrient`, `Cam_eCorpOrient` bzw. `Cam_eBank`, `Cam_eFOV`, `Cam_ePanX`, `Cam_ePanY`, `Cam_eZoomX`, `Cam_eZoomY` |

### 6.6 Animationskanäle (`Obj_e*`, `Cam_e*`, `Mat_e*`, `Sky_e*`, `SunL_e*`, `Btx_e*`, `HF_e*`, …)

| Offset | Typ | Feld |
|---|---|---|
| 0x00 | u32 | 1 (Version, wird geprüft) |
| 0x04 | u32 | PC-Flag |
| 0x08 | u32 | Anzahl Keys n |
| 0x0C | u32 | [offen] |
| 0x10 | u32 | Zusatzbytes am Ende |
| 0x14 | u32 | [offen] (in ein Kanalfeld übernommen) |
| 0x18 | 124 Byte | [offen] |
| 0x94 | n × K | Keys |
|  | Zusatz | kanalabhängig |

Prüfregel des Lesers und im Korpus ausnahmslos erfüllt (8854 Kanäle): `Größe = 0x94 + n·K + Zusatz`. Ein Key beginnt mit einem double (beim Tauschen als double behandelt, [abgeleitet] Zeitpunkt) und einem u32 an +8. Die Key-Größe K hängt vom Werttyp ab:

| K | Kanäle (Auswahl) |
|---|---|
| 48 | `Obj_ePathT`, `Mat_eRefract`, `Mat_eMatFlags`, `HF_eGradShift` |
| 52 | `Cam_eFOV`, `Cam_ePanX/Y`, `Cam_eZoomX/Y`, `Cam_eBank`, viele `Sky_e*` |
| 56 | Farben: `Sky_e*C`, `SunL_eDiscClr`, `SunL_eHaloClr`, `Mat_eVolC` |
| 68 | `Obj_eScale`, `Obj_eShear`, `Cam_eCraOrient`, `SunL_eSunVctr`, `Btx_eTrnScale/Offset` |
| 76/80 | `Obj_eOrient`, `Cam_eCorpOrient`, `Btx_eTrnRot` |
| 84 | `Mat_eDiffuse`, `Mat_eAmbient`, `Mat_eSpecular`, `Mat_eBump` … |
| 92 | `Obj_ePosition`, `Obj_eOrigin` |
| 100 | `Mat_eDiffuseC`, `Mat_eAmbientC`, `Mat_eSpecularC` … |

### 6.7 Material (`C3dBaseMaterial`, 976 Byte)

Quelle: Leser der Materialklasse über die vtable (Basis 0x6296f8, Slot +0x28 = Leser bei `0x563e60`, Slot +0x18 = Klassennamen-Getter `0x56d690`, FourCC-Getter `matr`). Ghidra legt diese vtable-Methoden nicht als Funktionen an, sie stehen deshalb nicht im Dekompilat und wurden per Disassembly gelesen. Das PC-Flag liegt wie bei den Objekten bei Offset 0. Das Material ist 0x108 (Basis) + 0x2c8 (Materialteil) Byte groß. Der Leser tauscht jedes Feld nach seiner Breite, wenn das Flag ≠ 1 ist, und legt für die Animationskanäle ein zweites Laufzeitobjekt an (0x1ec Byte).

Offsets relativ zu 0x108 („M“):

| Offset | Typ | Feld |
|---|---|---|
| M+0x000 | u32 | Version, immer 1 |
| M+0x004 | u32 | **Materialflags** (Laufzeitfeld 0x324). Häufig 0x10048, 0x10000, 0x48, 0. Wenn die Bits 8–10 (`& 0x700`) null sind, setzt der Leser die Halftone-Werte zurück (HTDensity 0, HTEdge 0, HTSharp 1.0, HTQuality 0.75) |
| M+0x008 | 5 × 0x38 | **Farbkanäle**, Aufbau siehe unten, in der Reihenfolge `DiffuseC`, `AmbientC`, `SpecularC`, `SpecExp`, `TransC` |
| M+0x120 | 8 × 0x28 | **Skalarkanäle**, in der Reihenfolge `Diffuse`, `Ambient`, `Specular`, `Trans`, `Reflect`, `Bump`, `Metallic`, `HTDensity` |
| M+0x260 | 3 × float | Volumenfarbe (`Mat_eVolC`), Standard 1,1,1 |
| M+0x26C | float | Brechungsindex (`Mat_eRefract`), z. B. 1.33 bei Wasser |
| M+0x270 | float | `Mat_eHTEdge`, Standard 0 |
| M+0x274 | float | `Mat_eHTSharp`, Standard 1.0 |
| M+0x278 | float | `Mat_eHTQuality`, Standard 0.75 |
| M+0x27C | 19 × u32 | Identitäten der Animationskanäle, 0 = kein Kanal (Reihenfolge unten) |

**Farbkanal (0x38 Byte):**

| Offset | Typ | Feld |
|---|---|---|
| +0x00 | 3 × (FourCC, u32) | bis zu drei **Texturverweise**: Klasse und Identität der Textur. Klasse `brtx` → Ressource `C3dTexture`, `pctx` → `CPictTexture`. 0/0 = leer. Auf dem PC stehen die Codes umgedreht (`xtrb`, `xtcp`) |
| +0x18 | u32 | Modus, Bitfeld. Beobachtet: 0, 0x2, 0x4, 0x10000000, 0x20000000, 0x40000000, 0x80000000. [offen] Bedeutung der Bits |
| +0x1C | 3 × float | Farbe RGB (0..1) |
| +0x28 | 12 Byte + u32 | [abgeleitet] unbenutzt. Der Leser kopiert die Bytes, die Werte sind aber Speicherreste: je Datei konstant, in Mac-Dateien oft dieselben Adressen (z. B. `0x02adffff` in 546 Materialien) |

**Skalarkanal (0x28 Byte):** wie der Farbkanal, aber nach den drei Texturverweisen (+0x00) folgen Modus (+0x18), **ein float** (+0x1C, Wert) und 8 Byte Speicherreste (+0x20).

**Kanalreihenfolge der 19 Identitäten** (M+0x27C, Slot 0…18): `Mat_eDiffuseC`, `Mat_eAmbientC`, `Mat_eSpecularC`, `Mat_eSpecExp`, `Mat_eTransC`, `Mat_eDiffuse`, `Mat_eAmbient`, `Mat_eSpecular`, `Mat_eTrans`, `Mat_eReflect`, `Mat_eBump`, `Mat_eMetallic`, `Mat_eHTDensity`, `Mat_eMatFlags`, `Mat_eVolC`, `Mat_eRefract`, `Mat_eHTEdge`, `Mat_eHTSharp`, `Mat_eHTQuality`. Slot 13 (`Mat_eMatFlags`) ist in allen 1054 PC-Materialien 0. Die Zuordnung ist [abgeleitet] aus dem Typ der Ressource, auf die jede Identität zeigt (alle belegten Slots lösen auf den passenden Typ auf; Kollisionen der 32-Bit-Identitäten zwischen Typen sind Zufall) und stimmt mit den 19 Namen der EXE überein. Die Farb- und Skalarblöcke folgen derselben Reihenfolge. Das bestätigen die ersten Keys der Kanäle (Farbe und Wert stimmen mit dem Block überein) und die Presets (Gold in `Metals.mat`: Diffus gelb, Specular, Reflect und Metallic = 1.0).

Querverweise: Die Identitäten der Texturverweise und Kanäle stehen im selben Container. `br3dump.py --check` prüft für jedes Material, dass alle Verweise auf eine Ressource des erwarteten Typs auflösen (alle 169 Dateien, beide Byte-Ordnungen, ohne Warnung). In `.mat`-Dateien tragen die Ressourcen `matprevcimage` (Vorschaubild) und `CPresetDescr` (Presetbeschreibung) **dieselbe Identität** wie ihr Material [beobachtet, Inhalt nicht untersucht].

### 6.8 Textur (`C3dTexture`, 1588 Byte) – teilweise

Quelle: Leser bei `0x5a1120` (vtable-Slot +0x28; FourCC-Getter `brtx`, `0x5b5800`), Tausch-Routinen `0x5a1700` ff. Die Beschreibung ist auf Regionenebene gesichert, die Einzelfelder der prozeduralen Teile sind größtenteils [offen]. Größe: 0x108 (Basis) + 0xEC (Texturkopf, Leser `0x5c48b0`) + 0x440 (prozeduraler Teil, Leser `0x5a1120`) = 0x634.

Texturkopf (ab 0x108, „H“, 0xEC Byte):

| Offset | Typ | Feld |
|---|---|---|
| H+0x00 | u32 | immer 1 |
| H+0x04 | u32 | [offen] 1 (485×) oder 0, 2, 6 |
| H+0x08 | u32 | [offen] 0 (639×) oder 1 |
| H+0x0C | 3 × double | **Skalierung** xyz. Standard 0.1875 bzw. 0.51 |
| H+0x24 | 3 × double | **Versatz** xyz, meist 0 |
| H+0x3C | 8 Byte | [offen] |
| H+0x44 | Unterstruktur | [offen], eigene Lese- und Tauschroutine (`0x5c4b40`) |
| H+0x5C | 4 × u32 | Identitäten der Kanäle `Btx_eMapping`, `Btx_eTrnScale` (beobachtet), `Btx_eTrnOffset`, `Btx_eTrnRot` (die letzten zwei [abgeleitet], im Korpus nie belegt) |

Prozeduraler Teil („T“, Dateioffset 0x1F4, 0x440 Byte). Typen aus dem Kopierteil des Lesers (`fild`/`fstp` = Ganzzahl in der Datei, float im Speicher). Offsets relativ zu T:

| Offset | Typ | Feld |
|---|---|---|
| T+0x00 | u32 | Version 1 |
| T+0x04 | 5 × u32 | `kind`: Werte [abgeleitet] kleine Codes. Feld 0 ∈ {1,2,3}, Feld 1 ∈ {2,3,6,7} (beobachtet), Feld 4 oft 1 oder 256 |
| T+0x18 | 6 × int32 | in float gewandelt. Die ersten drei sind Winkel in Grad (Standard 0,45,0), die letzten drei ein Wertetripel mit gleichen Werten (140, 360, 504, 643, −43, 20 …). [offen] Bedeutung |
| T+0x30 | 2 × u32 | Ganzzahlen |
| T+0x38 | 2 × float | Werte, z. B. 1.278 |
| T+0x40 | 5 × u32 | zwei Modi, dann **drei Farben** als 24-Bit-Wert (z. B. `ff0000`, `ffff00`, `0000ff`) |
| T+0x54 | 3 × 0x110 | **drei Komponentenblöcke** (T+0x54, T+0x164, T+0x274) |
| T+0x384 … T+0x43C | 0xB8 | Rest, [offen]: 1 u32, 7 u32, ein 72-Byte-Block (18 u32, kopiert über `0x5a1430`) mit zwei Nachzüglern, 15 u32 bei T+0x400 (kleine Ganzzahlen wie 6, 7, 3) und ein u32 bei T+0x43C |

Komponentenblock (0x110 Byte, Offsets relativ zum Blockanfang). Die drei Blöcke haben dieselbe Gestalt. [abgeleitet] Sie beschreiben orthogonale Vorschauansichten des Texturs (Winkel 0/90, Maßstab 62 bzw. 100 in den Presets), also zum Teil Editorzustand:

| Offset | Typ | Feld |
|---|---|---|
| +0x00 | 3 × u32 | Kopf. Feld 0 ∈ {1…7} |
| +0x0C | 5 × int32 | → float. Winkel (0, 90) und Maße |
| +0x20 | u32 | z. B. 256 |
| +0x24 | 6 × int32 | → float |
| +0x3C | u32 | Der Kopierer übernimmt die Bytes +0x3C bis +0xA7 unverändert |
| +0x54 | 3 × u32 | **drei Farben** (24 Bit), beobachtet |
| +0x60 … | doubles | 4-Byte-ausgerichtet, in der Datei bei +0x44 beginnend. Enthalten u. a. Werte wie ±62.0 und eine Matrix (±1.6597 …). Die Aufteilung in Einzelfelder ist [offen] |
| +0xB0 | 2 × u32 | Flags 0..3 |
| +0xB8 … +0xFF | doubles | [abgeleitet] 3×3-Matrix (9 doubles) |
| +0x108 | u32 | Flag 0..3 |
| +0x10C | u32 | [offen] |

**Korrektur:** In der ersten Fassung dieses Abschnitts stand ein Block „T+0x384, 4 doubles“ und die Komponentenfelder waren vermischt. Die Statistik damals lief über Dateioffsets, nicht über T-relative. Die obige Aufstellung ersetzt sie.

`br3dump.py` dekodiert den Texturkopf, die typisierten Parameter ab T und den Kopf, die Farben und die Flags der Komponenten. Der Rest bleibt unausgewertet.

### 6.9 Noch nicht entschlüsselt

[offen] Lichter, Terrain (`C3dHeightField`, 2584 bis 503 672 Byte), Dreiecksnetz, Pfad, Himmel (1344), Sonne (896), `CPictTexture`, `C3dCImage` (`rle2`), `brycedocset` (DataR-Eintrag), die Einzelfelder der Texturkomponenten und des Texturrests (§6.8), die Bedeutung der Modus-Bits der Materialkanäle sowie die Bedeutung der mit [offen] markierten Felder und der Key-Wertformate der Kanäle.

## 7. Fehlercodes von `CCmFile` (Auswahl)

| Code | Bedeutung (abgeleitet aus den Prüfungen) |
|---|---|
| 0 | OK |
| 1 | Verzeichnis-Deskriptor ungültig / interner Zustand |
| 2 | Formatname passt nicht |
| 4, 5 | Reservewort 0x220 bzw. 0x224 ≠ 0 |
| 8 | Kettenlänge ≠ Eintragsgröße / Puffer zu klein |
| 9 | Speicher für Verzeichnis nicht verfügbar |
| 0x0F | Datei nicht geöffnet |
| 0x1C | Eintrag ist bereits belegt bzw. bereits frei |
| 100 | Kennungs-String passt nicht oder Datei < 0x291 Byte |
| −0x2B | (von `CFile::Open`) Datei existiert nicht. `CResourceEnv` legt sie dann an, wenn Schreibrecht verlangt ist |

## 8. Anlegen einer neuen Datei (für spätere Schreibunterstützung)

[abgeleitet aus `CResourceEnv`-Konstruktor und `CCmFile::Create`] Beim Anlegen wird das Verzeichnis mit doppeltem Standardwert erzeugt (Kapazität 128, Wachstum 128). Danach werden die Einträge 0–15 belegt und die Master-Tabelle auf (16, 17) angelegt. Weitere Einträge werden über „ersten freien Eintrag benutzen“ vergeben. Deshalb folgen Ressourcentabelle und Ressourcen in Szenen paarweise aufeinander (z. B. Typtabelle 18/19, Ressource 20/21). [offen] Exakte Blockgrößen und Allokationsstrategie (`AllocateE`, `OptimizeEFree`) für bitgleiches Zurückschreiben.
