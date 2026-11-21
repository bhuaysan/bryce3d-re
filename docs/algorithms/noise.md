# CMath: Gitter-Noise, Zufallszahlen und Formfunktionen

Stand: 2026-11-21. Quelle: statische Analyse von `Axiom.dll` (SHA-256 `a7b5cef5…5d1d38`) und `Bryce 3D.exe` (SHA-256 `7cceb0d7…99ebe7`) mit Ghidra 12.2 über REA, ergänzt durch Disassembly zur Klärung von x87-Details.
Ausführbares Referenzmodell: [`tools/bryce_math.py`](../../tools/bryce_math.py).

Die Beschreibung ist eine eigene Formulierung des beobachteten Verhaltens. Sie enthält keinen Originalcode. Die einzige nicht ableitbare Konstante (`noise32_`, 256 Byte) steht **nicht** im Repo. Werkzeuge lesen sie zur Laufzeit aus der `Axiom.dll` des Nutzers.

Evidence: REA `decompile_all` Axiom.dll `ev_0ce71878fb83d786…`, Bryce 3D.exe `ev_cff33f09efffacba…` (lokal, nicht im Repo).

## 1. Überblick

| Gruppe | Funktionen | Rückgabe |
|---|---|---|
| Gitter-Noise | `uiunoise_`, `urunoise_`, `urgnoise_`, `srgnoise_` mit je 1, 2 oder 3 `unsigned long`-Argumenten | `unsigned long` 0…255 bzw. `float` |
| Zufall | `random_` (= `randomu_`, gleicher Code), `randomr_`, `randomlr_`, `uniform_` (1 und 2 Grenzen), `dice_`, `cointoss_`, Zustand in `CRndSeed` | `long`/`unsigned long`/`float`/`double` |
| Formfunktionen | `step_`, `pulse_`, `bstep_`, `smoothstep_`, `bias_`, `gain_`, `gamma_`, `gammai_` (= `pow_`), `sinc_` (4 Varianten), `log2_` | `float`/`double` |

Alle Funktionen sind statische `__cdecl`-Methoden. Fließkommawerte werden auf dem x87-Stack (`ST0`) zurückgegeben, siehe Abschnitt 6.

## 2. Tabellen

`CMath::Initialize()` baut beim Start der Anwendung (aufgerufen aus dem `CAppl`-Konstruktor) vier Tabellen im nicht initialisierten Datenbereich auf. Jede hat 256 Nutzeinträge plus 4 Wiederholungseinträge (Index 256…259 = Index 0…3):

| Tabelle | Typ | Inhalt |
|---|---|---|
| `uiun_` | `uint8[260]` | Permutation von 0…255 |
| `urun_` | `float[260]` | gleichverteilt, `uiun[i] / 255` |
| `urgn_` | `float[260]` | „gaußähnlich“ in [0, 1] (siehe unten) |
| `srgn_` | `float[260]` | `urgn[i] − 0.5`, also in [−0.5, 0.5] |

Einzige Eingabe ist die Konstante **`noise32_`**: 64 × `uint32` = 256 Byte in `.rdata` (VA `0x1009B860`, exportiert als `?noise32_@CMath@@0PBKB`). SHA-256 der 256 Byte: `1903d4cb4e18624d06724e2c67f88a72dafbc3256efea81f8f2fa21b4dd318b1`. Dieselbe Bytefolge liegt auch in `amberkrn.dll` (für `CAmberMath`).

Aufbau, in dieser Reihenfolge:

1. Für i = 0…255: `uiun[i] = i`. Mit `c = (float)(1/255)` (gespeichert als `0x3B808081`, also leicht **aufgerundet**) und `t = i·c` gilt
   `urgn[i] = float( 2t − (3 − 2t)·t² )`, also das Polynom 2t³ − 3t² + 2t.
   Das Polynom ist streng monoton, hat die Steigung 2 an den Rändern und 0.5 in der Mitte. Gleichverteilte Eingaben werden dadurch zur Mitte hin verdichtet. Daher kommt die „gaußähnliche“ Verteilung.
2. Mischen: Für i = 0…255 mit `j = noise32_byte[i]` (die 256 Byte von `noise32_` in Speicherreihenfolge, Little-Endian): `uiun[i]` ↔ `uiun[j]` **und** `urgn[i]` ↔ `urgn[j]` tauschen. Es ist dieselbe Vertauschungsfolge für beide Tabellen.
3. Für i = 0…255: `urun[i] = float(uiun[i] · c)`, `srgn[i] = float(urgn[i] − 0.5)`. `urun` folgt also der gemischten Permutation, `urgn`/`srgn` wurden mitgemischt.
4. Einträge 256…259 = Einträge 0…3 (alle vier Tabellen).

**Eigenheit durch Rundung:** Weil `c` größer als 1/255 ist, ergibt `t` für i = 255 den Wert 1.0000000592. Der größte `urgn`-Eintrag ist deshalb `0x3F800001` (1.0000001), nicht 1.0, und der größte `srgn`-Eintrag 0.5000001. Bei `urun` rundet derselbe Wert auf genau 1.0.

Kontroll-Hashes der erzeugten Tabellen (260 Einträge, Little-Endian float) aus dem Referenzmodell:

| Tabelle | SHA-256 |
|---|---|
| uiun | `b75f8176841c794d8b17b0b5fc5357dd7d4ebcf1e154905df2f125f55b23f2f6` |
| urun | `3a4c24e27bc8566df9e7550aec069aaa68723c0fcbadb667386ead26cb6d6f7b` |
| urgn | `6428704d7a686150b79e63b6392184c2e018532eb0fd1d94424308c2f9ceeede` |
| srgn | `4997ac2f6c821429bdbc204d4068b876b8aa59f831e91257d888ac53e048498d` |

[offen] Gegenprobe durch Speicherabzug der Tabellen aus dem laufenden Original unter Wine.

## 3. Gitter-Noise (Hash-Kette)

Die vier Noise-Familien unterscheiden sich nur in der Zieltabelle. Der Index entsteht durch eine Kette über `uiun`:

```
h = k0 & 0xFF
für jedes weitere Argument kn:  h = (uiun[h] + kn) & 0xFF
Ergebnis = Tabelle[h]
```

- 1 Argument: `T[k0 & 255]`
- 2 Argumente: `T[(uiun[k0 & 255] + k1) & 255]`
- 3 Argumente: `T[(uiun[(uiun[k0 & 255] + k1) & 255] + k2) & 255]`

Die Addition läuft in 32 Bit und wird danach maskiert. Nur die unteren 8 Bit jedes Arguments wirken sich aus. Das Gitter wiederholt sich daher mit Periode 256 je Achse. Die Wiederholungseinträge 256…259 werden von diesen Funktionen nicht erreicht. [abgeleitet] Sie sind für interpolierende Aufrufer gedacht, die `idx+1…+3` lesen (z. B. Spline-Interpolation, [offen] wer genau).

| Funktion | Tabelle | Wertebereich |
|---|---|---|
| `uiunoise_` | `uiun` | 0…255 (ganzzahlig) |
| `urunoise_` | `urun` | k/255, 0.0…1.0 |
| `urgnoise_` | `urgn` | 0.0…1.0000001 |
| `srgnoise_` | `srgn` | −0.5…0.5000001 |

Der Rückgabewert ist ein exakter float aus der Tabelle (`fld dword`). Diese Funktionen haben keine x87-Besonderheiten.

## 4. Zufallsgenerator `CRndSeed`

Zustand: drei `uint32` `{s, i, j}`. Konstruktor bzw. `SetSeed(x)` setzen `s = x, i = 1, j = 1`. Der globale Seed (`CMath::gs_`, erreichbar über `globalseed_()`) startet mit `s = 0, i = 1, j = 1`, `setseed_(x)` setzt ihn wie `SetSeed`.

Ein Schritt (identisch in allen Funktionen dieser Gruppe, mit `N = noise32_` als `uint32[64]`):

```
s ← s XOR N[i & 31] XOR N[32 + (j & 31)]
i ← i + 1
j ← j + s        (mit dem neuen s, modulo 2^32)
Ergebnis u = s
```

Abbildung auf Rückgabewerte (`R = 2^-32`, gespeichert als float `0x2F800000`):

| Funktion | Ergebnis | Bereich | Hinweis |
|---|---|---|---|
| `random_`, `randomu_` | `u` | uint32 | gleicher Maschinencode, zwei Exportnamen |
| `randomr_` | `R · (int32)u` | [−0.5, 0.5) | **vorzeichenbehaftet**; Rückgabetyp float, Wert aber ungerundet in ST0 |
| `randomlr_` | `(uint32)u · R` | [0, 1) | exakt, als double |
| `uniform_(a)` | `((uint32)u · R) · a` | [0, a) | |
| `uniform_(a, b)` | `(uint32)u · (R · (b − a)) + a` | [a, b) | genau diese Klammerung |
| `dice_(n)` | `u mod n` | 0…n−1 | vorzeichenlose Division; n = 0 löst eine Divisionsausnahme aus |
| `cointoss_(p)` | `R · (int32)u < p` | 0/1 | vergleicht mit dem **vorzeichenbehafteten** Wert aus [−0.5, 0.5): `p = 0` ergibt ≈ 50 %, `p ≥ 0.5` immer 1 |

[abgeleitet] `cointoss_` wirkt wie ein Fehler im Original: Gemeint war vermutlich eine Wahrscheinlichkeit p auf [0, 1). Für Bit-Treue muss das Verhalten trotzdem genau so nachgebaut werden.

Hinweis zur Dekompilierung: Ghidra zeigt bei `randomlr_`/`uniform_` eine Rundung auf float vor der Multiplikation. Die Assembly hat diese Rundung nicht: `u` wird als 64-Bit-Integer geladen (`fild qword`) und direkt multipliziert.

## 5. Formfunktionen

Bezeichnungen: `x` = Eingabe; float-Parameter werden exakt geladen.

| Funktion | Verhalten |
|---|---|
| `step_(x, e, a, b)` | `x < e ? a : b` (bei NaN: a) |
| `pulse_(x, e0, e1, a, b)` | `f = (x < e0) ? a : b`; Ergebnis `x < e1 ? f − a : f − b`. Für e0 ≤ e1 also `b − a` auf [e0, e1), sonst 0 |
| `bstep_(x, e0, e1, lo, hi)` | `v = (x − e0)/(e1 − e0)`, auf [lo, hi] begrenzt |
| `smoothstep_(x, e0, e1)` | `x < e0` → 0; `x ≥ e1` → 1; sonst `v = (x−e0)/(e1−e0)`, Ergebnis `((3 − 2v)·v)·v` |
| `gamma_(x, g)` | `pow(x, 1/g)` |
| `gammai_(x, g)` / `pow_` | `pow(x, g)` (ein Funktionskörper, zwei Exportnamen) |
| `bias_(x, b)` | `pow(x, log2_(b) · K)` |
| `gain_(x, g)` | `k = log2_(1 − g) · K`; `x < 0.5` → `0.5 · pow(2x, k)`; sonst `1 − 0.5 · pow(2 − 2x, k)` |
| `sinc_(x)` / `sinc_(x, k)` | `a = x·π` bzw. `(k·x)·π`; Ergebnis `sin(a)/a`. **Keine Sonderbehandlung für 0**: `sinc_(0)` liefert NaN |

`bias_`/`gain_` sind die bekannten Perlin-Funktionen, mit zwei Abweichungen:

- **K ist nicht −1**, sondern der double `0xBFEFFFFFFFFFFFFD` = −0.9999999999999997 (3 ulp neben −1). [abgeleitet] Das ist vermutlich ein vorausberechnetes `1/log2(0.5)`. Für Bit-Treue genau diesen Wert verwenden.
- Der Logarithmus kommt aus der eigenen `CMath::log2_` (unten), `pow` aus der msvcrt (`_CIpow`).

### `log2_(x)` (eigene Implementierung)

Tabellengestützt mit 64 Stützstellen:

1. Wenn nicht `x > 1e-307` (auch bei NaN und negativen x): Ergebnis −3.4e38 (float `0xFF7FC99E`).
2. Exponent `e = biased_exp − 1023`; Mantisse `m` ∈ [1, 2) durch Setzen des Exponentenfelds auf 0x3FF. Das Vorzeichenbit wird dabei gelöscht.
3. Index `k` = die obersten 6 Mantissenbits.
4. Zwei Tabellen mit je 64 doubles: `RCP[k] = 1/(1 + k/64)` und `LG[k] = log2(1 + k/64)`. Beide sind **bitgenau per Formel reproduzierbar** (`LG` korrekt gerundet). Die dritte Tabelle (`2^(k/64)`, für `exp2_`) ebenso. Es müssen also keine Originaldaten übernommen werden.
5. `r = RCP[k]·m − 1`, `r2 = r·r`
6. `p = (C0 − r·C1) + ((B0 − r·B1) + (A0 − r·A1)·r2)·r2`
7. Ergebnis `r·p + (e + LG[k])`, wobei `e + LG[k]` zuerst gebildet wird.

| Koeffizient | c0 | c1 |
|---|---|---|
| A | 0.2883070248990067 | 0.22949900023246153 |
| B | 0.48089833404997356 | 0.36067132973951144 |
| C | 1.442695040888937 | 0.7213475204127876 |

[abgeleitet] Die Koeffizienten bilden eine leicht angepasste Taylorreihe von log2(1+r) (C0 ≈ 1/ln 2, C1 ≈ 1/(2 ln 2) …). Die Abweichung zu `math.log2` liegt im Referenzmodell unter 1e−12.

## 6. float/double und x87-Verhalten

- **Präzisionssteuerung:** Weder `Axiom.dll` noch `Bryce 3D.exe` enthalten `fldcw` oder importieren `_controlfp`. [abgeleitet] Es gilt das Windows-Standard-Steuerwort `0x027F`: 53-Bit-Mantisse, Runden zum Nächsten, erweiterter Exponentenbereich. Zwischenergebnisse werden also wie IEEE-double gerundet. Gespeicherte floats werden erst beim `fstp dword` auf 24 Bit gerundet. Eine Neuimplementierung mit double-Arithmetik (SSE2) trifft das bitgenau, solange kein Über- oder Unterlauf im double-Bereich auftritt.
- **Ungerundete Rückgabewerte:** Funktionen mit Rückgabetyp `float` (`randomr_`, `smoothstep_`, `bstep_`, `pulse_`, `sinc_(float)`) lassen das Ergebnis mit 53-Bit-Genauigkeit in ST0. Ob es auf float gerundet wird, entscheidet der **Aufrufer**: beim Speichern ja, bei direkter Weiterrechnung nein. Für `randomr_` ist das relevant, weil `R·int32` bis zu 31 signifikante Bits hat. [offen] Pro Aufrufstelle im Renderer prüfen.
- **Transzendente Funktionen:** `sinc_` nutzt den x87-Befehl `fsin` (für |a| > 2^63 kein Ergebnis; bei großen Argumenten ungenaue Argumentreduktion). `pow` ist msvcrt `_CIpow` (x87 `fyl2x`/`f2xm1`). Bitgleichheit mit einer libm-`pow` ist **nicht** garantiert. [offen] Abweichungen messen; ggf. x87-`pow` nachbilden.
- **Vergleiche** laufen über `fcomp` + `fnstsw` + `test ah, 1/0x41`. Bei NaN ist C0 gesetzt, „<“ wird also als wahr gewertet (z. B. `step_(NaN, …)` → a, `smoothstep_(NaN, …)` → 0).

## 7. EXE gegen DLL

`Bryce 3D.exe` enthält eigene Kopien aller hier beschriebenen Funktionen (exportiert für Plugins, u. a. für `AVIExport.brm`). Ein instruktionsweiser Vergleich mit normalisierten Adressen ergibt:

- **Identisch** zu Axiom: alle Zufallsfunktionen, `smoothstep_`, `bias_`, `gain_`, `gamma_`, `gammai_`, `step_`, `pulse_`, `bstep_`, `split1_`/`split2_`, `sinc_`, Noise mit 1 Argument.
- **Semantisch identisch, anders codiert:** Noise mit 2 und 3 Argumenten. Die EXE ruft die Tabellenzugriffe (`uiun()`, `urun()` …) über die Importtabelle von `Axiom.dll` auf und hat eine andere Registerbelegung. `mulhw_` und `CAmberMath::sinc_(float,float)` haben vertauschte Operanden einer Multiplikation, das Ergebnis ist kommutativ gleich.
- **Keine eigenen Tabellen in der EXE:** `CMath::*` liest die Tabellen aus `Axiom.dll`, `CAmberMath::*` die aus `amberkrn.dll`. Der Linker hat gleiche Funktionen zusammengelegt (COMDAT folding). Deshalb tragen manche CMath-Kopien in Ghidra den Namen `CAmberMath::…`.
- [offen] `CAmberMath::Initialize` in `amberkrn.dll` gegen `CMath::Initialize` prüfen. `noise32_` ist dort byte-identisch vorhanden.

## 8. Offene Punkte

- [offen] Laufzeit-Gegenprobe der Tabellen und Testvektoren (Speicherabzug unter Wine oder Emulation von `CMath::Initialize`).
- [offen] Welche höheren Noise-Funktionen (Perlin/Value-Noise, fBm, Turbulenz) der Renderer auf diesen Bausteinen aufbaut. Sie liegen in der EXE, nicht in Axiom.
- [offen] Nutzung der Wiederholungseinträge und von `CMath::spline_`/`splineb`.
