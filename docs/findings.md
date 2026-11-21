# Befunde (Findings-Ledger)

Jede Aussage ist markiert als **[beobachtet]** (direkt aus Datei/Tool-Ausgabe), **[abgeleitet]** (Schlussfolgerung) oder **[offen]**.
REA-Evidence-IDs beziehen sich auf lokale Sessions und sind nicht im Repo gespeichert.

## Installationsarchiv

- [beobachtet] `data.1` auf der Windows-Seite der CD ist ein **InstallShield-3-Archiv** (Magic `0x8C655D13`), kein IS5-Cabinet. `unshield` 1.6.2 lehnt es ab.
- [beobachtet] Header: 93 Dateien, 4 Verzeichnisse (`prog`, `prog\Data`, `prog\Plugins`, `prog\Presets`). Archivgröße im Header = Dateigröße (28 777 445 Bytes, also ein Volume).
- [beobachtet] Einträge sind PKWARE-DCL-„implode“-komprimiert. `tools/is3extract.py` entpackt alle 93 Dateien mit exakt der im Header angegebenen Größe.

## Binaries (alle PE32, i386, Windows 4.00)

| Datei | Größe | Exporte | Rolle |
|---|---|---|---|
| `Bryce 3D.exe` | 2 357 760 | 69 | Hauptprogramm. Exportiert `CMath`/`CAmberMath`-Hilfsfunktionen für Plugins |
| `Axiom.dll` | 875 520 | 4468 | Plattformübergreifendes App-Framework (Mac/Win) |
| `amberkrn.dll` | 174 592 | 1112 | „Amber“ 3D-Echtzeit-Engine, Kernel |
| `amberd3d.dll` / `amberogl.dll` / `ambersgi.dll` / `ambers3d.dll` | 67–186 KB | 181–340 | Amber-Backends: Direct3D / OpenGL / SGI-OpenGL / Software(?) |
| `Plugins/*.bro` | | 1 (`BRIOPLUGINENTRY`) | Import-Plugins (OBJ, 3DS, DXF, 3DMF) |
| `Plugins/*.bri` | | 1 (`BRIMAGEPLUGINENTRY`) | Bild-Export (BMP, PS3) |
| `Plugins/AVIExport.brm` | | 39 | Movie-Export |

- [beobachtet] Alle Exporte sind **MSVC-dekorierte C++-Namen** und lassen sich mit `llvm-undname` vollständig auflösen (Klasse, Methode, Signatur).
- [beobachtet] Die EXE enthält **keine RTTI** (`.?AV`-Strings: 0). Klassennamen innerhalb der EXE müssen also aus den DLL-Schnittstellen, Strings und Serialisierungs-Tags abgeleitet werden.

### Win32-Abhängigkeiten der EXE (REA `inspect_pe_imports`, vollständig)

- [beobachtet] 938 Imports aus 11 DLLs: `Axiom.dll` 677, `amberkrn.dll` 62, USER32 80, MSVCRT 44, GDI32 39, KERNEL32 29, comdlg32 4, SHELL32 2, WINMM 1 (`timeGetTime`), AVIFIL32/MSVFW32 0 (nur deklariert).
- [abgeleitet] Die Oberfläche ist **selbst gezeichnet**: Es gibt nur einfache GDI-Primitive (`BitBlt`, `StretchDIBits`, `CreateCompatibleDC`, `LineTo`, `Ellipse` …) und keine Common Controls. Für eine native UI braucht man also kein Win32-Widget-Mapping, nur Blitting und Eingabe.
- [abgeleitet] Fast die gesamte Plattformschicht steckt in `Axiom.dll`. Eine Linux-Portierung muss im Kern die von der EXE genutzten 677 Axiom-Funktionen nachbilden.

### Axiom.dll – Klassen (Top nach Anzahl exportierter Member)

`CMath`, `CElement`, `CImage`, `CImElement`, `CSpoolMem`, `CAppl`, `CImVector`, `CTxElement`, `CWinAppl`, `CCoElement`, `CResourceEnv`, `CPaElement`, `COutlineFont`, `CFrImNavigator`, **`CCmFile`**, `CImVector64`, `CCanvas`, `CNavigator`, `CExSystem`, `CRmElement`, `CRect`, `CMemPool`, `CUniMotion`, `CTeCanvas`, `CFile`, `CSpoolSys`, `CCmSystem`, `CScanner`, Vektor/Matrix-Typen (`C2Vector` … `C44Matrix`), `CSpGradient`, `CRndSeed` u. v. m.

- [beobachtet] **`CCmFile`** ist der Container hinter dem `.br3`-Header `CCmF - Universal - Axiom - AGP - Composite File Management System`. Details unten unter „Dateiformat CCmF“.
- [beobachtet] **Korrektur:** `CCmSystem`, `CCmFunction`, `CCFEntry`, `CCFEntryParms`, `CCFEntryData` gehören **nicht** zum Dateiformat. Sie bilden ein Nachrichten- und Callback-System (`MessageTo*`, `CountBy*`, `Execute`, Heartbeat). Die frühere Annahme „CCmSystem/CCFEntry = Composite File“ war falsch.
- [abgeleitet] Präfix-Familien: `CIm*` Image, `CTx*`/`CIxText` Text, `CSg*`/`CSt*`/`CAc*` vermutlich Saver/Streams, `CSpool*` Spooling/Speicher, `CNavigator`/`CScanner` Serialisierungs-Traversal. **[offen]** Genaue Bedeutung.

### amberkrn.dll – Klassen

`CAmberMath`, `CContextBase`, `CRender`, `CTransform`, `CMatrix`, `COptMaterial`, `COptLight`, `COptNode`, `COptScene`, `COptCamera`, `COptGeometry`, `COptPrimitive`, `COptTexture`, `CQuaternion`, `CFrustum`, `COffscreenDIB`, `CTextureFile`.

- [abgeleitet] Amber ist ein Szenengraph mit Echtzeit-Renderer (für Vorschau/Wireframe). Es ist nicht der Raytracer, der Raytracer liegt in der EXE. **[offen]** verifizieren.

### Noise- und Mathe-Funktionen (wichtig für Phase 4)

- [beobachtet] `Bryce 3D.exe`, `Axiom.dll` und `AVIExport.brm` exportieren je `CMath::`/`CAmberMath::` `urgnoise_`, `srgnoise_`, `urunoise_`, `uiunoise_` (1–3 `unsigned long`-Argumente), dazu `bias_`, `gain_`, `gamma_`, `smoothstep_`, `pulse_`, `step_`, `bstep_`, `sinc_`, `split1_`/`split2_`, `random_`, `uniform_`, `dice_`, `cointoss_` mit `CRndSeed`.
- [abgeleitet] Das sind die Bausteine der prozeduralen Texturen. Sie sind mit Namen und Signatur auffindbar und daher gute erste Kandidaten für eine bit-genaue Re-Implementierung mit Unit-Tests.
- **Ausgewertet am 2026-11-21**, volle Beschreibung in [algorithms/noise.md](algorithms/noise.md), Referenzmodell `tools/bryce_math.py`. Evidence: Axiom `ev_0ce71878fb83d786…`, EXE `ev_cff33f09efffacba…` (REA `decompile_all`, Ghidra 12.2).
  - [beobachtet] Die Noise-Funktionen sind reine Tabellen-Lookups mit einer Hash-Kette über eine 256er-Permutation `uiun`: `T[(uiun[(uiun[k0&255]+k1)&255]+k2)&255]`. Die Tabellen `uiun`/`urun`/`urgn`/`srgn` (je 256 + 4 Wiederholungseinträge) baut `CMath::Initialize` (aus dem `CAppl`-Konstruktor) zur Laufzeit.
  - [beobachtet] Einzige nicht ableitbare Eingabe: `CMath::noise32_`, 256 Byte in `.rdata` (exportiert, SHA-256 `1903d4cb…dd318b1`). Sie liefert die Mischreihenfolge der Tabellen und die XOR-Konstanten des RNG. Byte-identisch auch in `amberkrn.dll`. Sie kommt nicht ins Repo, `tools/bryce_math.py` liest sie aus der Axiom.dll des Nutzers.
  - [beobachtet] Die 64er-Tabellen von `log2_`/`exp2_` lassen sich bitgenau per Formel erzeugen (`1/(1+k/64)`, korrekt gerundetes `log2(1+k/64)` und `2^(k/64)`).
  - [beobachtet] Eigenheiten: `1/255` ist als float aufgerundet gespeichert, deshalb ist max(`urgn`) = 1.0000001. Die bias/gain-Konstante ist −0.9999999999999997 statt −1. `cointoss_` vergleicht mit einem vorzeichenbehafteten Wert aus [−0.5, 0.5). `sinc_(0)` liefert NaN. `randomr_`/`randomlr_`/`uniform_` geben ungerundete x87-Werte zurück, die Ghidra-Dekompilierung zeigt fälschlich einen float-Cast.
  - [beobachtet] Weder Axiom noch EXE setzen das FPU-Steuerwort (kein `fldcw`, kein `_controlfp`). [abgeleitet] Es gilt der Windows-Standard (53-Bit-Präzision), double-Arithmetik reproduziert also bitgenau. `pow` kommt aus msvcrt `_CIpow`. [offen] Abweichung gegenüber libm-`pow`.
  - [beobachtet] Die gleichnamigen Exporte der EXE sind instruktionsgleich (Zufall, Formfunktionen, 1-Arg-Noise) oder semantisch gleich (2-/3-Arg-Noise mit anderer Registerbelegung). Die EXE hat keine eigenen Tabellen: `CMath::*` nutzt die Tabellen aus Axiom, `CAmberMath::*` die aus amberkrn.
  - [offen] Laufzeit-Gegenprobe der Tabellen (Speicherabzug unter Wine) und die darauf aufbauenden Perlin/fBm-Funktionen in der EXE.

## Dateiformat CCmF (`.br3` und alle Presets/UI-Daten)

Spezifikation: [formats/axiom-cmf.md](formats/axiom-cmf.md), Werkzeug `tools/br3dump.py`. Evidence: Axiom `ev_0ce71878fb83d786…` (`CCmFile::*`, `CResourceEnv::*`, `CMasterTable::StorageP`, `CResTypeTable::StorageP`, `CResourceEnv::Subscribe`).

- [beobachtet] Drei Schichten: (1) CCmF-Container mit 0x248-Byte-Header, 0x48-Byte-Verzeichnisdeskriptor, Verzeichnis aus 12-Byte-Einträgen und Freiliste. Jeder Eintrag ist eine Kette von Blöcken mit 12-Byte-Tag `{Kapazität inkl. Tag, Nutzbytes, next}`. (2) Ressourcenschicht: Master-Tabelle fest auf Einträgen (16, 17) → je Typ eine Tabelle aus 60-Byte-`CResEntry` (Identität, PersR, DataR, Name[32], 4 User-Felder) → Nutzdaten. Tabellen bestehen aus Datenblock plus 32-Byte-Kopf `{Elementgröße, Kapazität, Wachstum, Anzahl}`. (3) Bryce-Nutzdaten.
- [beobachtet] Schichten 1 und 2 sind **immer Big-Endian** (Mac-nativ, Axiom tauscht auf x86). Schicht 3 hat die **Bytereihenfolge des schreibenden Rechners, je Ressource**. Die Tutorials auf der CD sind PC-Dateien (LE), die Beispielszenen Mac-Dateien (BE). `Presets/Paths.obp` mischt beides.
- [beobachtet] Die „umgedrehten FourCCs“ (`rmacesab`) entstehen, weil Bryce Klasse/Unterklasse als native `uint32` schreibt. Sie stehen in den Objektlisten (`C3dRootObjList`, `C3dObjList`): Header `{1, Byte-Order-Kennung 0=BE/1=LE, n}` plus n × `{Klasse, Unterklasse, Objekt-Identität, Kindlisten-Identität}`. Daraus lässt sich der komplette Szenenbaum inkl. Gruppen rekonstruieren.
- [beobachtet] Ressourcentypen sind C++-Klassennamen (`C3dSphere`, `C3dBaseCamera`, `C3dTexture`, `CPictTexture` …) und Animationskanäle (`Obj_ePosition`, `Cam_eFOV`, `Mat_eDiffuseC` …). Damit sind die Bryce-Klassen in der EXE (die keine RTTI hat) benannt.
- [beobachtet] `br3dump.py --check` liest alle **169** CCmF-Dateien (54 `.br3`, 46 `.dat`, 36 `.mat`, 13 `.obp`, 11 `.bsk`, 7 `.brt`, 2 `.tme`) ohne Warnung. Die Blockketten überdecken jede Datei lückenlos bis zum Ende. Alle 3279 Objektlisten-Records der Szenen lösen sich auf eine Ressource auf.
- [abgeleitet] Die Identitäten sind Speicheradressen der Objekte zum Speicherzeitpunkt und dienen als Schlüssel für Querverweise.
- [offen] Inhalt der Objekt-, Material-, Textur-, Himmel-, Kamera- und Animationsdatensätze, `brycedocset`, UI-Ressourcen. Dafür braucht es die Serialisierungsroutinen in der EXE.

### Serialisierung in der EXE (2026-11-21)

Details in [formats/axiom-cmf.md](formats/axiom-cmf.md) §6.1–6.7. Evidence: EXE `decompile_all` `ev_cff33f09…`, Ghidra-Umbenennungen `ev_993482af…` (im Checkpoint gespeichert).

- [beobachtet] Ladepfad: `Doc_OpenFile` (0x424de0) unterscheidet nach Endung. `.brc` → `Doc_LoadBrc_Converting` (0x429680), das unbedingt Bytes tauscht und andere Offsets nutzt; sonst → `Doc_LoadBr3_Native` (0x429420) → `Scene_Load` (0x5296e0). `Scene_CreateObjectByFourCC` (0x529bc0) bildet (Klasse, Unterklasse) auf Fabrikfunktionen ab, inklusive im Korpus fehlender Codes `circ`, `pyra`, `symh` und Lichter `pyrm`, `cyln`, `sunl`.
- [beobachtet] Jede Klasse liest ihren Datensatz über eine virtuelle Methode (vtable +0x28), geschichtet: Basis 0x108 Byte (`C3dObject_ReadBase108`, 0x52cd90), Objektteil 0x178 (`C3dPrimitive_Read178`, 0x578390; geteilt von allen einfachen Primitiven), Anhänge Gruppe +4, Torus +28, Kamera +168. Daraus ergeben sich genau die Ressourcengrößen 640/644/668/808.
- [beobachtet] Byte-Order: Jeder Leser tauscht seine Felder mit korrekter Breite, wenn das PC-Flag nativ ≠ 1 ist. Klassendatensätze tragen das Flag an Offset 0, Listen und Animationskanäle an Offset 4 (dort Offset 0 = Version 1). Im Korpus ausnahmslos bestätigt.
- [beobachtet] Objektteil: Material-Identität (→ `C3dBaseMaterial`), zwei 3×4-double-Matrizen und 7 Kanal-Identitäten in der festen Reihenfolge `Obj_eUnused/PathT/Position/Orient/Scale/Shear/Origin`. Die Kamera hat zusätzlich 7 `Cam_e*`-Kanäle. Animationskanäle haben einen 0x94-Byte-Header mit n Keys; die Key-Größe (48–100 Byte) hängt vom Werttyp ab, `Größe = 0x94 + n·K + Zusatz` (8854/8854 Kanäle).
- [beobachtet] `br3dump.py` dekodiert damit alle 3570 Objekt- und 12 499 Kanaldatensätze aus Szenen und `.obp`-Presets und zeigt Position, Material und animierte Kanäle im Szenenbaum.
- [beobachtet] Die erste Ghidra-Analyse der EXE über MCP überschritt das Zeitlimit des Clients. Abhilfe: einmal per REA-CLI mit `REA_GHIDRA_CHECKPOINT_DIR` analysieren (`rea function <exe> --procedure …`, ~4 min). Danach öffnet MCP sofort aus dem Checkpoint.
- [offen] Lichter, Terrain, Dreiecksnetz, Pfad, Material, Textur, Himmel, Sonne, `brycedocset`, Key-Wertformate und die mit [offen] markierten Felder. Unklar ist auch, was `.brc` ist.

### Material und Textur (2026-11-21)

Details in [formats/axiom-cmf.md](formats/axiom-cmf.md) §6.7–6.8. Evidence: EXE `xrefs`/`search_bytes` auf die Klassennamen-Zeichenkette und FourCC-Getter (`ev_af686598…`, `ev_86827f51…`, `ev_590dc0f1…`), Leser per Disassembly.

- [beobachtet] Die vtable-Methoden von Material (Basis 0x6296f8) und Textur (Slot +0x28 bei 0x5a1120) hat Ghidra nicht als Funktionen angelegt, deshalb fehlen sie im Dekompilat. Zugang: Zeichenkette `C3dBaseMaterial` → Tabelle der Typnamen → Getter `0x56d690` (liefert FourCC `matr`) → `search_bytes` nach dem Getter-Zeiger → vtable → Leser. Für solche Klassen taugt `objdump -d` auf der EXE.
- [beobachtet] Material: Leser `0x563e60` (Basis 0x108 + 0x2c8 = 976). Layout: 5 Farbkanalblöcke à 0x38, 8 Skalarkanalblöcke à 0x28, Volumenfarbe, Brechung, 3 Halftone-Parameter, 19 Kanalidentitäten. Jeder Kanal hat bis zu 3 Texturverweise (FourCC `brtx`/`pctx` + Identität), ein Modus-Bitfeld und den Basiswert.
- [beobachtet] Alle Texturverweise und alle belegten Kanalidentitäten aller Materialdatensätze des Korpus (PC und Mac) lösen im selben Container auf eine Ressource des erwarteten Typs auf. `br3dump.py --check` prüft das und läuft über alle 169 Dateien ohne Warnung.
- [abgeleitet] Die Reihenfolge der 19 Kanäle und der Kanalblöcke folgt den `Mat_e*`-Typen, auf die die Identitäten zeigen. Bestätigt durch die Keywerte der Kanäle und die Presets (Gold: Metallic/Reflect/Specular = 1.0).
- [abgeleitet] Je Block liegen 12 bzw. 8 Byte Speicherreste, die Bryce beim Speichern mitschreibt (je Datei konstant, adressartig). Sie müssen beim Round-Trip erhalten, aber nicht interpretiert werden.
- [beobachtet] Textur: Leser `0x5a1120` (Basis 0x108 + Kopf 0xEC + 0x440 = 1588). Kopf: Skalierung (3 double), Versatz (3 double), 4 `Btx_e*`-Kanalplätze. Der prozedurale Teil enthält drei gleichartige Komponentenblöcke à 0x110 Byte. Die Ganzzahlen darin wandelt der Leser mit `fild` in float.
- [beobachtet] Texturparameter ab T (`kind`, Winkel, Farben, Modi) und die Farben, Flags und typisierten Köpfe der drei Komponentenblöcke sind dekodiert und werden mit `--check` ohne Warnung über alle 169 Dateien gelesen. Die Typen (Ganzzahl in der Datei, float im Speicher) stammen aus dem Kopierteil der Leser.
- [abgeleitet] Die drei Komponentenblöcke sind orthogonale Vorschauansichten des Texturs (Winkel 0/90, Maßstab). Sie enthalten damit zum Teil Editorzustand, der für den Renderer unwichtig sein dürfte.
- **Korrektur** (2026-11-21): Eine frühere Gliederung des Texturrests („T+0x384, 4 doubles“) beruhte auf Dateioffsets, die ich für T-relative gehalten hatte. Sie ist ersetzt (axiom-cmf.md §6.8).
- [offen] Einzelfelder der Texturkomponenten und des Texturkopfs (H+0x04, H+0x08, H+0x3C, H+0x44), Bedeutung der Modus-Bits, `CPictTexture`, `C3dCImage`, Inhalt von `matprevcimage` und `CPresetDescr`.

## Wine-Baseline

- [beobachtet] Wine 11.21 (Staging, WoW64), Prefix `~/.local/share/wineprefixes/bryce3d`. Die EXE startet direkt aus dem entpackten `prog/`-Ordner, **ohne Installer**.
- [beobachtet] Geladen werden alle Amber-Backends inklusive `DDRAW.dll`/`wined3d` und `opengl32`. Kein Absturz in 60 s.
- [beobachtet] Beim ersten Start wird `data\register.dat` gelesen und ein Dialog mit drei `EDIT`-Feldern erzeugt. [abgeleitet] Registrierungsdialog.
- [beobachtet] Optional gesucht, nicht vorhanden: `CornholioB3D` (vermutlich Entwickler-/Debug-Schalterdatei) und `MetaImage.dll`.
- [offen] Unter Wayland lieferte `xprop` kein Bryce-Fenster. Sichtprüfung durch den Nutzer steht aus.

## Werkzeug-Status

- [beobachtet] REA ist verbunden. PE-Imports/Exports laufen ohne Engine. Für die Dekompilierung lehnt REA **Ghidra 12.1.4** ab (benötigt ≥ 12.2).
- [beobachtet] 2026-11-21: `binary_session` meldet den Ghidra-Provider weiterhin als `unsupported_version`. Ursache: Die REA-Instanz dieser Claude-Desktop-Session stammt aus `~/.config/Claude/claude_desktop_config.json` (Client `local-agent-mode-rea`, `GHIDRA_INSTALL_DIR=/home/ben/Tools/ghidra` → Symlink auf 12.1.4). Die lokale Projektregistrierung (`claude mcp get rea`: Scope local, `GHIDRA_INSTALL_DIR=…/ghidra_12.2_PUBLIC`, `GHIDRA_HEADLESS_MAXMEM=8G`, Status Connected) ist korrekt, wird von der Desktop-Session aber nicht verwendet. Alle laufenden `rea mcp`-Prozesse haben die Symlink-Umgebung.
- [beobachtet] Zuordnung geprüft: Der Server dieser Session (Start 17:33:35) ist PID 512189, gestartet von `claude-desktop` aus `claude_desktop_config.json`. bloodborne-linux startet zusätzlich eigene Server aus seiner `.mcp.json` (`rea-agents@6.8.0`, Symlink).
- [beobachtet] 2026-11-21, mit Freigabe des Nutzers: In `claude_desktop_config.json` steht `rea` jetzt auf `GHIDRA_INSTALL_DIR=…/ghidra_12.2_PUBLIC`, `GHIDRA_HEADLESS_MAXMEM=8G` (Sicherung `claude_desktop_config.json.bak-2026-11-21-rea`). Der Symlink ist unverändert. Nach dem Neustart der Desktop-App meldet `binary_session` Ghidra als `available`. `open_binary` auf Axiom.dll mit `provider_id: ghidra` und `procedure_pseudo_code` (CMath::urgnoise_, 0x10002850) funktionieren über MCP (Evidence `ev_aea59de16a91595c…`). Die User-Scope-Registrierung in `~/.claude.json` zeigt weiterhin auf den Symlink, wird von dieser Session aber nicht gebraucht.
- [beobachtet] 2026-11-21: `REA_GHIDRA_CHECKPOINT_DIR=/home/ben/Projects/bryce3D/extracted/ghidra-checkpoints` (Modus 700, git-ignoriert) in `claude_desktop_config.json` eingetragen (Sicherung `…bak-2026-11-21-checkpoint`). Ghidra-Metadaten wie Umbenennungen und Kommentare werden damit beim Schließen gesichert und beim Öffnen wiederhergestellt. Nach dem Neustart verifiziert: `binary_session` meldet „checkpoint mode is enabled“. Rundlauf getestet: `FUN_10028600` in `CMasterTable_ConstructFromCreation` umbenannt (`ev_52a3ad3f…`), geschlossen, neu geöffnet. Der Name war wiederhergestellt (`ev_f69cc428…`), Auto-Analyse lief nicht erneut. Kommentare (`set_comment`) bietet der Ghidra-Provider nicht an. Namen gehen, Kommentare nur über `annotate_native_function` am Funktionsanfang.
- [beobachtet] Umgehung ohne Konfigurationsänderung: REA-CLI mit `GHIDRA_INSTALL_DIR=~/Tools/ghidra_12.2_PUBLIC` und `rea decompile-all`. Axiom.dll: 3685 Funktionen (3553 geschrieben, 132 extern, 0 Fehler, ~100 s). EXE: 4434 Funktionen (3524 geschrieben, 909 extern, 1 Fehler). Ausgabe unter `extracted/decomp/{axiom,exe}/` (git-ignoriert), mit Journal (Name → Adresse).
