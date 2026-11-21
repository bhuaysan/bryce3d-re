# START – Übergabe

Stand: 2026-11-21. Nur der aktuelle Stand, keine Historie (die steht in `git log` und `docs/findings.md`). Phasen und Entscheidungen: [PLAN.md](PLAN.md) (offen: UI-Toolkit, nachfragen).

## Stand
- Phase 0 fertig (Extraktion, Dekompilat lokal in `extracted/decomp/`).
- Formate: Container, Objekte, Kameras, Gruppen, Torus, Animationskanal-Header, Material komplett, Textur teilweise (Kopf, Parameter, Komponentenköpfe). Spezifikation: `docs/formats/axiom-cmf.md` §6.
- Regressionstest grün (Befehl in CLAUDE.md).
- Rust: Workspace + `libbryce-io` (Container, Ressourcen, Objektlisten/Szenenbaum mit Typauflösung über die Subklasse, Objekt-/Kamera-/Gruppen-/Torus-/Material-/Textur-Records, Kanal-Header, 7 synthetische Tests). `br3check` ist zeilengleich zu `br3dump.py --check` auf allen 169 CCmF-Dateien; Texturrecords stimmen mit Python über alle 5277 Texturen überein. Keine Wine-Baseline.

## Als Nächstes
1. Phase 1: Registrierungsdialog unter Wine, Golden Renders der 54 Szenen.
2. Textur-Rest (Doubles/Matrizen der Komponenten, T+0x384–0x43C), danach Terrain, Lichter, Himmel, Sonne, Dreiecksnetz, Pfad, `CPictTexture`, `C3dCImage`.
3. Noise: Tabellen aus laufendem Bryce unter Wine gegen `tools/bryce_math.py` prüfen, höhere Funktionen (Perlin/fBm/Turbulenz) in der EXE lokalisieren.

## Stolpersteine
- vtable-only-Methoden fehlen im Ghidra-Dekompilat. Weg: Typname → Getter → `search_bytes` auf den Zeiger → vtable → `objdump -d` auf der EXE.
- Offsets in Statistiken sind Dateioffsets, nicht klassenrelativ.
