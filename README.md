# Generátor rozvrhů – Gymnázium Františka Palackého, Neratovice

**Kompletní dokumentace projektu** – konsoliduje celou konverzaci (zadání, analýzu, architekturu,
všechny verze programu, otevřené otázky a plán do budoucna).

- Verze dokumentu: 1.0
- Stav aplikace: Rust desktop v0.4 (viz kap. 4 – Verze)
- Autor konzultací: AI asistent; zadavatel: vyučující GFP

---

## Obsah

1. [Zadání a požadavky](#1-zadání-a-požadavky)
2. [Analýza školy – data odvozená z rozvrhu 2026/27](#2-analýza-školy)
3. [Klíčová zjištění a problémy](#3-klíčová-zjištění)
4. [Verze programu a historie vývoje](#4-verze)
5. [Architektura aplikace](#5-architektura)
6. [Datový model](#6-datový-model)
7. [Řešič – pravidla a omezení](#7-řešič)
8. [Uživatelská příručka](#8-uživatelská-příručka)
9. [Instalace a spuštění](#9-instalace-a-spuštění)
10. [Exporty](#10-exporty)
11. [Co je hotové × co chybí](#11-stav)
12. [Otevřené otázky (nutno doplnit zadavatelem)](#12-otevřené-otázky)
13. [Roadmapa](#13-roadmapa)
14. [Technické poznámky pro vývojáře](#14-technické-poznámky-pro-vývojáře)
15. [Příloha: inventář zdrojových souborů](#15-inventář-zdrojových-souborů)

---

## 1. Zadání a požadavky

### 1.1 Původní požadavky (č. 1–4)

**P1 – Omezení ročníků** (raná = 7:20, odpolední = 12:55 a později):

| Ročník | Max raných hodin | Max odpoledních hodin |
|---|---|---|
| prima–tercie | 0 | 0 |
| kvarta | 1 | 1 |
| kvinta (+ první 4leté) | 2 | 2 |
| sexta, septima, oktáva (+ 2.,3.,4. 4leté) | bez limitu | bez limitu („ať to dává smysl“) |

Interpretace limitů (za den vs. za týden) je konfigurovatelná; default = za den,
s auto-uvolněním (viz 3.1).

**P2 – Učebny**: každá třída má kmenovou učebnu + odborné pracovny s pravidly
(viz 2.2).

**P3 – Skupiny a volitelné předměty**:
- třídy se dělí na **AJa/AJb** (angličtina, aj. rozdělené předměty);
- volitelné jazyky (ŠJ/FJ/NJ) a **semináře spojují paralelní třídy stejného
  ročníku → musí být ve stejném čase** (aby se skupiny mohly míchat napříč třídami).

**P4 – Učitelé**: primárně učí v odborných učebnách svého předmětu; někteří mají
preferovanou učebnu (volitelné pole).

### 1.2 Rozšířené požadavky (desktopová Rust aplikace)

Aplikace s tlačítkem **„✚ Vytvořit nový rok“** a průvodcem:

1. **Noví žáci** – seznam jmen (řádek = žák), automatické rozdělení **AJa/AJb 15:15**
   (s ručními úpravami; později i vyrovnaným poměrem pohlaví – vylepšení č. 15).
2. **Změny v seznamu žáků** – zápis odchodu / propadnutí (propadlý zůstává ve třídě,
   odchozí se vyřadí, maturanti odcházejí automaticky).
3. **Volitelné předměty** (přiřazování studentů, možnost importu z předchozího roku):
   - a) **sekunda**: informatika NEBO čeština
   - b) **tercie**: 3. jazyk – španělština / němčina / francouzština (míchání tříd)
   - c) **kvinta**: výtvarka NEBO dramatka
   - d) **sexta**: výtvarka / dramatka / hudebka
   - e) **septima**: 3 volitelné semináře × 2 vyučovací hodiny (míchání 2 tříd
     ročníku, katalog importovatelný z předchozího roku)
   - f) **oktáva**: 6 volitelných seminářů × 2 hodiny (dto.)

### 1.3 Dodatečné pravidlo

**P5 – Pravidlo 7 hodin**: žák nesmí mít více než **7 vyučovacích hodin v kuse**
(bez pauzy). 7 hodin za sebou je přípustné, poté nutná pauza ≥ 1 hodina
(konstanta `MAX_RUN = 7` v `data.rs`; tvrdé omezení, kontrolované pro třídu
i přesně pro každého studenta).

---

## 2. Analýza školy

*(vše odvozeno z „rozvrh-tridy-predbezny-2026-08-15_01.pdf“ – předběžný rozvrh
2026/27; položky s ⚠ je nutno ověřit)*

### 2.1 Třídy

16 tříd v PDF (+ chybějící 4A „čtvrtá“ 4letá – v PDF není, doplněna):

| ID | Název | Ročník | Typ | Kmenová | Následník |
|---|---|---|---|---|---|
| prima | prima | 1 | 8leté | 1G | sekunda |
| sekunda | sekunda | 2 | 8leté | 2G | tercie |
| tercie | tercie | 3 | 8leté | 3G | kvartaA |
| kvartaA/B | kvarta A/B | 4 | 8leté | kvartaA/B | kvintaA/B |
| kvintaA/B | kvinta A/B | 5 | 8leté | kvintaA/B | sextaA/B |
| sextaA/B | sexta A/B | 6 | 8leté | sextaA/B | septimaA/B |
| septimaA/B | septima A/B | 7 | 8leté | septimaA/B | oktavaA/B |
| oktavaA/B | oktáva A/B | 8 | 8leté | oktavaA/B | – (maturita) |
| 1A | první A | 1 | 4leté | 1A | 2A |
| 2A | druhá A | 2 | 4leté | 2A | 3A |
| 3A | třetí A | 3 | 4leté | 3A | 4A |
| 4A | čtvrtá A | 4 | 4leté | 4A | – (maturita) |

Efektivní ročník (pro omezení a volitelné): 1A→5, 2A→6, 3A→7, 4A→8.
Velikosti tříd ⚠ (odhad 22–30).

### 2.2 Místnosti (37)

Kmenové (kap. 30, jakýkoliv předmět): `1G, 2G, 3G, kvartaA…4A` (podle tříd).

Odborné:

| ID | Název | Kap. | Pravidlo |
|---|---|---|---|
| PSV | pracovna společenských věd | 30 | humanitní předměty (i kmenová) |
| LCH | laboratoř chemie | 24 | **pouze** PCCH, SPCH |
| PCH | posluchárna chemie | 28 | CHE, BIO, SCHE, SBCH, PCBI |
| PŠJ | pracovna španělštiny | 20 | **pouze** španělština |
| PAJ | pracovna angličtiny | **17** | AJ (+ další jazyky) |
| PNJ | pracovna němčiny | 24 | NJ + AJ |
| PFJ | pracovna francouzštiny | **17** | FJ, LAT |
| VJP | velká jazyková pracovna | 24 | jakýkoliv jazyk |
| MJP | malá jazyková pracovna | **14** | jakýkoliv jazyk |
| PFB | posluchárna fyziky a biologie | 28 | FY, BIO (i CHE) |
| PVV | pracovna výtvarné výchovy | 28 | **pouze** VV |
| LFY | laboratoř fyziky | 24 | **pouze** PCFY |
| PIF | pracovna informatiky | 24 | **pouze** INF, PRG, SEK |
| AULA | aula | 80 | DV, EV |
| HAL1/HAL2 | haly | 30 | **pouze** TV |
| BAZ1–4 | bazén | 20 | **pouze** PL |
| PHV | pracovna hudební výchovy | 28 | **pouze** HV |
| DisV | distanční výuka | 8 | IS, AS |

### 2.3 Učitelé (42 zkratek)

Zkratky z rozvrhu: SOJ, ŠAF, SLO, HAM, UHR, PAŘ, LIP, ŠMK, TES, ČER, TRO, VAI,
HOR, SAU, FID, PAL, ŠMD, LEB, ŠVE, CHOU, BER, VYS, LIH, KOŠ, VEJ, PALB, NEŠ,
SKŘ, PET, HRK, JIN, MAN, SMO, FAL, SCHO, PAV, BRD, SVI, WIE, ČEM, FOG, VAIS.

⚠ Jména známe jen u třídních (Sojková, Sloupová, Hamerníková, Uhrinčaťová,
Pařík, Lipovská, Smolková, Tesař, Červenková, Trojanová, Vaisová, Horáková,
Šafránková, Sauerová, Fidlerová); ostatní `(doplnit)` – seznam na gfp.cz/zamestnanci
(není dostupný z konzultace, nutno ručně).

### 2.4 Časová mřížka

12 slotů/den, 5 dní: `7:20, 8:10, 9:00, 10:00, 10:55, 12:00, 12:55, 13:45,
14:35, 15:25, 16:15, 17:05`. Odpolední = slot ≥ 6 (12:55+), konfigurovatelné.

### 2.5 Struktura rozvrhu (poznatky z PDF)

- **AJa/AJb** – angličtinu (a část dalších předmětů v nižších ročnících) učí
  paralelně 2 učitelé dvě poloviny třídy; v oktávě se AJ dělí na 4 skupiny
  napříč 8A+8B.
- **TV/PL** – rozdělení dívky/chlapci (2 učitelé, HAL1/HAL2, BAZ1–4).
- **Volitelné jazyky (NJ2/ŠJ2/FJ2)** – běží od tercie dál; skupiny různých tříd
  nemusejí mít stejného učitele (4A: UHR×WIE; 5B: UHR; 1A: UHR+KOŠ …), sladění
  slotů napříč třídami je požadavek, ne fakt z PDF.
- **Semináře septimy/oktávy** – 2hodinové bloky; skupiny slučující více tříd
  poznatelné podle společného učitele (SPP PAL, SFY SCHO, SČD HOR, SFAV LIP,
  SLR FID, SBR FOG …).
- **L/S (lichý/sudý týden)** – DV↔VV, PCBI↔PCFY/PCCH, HV↔VV sdílejí slot
  a střídají se po týdnech.
- **DV/VV v primě–tercii** – 2 sloty; praktika (PCBI/PCFY/PCCH) 1 slot.

### 2.6 Učební plán (odhady ⚠)

Kompletní plán je v `data.rs → vychozi()` (≈186 položek). Příklady:
prima ≈ 28–30 h (ČJL 4, MAT 4, AJ 4, ZE 2, BIO 2, DĚ 2, TV 2, DV/VV 2,
PCBI 1, HV 1, INF 1, LAT –, OV 1, FY 1, PL 1); septima ≈ 34 h vč. seminářů.
**Přesné dotace nutno ověřit proti učebnímu plánu školy.**

---

## 3. Klíčová zjištění

### 3.1 ⛔ Fundamentální konflikt zadání (P1 vs. dotace)

Bez 7:20 a bez odpoledních mají prima–tercie k dispozici **25 slotů/týden**
(8:10–12:45 × 5 dní). Skutečná potřeba:
prima ≈ 28, sekunda ≈ 28, tercie ≈ 30, kvarta ≈ 33–34 h.

→ **Přísná pravidla P1 jsou s aktuálními dotacemi matematicky neřešitelná.**

Řešení v aplikaci: **auto-uvolnění** – solver spočítá potřebu per student,
minimálně zvýší odpolední limit a **vypíše to do Reportu** jako varování
(„AUTO-UVOLNĚNO prima: 25 → 28…“). Trvalé řešení je buď povolit 12:55,
nebo snížit hodinové dotace. *L/S střídavé týdny ušetří jen ~0–2 sloty/třídu,
problém neřeší.*

### 3.2 Ostatní zjištění

- L/S = lichý/sudý týden (potvrzeno strukturou PDF).
- 4leté třídy mají vyšší dotaci 2. jazyka (4 h vs 3 h u 8letých).
- Jména učitelů nejsou ve zkratkách – nutno doplnit ručně.
- Čtvrtá A v PDF chybí – přidána do modelu.
- Bakaláři: importní formát je proprietární a verzovaný – k přímému exportu
  potřebujeme vzorový soubor z instalace školy.

---

## 4. Verze

| Verze | Forma | Obsah | Kde v konverzaci |
|---|---|---|---|
| v0.1 | Python + OR-Tools (CP-SAT) | referenční model omezení, diagnostika slotů, auto-uvolnění, HTML export | zpráva 2 |
| v0.2 | Rust + egui desktop | datový model, solver (greedy + annealing), průvodce „Vytvořit nový rok“ (noví žáci 15:15, změny, volitelná), P5 pravidlo 7 h, učební plán/číselníky/nastavení | zpráva 3 |
| v0.3 | Rust + `validation.rs` | interaktivní úpravy rozvrhu (klik ⇥, Ctrl+Z), okamžitá validace, režim „student“, export CSV/XML | zpráva 4 |
| v0.4 | Rust | **L/S střídavé týdny, 📌 piny, kapacity+✂ rozdělení skupin, osobní rozvrhy žáků, zátěže učitelů, tisk celé školy, slepování bloků + semináře dopoledne, 15:15 dle pohlaví** | zpráva 5 |

Kompletní zdrojové kódy jsou v odpovídajících zprávách konverzace
(inventář viz kap. 15).

---

## 5. Architektura

```
┌───────────────────────────── aplikace (eframe/egui) ─────────────────────┐
│  hlavní panel: Rozvrh | Studenti | Volitelné | Plán | Číselníky |         │
│                Nastavení | Report      + „✚ Vytvořit nový rok…“          │
│  průvodce (Window): 0 rok → 1 noví žáci → 2 změny → 3 volitelné → 4 fin. │
├─────────────────────────────── data (JSON) ──────────────────────────────┤
│  skola_data.json: { skola, rok, historie[], piny{} }  (autosave)        │
├─────────────────────────────── solver (vlákno) ─────────────────────────┤
│  lekce → jednotky (sync) → konflikty (bitset) → greedy → annealing      │
│  → slep_bloky → room polish → report                                     │
└───────────────────────────────────────────────────────────────────────────┘
```

- **GUI**: eframe/egui 0.27 (desktop, jediný binary). Řešič běží ve vlákně,
  UI dotazuje stav přes `Arc<Mutex<…>>` + `request_repaint_after`.
- **Řešič**: vlastní (bez externího solveru) – viz kap. 7.
- **Perzistence**: `serde_json`, soubor `skola_data.json` v cwd; ukládání při
  zavření + tlačítkem; otevírání přes rfd dialog.
- **Historie let**: minulé `SkolniRok`y → zdroj pro import voleb a katalogů.

---

## 6. Datový model

### 6.1 `Skola` (statická, číselníky + plán)

- `Trida { id, nazev, rocnik, typ, kmenova, naslednik, omezeni_prepsat }`
- `Mistnost { id, nazev, kapacita }`
- `Ucitel { id, jmeno, volno[], max_den, preferovana_mistnost }`
- `Predmet { id, nazev, specialni_mistnosti[], kmenova_ok }`
- `PlanovaHodina { trida, predmet, hodin, struktura, dvoj }`
  - `Struktura`:
    - `CelouTridou(učitel)`
    - `Rozdeleno { a, b }` – AJa/AJb
    - `Pohlavi { div, chl }` – TV/PL (vynuceně paralelní slot)
    - `Stridave { l: Pol, s: Pol }` – L/S celou třídou
    - `StridaveSkupiny { a: [Pol;2], b: [Pol;2] }` – L/S per AJa/AJb
    - `Pol { predmet, ucitel }`

### 6.2 `SkolniRok` (roční data)

- `label`, `studenti[]`, `menu[]` (katalogy voleb per rok), `dalsi_id`
- `Student { id, jmeno, trida, skupina_aj, pohlavi, volby{menu→[volby]}, status }`
- `Menu { id, nazev, tridy[], volby[], hodin_tydne, dvojhodina, pocet_voleb }`
- `Volba { id, nazev, predmet, ucitel }`

### 6.3 `Vysledek` (vygenerovaný rozvrh)

- `lekce[]` (LekceInfo: predmet, ucitel, tridy, skupina, zaci, len, kandidati,
  **klic** – stabilní ID pro piny, **par** – partner L/S, **dopoledne** – flag)
- `slot[]` (den*12+slot), `mistnost[]`, `varovani[]`, `problemy[]`,
  `problemove_lekce[]`, `skore (tvrde, mekke)`, `iteraci`

### 6.4 Menu voleb (seed, mapuje zadání a–f)

| menu | třídy | volby | h/týden | 2h | voleb/žák |
|---|---|---|---|---|---|
| sek-volba | sekunda | INF / ČJL | 2 | ne | 1 |
| 3j-3…3j-8 | ročník 3–8 (vč. 4letých) | NJ2/ŠJ2/FJ2 | 3 | ne | 1 |
| vol-5 | kvintaA/B, 1A | VV2 / DV2 | 2 | ano | 1 |
| vol-6 | sextaA/B, 2A | VV2 / DV2 / HV2 | 2 | ano | 1 |
| sem-7 | septimaA/B, 3A | 14 seminářů | 2 | ano | **3** |
| sem-8 | oktavaA/B, 4A | 15 seminářů | 2 | ano | **6** |

Pravidlo synchronizace: menu s `pocet_voleb == 1` → všechny volby ve **stejném
slotu** (paralelně, bez volna). Menu s více volbami (semináře) se plní volně –
kolize se řeší přes skutečné volby studentů (sdílený žák = konflikt).

---

## 7. Řešič

### 7.1 Vstupní transformace

1. **Lekce** – z plánu (podle Struktury) a z menu (podle voleb studentů);
   střídavé L/S = dvě lekce s `par` odkazem, sdílí jednu **jednotku**.
2. **Jednotky** – skupiny lekcí pohybujících se společně (L/S pár, Dív+Chl,
   synchronizované volby „vyber 1“).
3. **Konflikty** (bitset n×n) – dvě lekce kolidují, pokud sdílejí **učitele**
   nebo **aspoň jednoho žáka** (kromě párů L/S). → přirozeně pokrývá
   míchání tříd, skupiny, osobní kolize seminářů.
4. **Povolené sloty** – z omezení ročníků (maska 60 bitů), volna učitelů,
   délka lekce; **piny** omezí jednotku na jediný slot.

### 7.2 Tvrdá omezení (musí = 0 konfliktů)

- kolize učitel / žáci / místnost;
- **pravidlo 7 hodin** – per třída (union) i per student (přesně);
- max 8 hodin třídy/den; max 2 h stejného předmětu+skupiny/den;
- limity P1 (po auto-uvolnění); max h učitele/den;
- lekce bez vhodné místnosti.

### 7.3 Měkká kritéria (penalizace)

| kritérium | váha |
|---|---|
| okna (díry) ve dni třídy | 5 / slot |
| dny s 7:20 hodinou | 3 |
| odpolední hodiny | 1 (roč. ≤5) / 2 (roč. ≥6) |
| semináře 2h odpoledne | 2 / h |

### 7.4 Algoritmus

1. **Greedy start** – jednotky dle tísně (málo slotů, velké), cenová funkce
   = konflikty + obsazenost místností, preferuje dřívější sloty.
2. **Simulované žíhání** – výběr konfliktních jednotek (80 %), kandidátní slot
   (50 % best-of / 50 % náhodný), akceptace dle Δ(1000·tvrdé + měkké)/T,
   T₀=60, chladnutí ×0,9995, shake každých 4000 iterací při konfliktech.
3. **`slep_bloky`** – post-pass: stejné předměty téhož dne posouvá vedle sebe
   (souvislé dvojice), jen pokud nezhorší skóre.
4. **Místnosti** – greedy volba (preferovaná → odborná → kmenová → přeliv),
   s opravou konfliktů na konci.

### 7.5 Report po řešení

Varování (auto-uvolnění, prázdné skupiny, nekompletní volby, neaplikované
piny, kapacity) + problémy (kolize, 7 h, překročené limity, maxima dnů,
přesná kontrola per student) + statistiky tříd.

---

## 8. Uživatelská příručka

### 8.1 Horní lišta

`Rozvrh | Studenti | Volitelné | Učební plán | Číselníky | Nastavení` +
**„✚ Vytvořit nový rok…“** (zelené) + 💾 Uložit + 📁 Otevřít + stav řešení.

### 8.2 Průvodce novým rokem (4 kroky)

1. **Noví žáci** – jména po řádcích do textových polí nových tříd (prima, 1.A);
   „Načíst a rozdělit (15:15)“ vytvoří skupiny (dívky/chlapci střídavě s
   offsetem → vyrovnané poměry), ruční přesuny tlačítky →AJa/→AJb,
   pohlaví combem. Náhled: „15 žáků (8 D + 7 CH)“.
2. **Změny** – tabulka všech žáků: postupuje / **propadá** / **odchází**;
   maturanti a už odchozí se zobrazují zašedle.
3. **Volitelné** – výběr menu (katalog + tabulka voleb per žák; import
   katalogu i voleb z minulého roku; kapacity; ✂ rozdělení skupiny).
4. **Dokončení** – souhrn počtů a kompletnosti voleb → „✔ Vytvořit rok
   a vygenerovat rozvrh“ (spustí solver, přepne na Report; piny se vymažou).

### 8.3 Rozvrh (prohlížeč + editor)

- Režimy: **třída / učitel / místnost / student** (osobní rozvrh žáka).
- **Editace**: klikni na hodinu (modrý rámeček) → v každé buňce ⇥ = přesun;
  `Esc` zruší výběr, `Ctrl+Z` vrací (zásobník). Po každém přesunu kompletní
  validace; konfliktní hodiny ⚠ + červený výpis nad mřížkou.
- **📌 Připnout/Odepnout** – připnutá hodina se při přegenerování neposune
  (uloženo per klic, přežije restart).
- Prefixy v buňkách: `L·`/`S·` = střídavý týden, `📌` = pin.
- Tlačítka: `⬇ Tisk (HTML)`, `⬇ CSV`, `⬇ XML`, `⬇ Žáci (HTML)`, `↻ Přegenerovat`.

### 8.4 Studenti

Filtr třídy; editace jmen, AJ skupiny, pohlaví, **stavu (aktivní / propadá /
odešel)**; ruční přidání žáka; ✕ smazat.

### 8.5 Volitelné

Menu selector → katalog voleb (název, předmět, učitel, počet žáků + kapacita,
✂ rozdělit) → tabulka žák × N voleb (comby) → obsazenost a validace
(„X žáků nemá kompletní volby“).

### 8.6 Učební plán

Per třída: předmět, hodin, 2h blok, rozdělení (celá třída / AJa-AJb /
dívky-chlapci / **L↔S třída / L↔S skupiny** s editací párů předmět+učitel),
+ přidat/✕.

### 8.7 Nastavení

Časový limit solveru; přehled pravidla 7 h; limity tříd (přepis raných /
odpoledních), následníci tříd.

### 8.8 Report

**Zátěže učitelů** (vždy – i před generováním; semafor ≥26 oranžová, ≥31
červená), varování, problémy, statistiky tříd (hodiny, 7:20 dny, odpolední,
nejdelší blok).

---

## 9. Instalace a spuštění

```bash
# Rust verze (doporučeno)
cargo new rozvrh && cd rozvrh
# → vložit soubory z kap. 15
cargo run --release          # první build ~2 min

# Python reference verze (volitelné)
pip install ortools
python generator.py --cas 120 --html rozvrh.html
```

Požadavky: Rust 1.75+ (edition 2021). Data se ukládají do `skola_data.json`
(automaticky při zavření). GUI okno 1500×950.

---

## 10. Exporty

| Formát | Obsah | Poznámka |
|---|---|---|
| **HTML tisk** | všechny třídy, A4 landscape, 1 třída/strana, `page-break` | otevřít → Ctrl+P |
| **HTML žáci** | osobní rozvrh každého žáka (1 strana) | třídním k distribuci |
| **CSV** | `trida;den;slot;cas;predmet;ucitel;mistnost;skupina;pocet_zaku` | UTF-8 BOM, středníky → Excel |
| **XML** | `<rozvrh><trida><den><hodina …/>` | vlastní schéma |

**Bakaláři**: pro přímý import je potřeba vzorový exportní/importní soubor
z instalace školy → dodá se konvertor „⬇ Bakaláři“.

---

## 11. Stav

### Hotovo
✔ P1–P5 (s auto-uvolněním), L/S střídavé týdny, piny, kapacity+rozdělení
skupin, osobní rozvrhy, zátěže učitelů, tisk, slepování bloků, semináře
dopoledne, 15:15 dle pohlaví, editace+undo+validace, průvodce novým rokem,
import voleb/katalogů z minulého roku, perzistence, vláknové generování.

### Známá omezení v1
- klasifikace omezení „union třídy“ u 7 h je konzervativnější než per-student
  (solver) – per-student se kontroluje v reportu;
- semináře bez seznamu voleb by se mohly překrývat (model je optimistický,
  dokud nejsou reálná data voleb);
- L/S ušetří málo slotů → auto-uvolnění zůstává aktivní u primy–tercie;
- chybí drag&drop myší (klik-klik), jezdci vah, SQLite, diff roků.

---

## 12. Otevřené otázky

1. **Prima–tercie**: jak vyřešit 28–30 h vs 25 slotů? (povolit 12:55 /
   snížit dotace / jinak)
2. Limity kvarty/kvinty – **za den, nebo za týden**?
3. 4leté třídy – první A ≈ kvinta (schváleno?), čtvrtá A existuje?
4. **Semináře**: pošlete čísla voleb studentů → přesné plánování bez překryvů.
5. Potvrzení L/S interpretace (předpoklad: lichý/sudý týden).
6. **Velikosti tříd a skupin** (kapacity MJP 14, PAJ/PFJ 17!).
7. Jména učitelů (doplnit ze seznamu zaměstnanců).
8. Přesné hodinové dotace (učební plán) – seed je odhad z PDF.
9. Vzorový soubor Bakalářů pro importní konvertor.
10. Dny, kdy jednotliví učitelé neučí (částečné úvazky).

---

## 13. Roadmapa

**Nejbližší (navržené pořadí):**
1. ⭐ Import skutečného rozvrhu z CSV jako výchozího stavu.
2. Více variant rozvrhu (3–5 × generování, srovnávací tabulka skóre).
3. Jezdci vah penalizací v Nastavení.
4. Volné dny učitelů – klikací mřížka 5×12.
5. Diff mezi roky (dotace, učitelé, učebny).
6. Drag & drop myší.
7. SQLite pro víceletou historii.
8. Konvertor do Bakalářů (až bude vzorek).

**Nápady na později:** PDF export přímo, tisk celé školy jedním klikem z app,
validátor voleb proti kolizím před generováním, návrh rozdělení přeplněných
skupin s volbou učitele, „okolí“ hodiny (kontext učitele/místnosti v tooltipu),
pravidla sousednosti (TV hala ↔ šatny), automatické vyrovnání týdenní zátěže
učitelů.

---

## 14. Technické poznámky pro vývojáře

- **egui 0.27**: `ComboBox::from_id_source` (0.28+ používá `from_id_salt`),
  `ui.vertical`, `ui.collapsing`, `eframe::App::on_close_event`.
- **Borrow checker v egui**: pattern „fází“ – mutace self (akce) se shromažďují
  do flagů před výpůjčkou `&self.vysledek` a aplikují po jejím uvolnění;
  disjoint field borrows (`self.piny` × `self.vysledek`) fungují díky
  precise capture (edition 2021).
- **Solver ve vlákně**: `JoinHandle::is_finished()` + `Arc<Mutex<Option<Vysledek>>>`
  + `ctx.request_repaint_after(150 ms)`.
- **Konflikty jako bitset** `Vec<Vec<u64>>` – O(1) test, build O(n²·|žáci|).
- **Lekce vs. jednotky**: vše (sync, L/S, Dív/Chl) je řešeno jednotkami;
  L/S pár = 2 lekce s `par` (vyloučené vzájemné konflikty), každá má vlastního
  učitele a místnost.
- **Stabilní `klic`** lekce = `P|trida|predmet|skupina|ord` / `M|menu|volba|i`
  → piny přežijí přegenerování; při změně dat se neaplikované piny nahlásí.
- **rfd** dialogy jen v UI vlákně.
- CSV s BOM (`\u{FEFF}`) kvůli diakritice v Excelu.

---

## 15. Inventář zdrojových souborů

### Rust aplikace (v0.4)

| Soubor | Obsah |
|---|---|
| `Cargo.toml` | eframe 0.27, serde, serde_json, rand 0.8, rfd 0.13 |
| `src/main.rs` | bootstrap eframe |
| `src/data.rs` | konstanty, model, seed školy (třídy, místnosti, učitelé, předměty, plán, menu) |
| `src/solver.rs` | LekceInfo/Vysledek, Stav, postav_lekce, generuj (greedy+annealing+slep_bloky), piny, L/S |
| `src/validation.rs` | zkontroluj() – plná validace po ručních úpravách |
| `src/app.rs` | UI (obrazovky, průvodce, editace, exporty, piny, zátěže, rozdělení skupin) |

### Python reference verze (v0.1)

| Soubor | Obsah |
|---|---|
| `skola_data.py` | data školy (kompatibilní obsah s data.rs) |
| `generator.py` | CP-SAT model, diagnostika, auto-uvolnění, HTML výstup |

> Kompletní zdrojáky: viz zprávy konverzace – Rust v0.2 (základ), v0.3
> (validation + editace + exporty), v0.4 (patche L/S, piny, kapacity,
> zátěže, tisky, 15:15). Při ručním přepisu dodržet pořadí patchů.

---

*Dokument vygenerován z konverzace k projektu „Generátor rozvrhů GFP Neratovice“.
Data v seedu jsou odvozená z předběžného rozvrhu 2026/27 a čekají na ověření
(viz kap. 12).*
