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

- [abgeleitet] **`CCmFile` / `CCmSystem` / `CCFEntry*` = „Composite File Management“**, also der Container hinter dem `.br3`-Header `CCmF - Universal - Axiom - AGP - Composite File Management System`.
- [abgeleitet] Präfix-Familien: `CIm*` Image, `CTx*`/`CIxText` Text, `CSg*`/`CSt*`/`CAc*` vermutlich Saver/Streams, `CSpool*` Spooling/Speicher, `CNavigator`/`CScanner` Serialisierungs-Traversal. **[offen]** Genaue Bedeutung.

### amberkrn.dll – Klassen

`CAmberMath`, `CContextBase`, `CRender`, `CTransform`, `CMatrix`, `COptMaterial`, `COptLight`, `COptNode`, `COptScene`, `COptCamera`, `COptGeometry`, `COptPrimitive`, `COptTexture`, `CQuaternion`, `CFrustum`, `COffscreenDIB`, `CTextureFile`.

- [abgeleitet] Amber ist ein Szenengraph mit Echtzeit-Renderer (für Vorschau/Wireframe). Es ist nicht der Raytracer, der Raytracer liegt in der EXE. **[offen]** verifizieren.

### Noise- und Mathe-Funktionen (wichtig für Phase 4)

- [beobachtet] `Bryce 3D.exe`, `Axiom.dll` und `AVIExport.brm` exportieren je `CMath::`/`CAmberMath::` `urgnoise_`, `srgnoise_`, `urunoise_`, `uiunoise_` (1–3 `unsigned long`-Argumente), dazu `bias_`, `gain_`, `gamma_`, `smoothstep_`, `pulse_`, `step_`, `bstep_`, `sinc_`, `split1_`/`split2_`, `random_`, `uniform_`, `dice_`, `cointoss_` mit `CRndSeed`.
- [abgeleitet] Das sind die Bausteine der prozeduralen Texturen. Sie sind mit Namen und Signatur auffindbar und daher gute erste Kandidaten für eine bit-genaue Re-Implementierung mit Unit-Tests.

## Wine-Baseline

- [beobachtet] Wine 11.21 (Staging, WoW64), Prefix `~/.local/share/wineprefixes/bryce3d`. Die EXE startet direkt aus dem entpackten `prog/`-Ordner, **ohne Installer**.
- [beobachtet] Geladen werden alle Amber-Backends inklusive `DDRAW.dll`/`wined3d` und `opengl32`. Kein Absturz in 60 s.
- [beobachtet] Beim ersten Start wird `data\register.dat` gelesen und ein Dialog mit drei `EDIT`-Feldern erzeugt. [abgeleitet] Registrierungsdialog.
- [beobachtet] Optional gesucht, nicht vorhanden: `CornholioB3D` (vermutlich Entwickler-/Debug-Schalterdatei) und `MetaImage.dll`.
- [offen] Unter Wayland lieferte `xprop` kein Bryce-Fenster. Sichtprüfung durch den Nutzer steht aus.

## Werkzeug-Status

- [beobachtet] REA ist verbunden. PE-Imports/Exports laufen ohne Engine. Für die Dekompilierung lehnt REA **Ghidra 12.1.4** ab (benötigt ≥ 12.2).
