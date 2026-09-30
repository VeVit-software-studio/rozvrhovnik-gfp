# Generátor rozvrhů – Gymnázium Františka Palackého, Neratovice

**Kompletní dokumentace projektu** – konsoliduje celou konverzaci (zadání, analýzu, architekturu,
všechny verze programu, otevřené otázky a plán do budoucna) a popisuje aplikaci v tomto repozitáři.

- Verze dokumentu: 1.2
- Stav aplikace: Rust desktop v0.5 – **implementováno v tomto repozitáři** (viz kap. 4 – Verze)
- Autor konzultací: AI asistent; zadavatel: vyučující GFP

**Rychlý start:** `cargo run --release` (GUI) nebo
`cargo run --release -- --generuj --export export` (vygeneruje rozvrh bez GUI) – viz kap. 9.

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
(konstanta `MAX_RUN = 7` v `data.rs`; tvrdé omezení, kontrolované přesně pro každého
studenta – v řešiči i ve validaci).

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
Velikosti tříd ⚠ (odhad 20–28; zástupní žáci v seedu: prima/sekunda/1A 28, tercie 28,
kvarta/kvinta 26, sexta 25, septima 24, oktáva 20, 2A 28, 3A 24, 4A 20).

**Vstupní třídy** (noví žáci) = 1. ročník bez předchůdce: *prima* a *první A*.
Kvarta B nemá předchůdce (prima–tercie mají jen 1 třídu) – aplikace ji považuje za
**dobíhající**: v dalším roce do ní nikdo nepostoupí a prázdná třída se z rozvrhu vynechá (⚠ ověřit).

### 2.2 Místnosti (39)

Kmenové (17, kap. 30, jakýkoliv předmět): `1G, 2G, 3G, kvartaA…4A` (podle tříd).

Odborné (22):

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

Pravidla jsou v modelu vyjádřena u předmětů: `specialni_mistnosti` (odborné učebny
v pořadí preference) a `kmenova_ok` (smí do kmenové / volné kmenové jiné třídy).

### 2.3 Učitelé (42 zkratek)

Zkratky z rozvrhu: SOJ, ŠAF, SLO, HAM, UHR, PAŘ, LIP, ŠMK, TES, ČER, TRO, VAI,
HOR, SAU, FID, PAL, ŠMD, LEB, ŠVE, CHOU, BER, VYS, LIH, KOŠ, VEJ, PALB, NEŠ,
SKŘ, PET, HRK, JIN, MAN, SMO, FAL, SCHO, PAV, BRD, SVI, WIE, ČEM, FOG, VAIS.

⚠ Jména známe jen u třídních (Sojková, Sloupová, Hamerníková, Uhrinčaťová,
Pařík, Lipovská, Smolková, Tesař, Červenková, Trojanová, Vaisová, Horáková,
Šafránková, Sauerová, Fidlerová); ostatní `(doplnit)` – seznam na gfp.cz/zamestnanci
(není dostupný z konzultace, nutno ručně).

⚠ **Aprobace a přiřazení učitelů k hodinám jsou v seedu odhad** (tabulka `UCITELE`
v `data.rs`): učitelé se přiřazují z aprobací tak, aby úvazky vyšly vyrovnaně
(aktuálně nejvýše 21 h; učitelé s jedinou aprobací – WIE, LIH – mají méně). Skutečné přiřazení se nastaví v Učebním plánu a katalozích voleb.

### 2.4 Časová mřížka

12 slotů/den, 5 dní: `7:20, 8:10, 9:00, 10:00, 10:55, 12:00, 12:55, 13:45,
14:35, 15:25, 16:15, 17:05` (hodina = 45 min). Odpolední = slot ≥ 6 (12:55+),
konfigurovatelné v Nastavení.

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

Plán je v `data.rs → sablona()` podle efektivního ročníku (189 položek pro 17 tříd).
Týdenní hodiny žáka (plán + volitelné):

| třída | h/týden | poznámka |
|---|---|---|
| prima | 28 | ČJL 4, MAT 4, AJ 4, DĚ 2, ZE 2, BIO 2, FY 1, OV 1, INF 1, TV 2, PL 1, HV 1, DV/VV 2 (L/S), praktika 1 |
| sekunda | 28 | 26 + volba INF/ČJL 2 |
| tercie | 30 | 27 + 3. jazyk 3 |
| kvarta | 33 | 30 (vč. LAT 2, INF 1) + 3. jazyk 3 |
| kvinta / 1A | 31 / 32 | 26 + VV/DV 2 + jazyk 3 (1A: 4) |
| sexta / 2A | 31 / 32 | 26 + VV/DV/HV 2 + jazyk 3 (2A: 4) |
| septima / 3A | 34 / 35 | 25 + jazyk 3 (3A: 4) + semináře 3 × 2 |
| oktáva / 4A | 32 / 33 | 17 + jazyk 3 (4A: 4) + semináře 6 × 2 |

**Přesné dotace nutno ověřit proti učebnímu plánu školy.**

---

## 3. Klíčová zjištění

### 3.1 ⛔ Fundamentální konflikt zadání (P1 vs. dotace)

Bez 7:20 a bez odpoledních mají prima–tercie k dispozici **25 slotů/týden**
(8:10–12:45 × 5 dní). Skutečná potřeba:
prima ≈ 28, sekunda ≈ 28, tercie ≈ 30, kvarta ≈ 33–34 h.

→ **Přísná pravidla P1 jsou s aktuálními dotacemi matematicky neřešitelná.**

Řešení v aplikaci: **auto-uvolnění** – solver spočítá potřebu per student,
minimálně zvýší odpolední limit a **vypíše to do Reportu** jako varování, např.:

```
AUTO-UVOLNĚNO prima: kapacita 25 < potřeba 28 h ➡ raných 0 / odpoledních 0 ➡ raných 0 / odpoledních 1 (/den).
```

Trvalé řešení je buď povolit 12:55, nebo snížit hodinové dotace. *L/S střídavé týdny
ušetří jen ~0–2 sloty/třídu, problém neřeší.*

### 3.2 Ostatní zjištění

- L/S = lichý/sudý týden (potvrzeno strukturou PDF).
- 4leté třídy mají vyšší dotaci 2. jazyka (4 h vs 3 h u 8letých) – proto mají vlastní
  menu `2j-1A … 2j-4A` (jazyky 8letých tříd stejného ročníku se s nimi nemíchají).
- Jména učitelů nejsou ve zkratkách – nutno doplnit ručně.
- Čtvrtá A v PDF chybí – přidána do modelu.
- Kvarta B nemá předchůdce → dobíhající třída (viz 2.1).
- Kapacity PVV/PHV (28) omezují skupiny VV/HV ve volitelných – volby mají kapacitu 28.
- Bakaláři: importní formát je proprietární a verzovaný – k přímému exportu
  potřebujeme vzorový soubor z instalace školy.

---

## 4. Verze

| Verze | Forma | Obsah | Kde |
|---|---|---|---|
| v0.1 | Python + OR-Tools (CP-SAT) | referenční model omezení, diagnostika slotů, auto-uvolnění, HTML export | jen v konverzaci (není v repozitáři) |
| v0.2 | Rust + egui desktop | datový model, solver (greedy + annealing), průvodce „Vytvořit nový rok“ (noví žáci 15:15, změny, volitelná), P5 pravidlo 7 h, učební plán/číselníky/nastavení | v0.4 v repozitáři |
| v0.3 | Rust + `validation.rs` | interaktivní úpravy rozvrhu (klik ➡, Ctrl+Z), okamžitá validace, režim „student“, export CSV/XML | v0.4 v repozitáři |
| v0.4 | Rust | **L/S střídavé týdny, 📌 piny, kapacity+✂ rozdělení skupin, osobní rozvrhy žáků, zátěže učitelů, tisk celé školy, slepování bloků + semináře dopoledne, 15:15 dle pohlaví** + režim příkazové řádky, váhy v Nastavení, volno učitelů, profily učitelů a předmětů, import/export žáků, testy | v0.5 v repozitáři |
| v0.5 | Rust | **požadavky školy z 9/2026**: import DBF a aliasy tříd (7.A, 3.A), pohlaví z rodného čísla, **import rozdělení do seminářů** (výstup školního nástroje), **⊞ Bloky seminářů** (které semináře mohou běžet proti sobě), učitelé po hodinách (NJ: 2 h UHR + 1 h KOŠ), **TV a PL ne v jeden den**, krátké dny a dlouhá okna, **🍴 jídelna** (rozložení obědů), demo data | **tento repozitář** |

Aplikace v repozitáři je nová, ucelená implementace funkcí v0.2–v0.5 (inventář viz kap. 15).

### 4.1 Požadavky školy (9/2026) a jak jsou vyřešené

| Požadavek | Řešení |
|---|---|
| Data z MS SQL / Bakalářů jako Excel, txt, csv, DBF | import žáků ze všech těchto formátů (kap. 10.1); DBF (dBase III) i s kódováním CP852 / Windows-1250 |
| Třídy se v exportech jmenují 7.B, 3.A … | **alias třídy** (Číselníky ➡ Třídy, sloupec „Alias (import)“); výchozí 5.A–8.B a 1.A–4.A |
| Pohlaví podle rodného čísla | sloupec „Rodné číslo“ se použije jen pro pohlaví (měsíc +50 = dívka), neukládá se |
| AJa/AJb se dělí ručně | ruční sloupec AJ v importu i v tabulce žáků (15:15 jen jako návrh) |
| Studenti volí semináře „jak je napadne“, ředitel skládá bloky | import souboru `Ročník;Seminář;Mezitřídní;Třídy;Počet žáků;Jméno žáka;Třída žáka` (kap. 8.5) ➡ záložka **⊞ Bloky**: nejmenší počet bloků, které semináře mohou být proti sobě, co brání menšímu počtu bloků (kap. 8.5a) |
| Jazyky mezitřídně i po třídách, NJ 1 h UHR + 1 h KOŠ, ŠJ část MAN, část NEŠ | menu voleb přes více tříd, ✂ rozdělení skupiny k jinému učiteli, **učitelé po hodinách** u volby |
| Malí nemají odpoledne | limity P1 (prima–tercie 0 odpoledních, auto-uvolnění hlásí výjimky) |
| **TV a PL ne v jeden den** | tvrdé pravidlo „předměty, které nesmí být ve stejný den“ (Nastavení, výchozí TV × PL) |
| Nikdo nemá chodit na jednu hodinu ani čekat 5 hodin | penalizace dne s 1–2 hodinami a oken delších než 2 h; varování v Reportu |
| Jídelna – všichni v 11:40 | vlny obědů 11:40 / 12:45 / 13:40 / 14:30, kapacita vlny v Nastavení, řešič rozkládá příchody, tabulka v Reportu (kap. 7.3a) |

---

## 5. Architektura

```
┌───────────────────────────── aplikace (eframe/egui) ─────────────────────┐
│  hlavní panel: Rozvrh | Studenti | Volitelné | ⊞ Bloky | Plán | Učitelé  │
│     | Předměty | Číselníky | Nastavení | Report + „✚ Vytvořit nový rok…“ │
│  průvodce (Window): 0 rok → 1 noví žáci → 2 změny → 3 volitelné → 4 fin. │
├─────────────────────────────── data (JSON) ──────────────────────────────┤
│  skola_data.json: { skola, rok, historie[], piny{}, rozvrh }  (autosave) │
├─────────────────────────────── solver (vlákno) ─────────────────────────┤
│  lekce → jednotky (sync) → profily žáků → konflikty (bitset) → greedy    │
│  → žíhání → slep_bloky + dočištění → místnosti → validace → report       │
└───────────────────────────────────────────────────────────────────────────┘
```

- **Knihovna + binárka**: logika (`data`, `rok`, `solver`, `validation`, `export`) je
  v knihovně `rozvrhovnik` bez závislosti na GUI – testovatelná a použitelná z příkazové
  řádky; GUI je jen v `src/app.rs`.
- **GUI**: eframe/egui 0.27 (desktop, jediný binary). Řešič běží ve vlákně,
  UI dotazuje průběh přes `Arc<Mutex<Prubeh>>` + `request_repaint_after(150 ms)`;
  tlačítko ⏹ Zastavit ukončí žíhání a použije dosud nejlepší rozvrh.
- **Řešič**: vlastní (bez externího solveru) – viz kap. 7.
- **Perzistence**: `serde_json`, soubor `skola_data.json` v pracovním adresáři (nebo cesta
  z příkazové řádky); ukládání při zavření + tlačítkem 💾; zápis přes dočasný soubor.
  Uložený je i poslední vygenerovaný rozvrh.
- **Historie let**: minulé `SkolniRok`y → zdroj pro import voleb a katalogů.

---

## 6. Datový model

### 6.1 `Skola` (statická, číselníky + plán)

- `Trida { id, nazev, rocnik, typ, kmenova, naslednik, omezeni_prepsat }`
- `Mistnost { id, nazev, kapacita, kmenova }`
- `Ucitel { id, jmeno, volno[], max_den, preferovana_mistnost, preferovane_tridy[] }` – `volno` = sloty
  (den*12+slot); `preferovane_tridy` = školní třídy, kterým řešič přednostně dává lepší hodiny tohoto učitele
- `Predmet { id, nazev, specialni_mistnosti[], kmenova_ok }`
- `PlanovaHodina { trida, predmet, hodin, struktura, dvoj }`
  - `Struktura`:
    - `CelouTridou(učitel)`
    - `Rozdeleno { a, b }` – AJa/AJb (hodiny skupin jsou nezávislé; řešič je páruje přes okna)
    - `Pohlavi { div, chl }` – TV/PL (vynuceně paralelní slot)
    - `Stridave { l: Pol, s: Pol }` – L/S celou třídou
    - `StridaveSkupiny { a: [Pol;2], b: [Pol;2] }` – L/S per AJa/AJb
    - `Pol { predmet, ucitel }`
- `Nastaveni { casovy_limit_s, limity_za_den, odpoledne_od, auto_uvolneni, seed, vahy, neslucitelne[(A, B)],
  obedova_pauza, jidelna_kapacita, jidelna_podil }` – chybějící položky ve starých souborech dostanou výchozí hodnoty
- `Trida.alias` – jiné názvy třídy pro import (např. „7.A, septima A“)
- `Volba { id, nazev, predmet, ucitel, kapacita, blok, ucitele_hodin[] }` – `blok` = blok seminářů (volby
  jednoho bloku běží paralelně), `ucitele_hodin` = učitel pro 1., 2., … hodinu týdně (prázdné = `ucitel`)

### 6.2 `SkolniRok` (roční data)

- `label`, `studenti[]`, `menu[]` (katalogy voleb per rok), `dalsi_id`
- `Student { id, jmeno, trida, skupina_aj, pohlavi, volby{menu→[volby]}, status }`
  (`status`: aktivní / propadá / odešel)
- `Menu { id, nazev, tridy[], volby[], hodin_tydne, dvojhodina, pocet_voleb }`
- `Volba { id, nazev, predmet, ucitel, kapacita }`

### 6.3 `Vysledek` (vygenerovaný rozvrh)

- `lekce[]` (LekceInfo: predmet, ucitel, tridy, skupina, zaci, len, kandidati, kmenova_ok,
  **klic** – stabilní ID pro piny, **klic_predmetu** – pro „max 2 h/den“ a slepování,
  **par** – partner L/S, **tyden** – oba/L/S, **dopoledne** – flag, **jednotka**, menu)
- `slot[]` (začátek den*12+slot), `mistnost[]`, `varovani[]`, `problemy[]`,
  `problemove_lekce[]`, `skore (tvrde, mekke)`, `iteraci`, `cas_s`,
  `limity` (efektivní limity P1 po auto-uvolnění), `zaci_tridy`, `virtualni_zaci`

### 6.4 Menu voleb (seed, mapuje zadání a–f)

| menu | třídy | volby | h/týden | 2h | voleb/žák |
|---|---|---|---|---|---|
| 3j-3…3j-8 | ročník 3–8 (8leté) | NJ2/ŠJ2/FJ2 | 3 | ne | 1 |
| 2j-1A…2j-4A | 1A … 4A (4leté) | NJ2/ŠJ2/FJ2 | 4 | ne | 1 |
| vol-5 | kvintaA/B, 1A | VV2 (kap. 28) / DV2 | 2 | ano | 1 |
| vol-6 | sextaA/B, 2A | VV2 / DV2 / HV2 (kap. 28) | 2 | ano | 1 |
| sem-7 | septimaA/B, 3A | 14 seminářů | 2 | ano | **3** |
| sem-8 | oktavaA/B, 4A | 15 seminářů | 2 | ano | **6** |

Pravidlo synchronizace: menu s `pocet_voleb == 1` → všechny volby ve **stejném
slotu** (paralelně, bez volna). Menu s více volbami (semináře) se plní volně –
kolize se řeší přes skutečné volby studentů (sdílený žák = konflikt).

Zástupní volby v seedu (a tlačítko „📝 Doplnit chybějící“) rozdělí volby do
`pocet_voleb` bloků a žák dostane z každého bloku nejméně obsazenou volbu – tak se
semináře dají rozvrhnout bez překryvů (realistická struktura „seminárních bloků“).

---

## 7. Řešič

### 7.1 Vstupní transformace

1. **Lekce** – z plánu (podle Struktury) a z menu (podle voleb studentů);
   střídavé L/S = dvě lekce s `par` odkazem, sdílí jednu **jednotku**.
   Dvouhodinovky (`dvoj`) = lekce délky 2 (lichý zbytek = lekce délky 1).
2. **Jednotky** – skupiny lekcí pohybujících se společně (L/S pár, Dív+Chl,
   synchronizované volby „vyber 1“).
3. **Profily žáků** – žáci se stejnou sadou jednotek mají stejný rozvrh; všechna
   omezení a okna se počítají přesně per profil (tedy per žák), vážená počtem žáků.
4. **Konflikty** (bitset jednotek) – dvě lekce kolidují, pokud sdílejí **učitele**
   nebo **aspoň jednoho žáka** a jejich týdny se překrývají (L × S nekoliduje).
   → přirozeně pokrývá míchání tříd, skupiny, osobní kolize seminářů.
5. **Povolené sloty** – z omezení ročníků (maska 60 bitů), volna učitelů,
   délky lekce; **piny** omezí jednotku na jediný slot.
6. **Prázdné třídy** – vstupní třída bez žáků se plánuje se 4 zástupními (virtuálními)
   žáky (AJa/AJb × D/CH); jiná prázdná třída (dobíhající) se vynechá.

### 7.2 Tvrdá omezení (musí = 0 konfliktů)

- kolize učitel / žáci / místnost (místnosti: kapacita „bazénů“ odborných učeben, které
  nesmí do kmenové – TV, PL, INF, VV, HV, praktika – se hlídá už při řešení, per týden L/S);
- **pravidlo 7 hodin** – přesně per žák;
- max 8 hodin žáka/den; max 2 h stejného předmětu+skupiny/den;
- limity P1 (po auto-uvolnění; za den nebo za týden); max h učitele/den;
- **neslučitelné předměty ve stejný den** – pro každého žáka (výchozí TV × PL, další dvojice
  v Nastavení); řešič počítá per profil žáků a den;
- lekce bez vhodné místnosti (kontroluje validace po přiřazení místností).

### 7.3 Měkká kritéria (penalizace, nastavitelné v Nastavení)

| kritérium | výchozí váha |
|---|---|
| okna (díry) ve dni žáka | 5 / slot |
| dny s 7:20 hodinou | 3 |
| odpolední hodiny | 1 (roč. ≤5) / 2 (roč. ≥6) |
| semináře 2h odpoledne | 2 / h |
| stejný předmět 2× za den (nesouvisle = víc) | 2 |
| okno učitele | 1 |
| hodina **preferované třídy učitele** v 7:20 / odpoledne | 3 |
| **den žáka s 1–2 hodinami** (×2 pro jedinou hodinu) | 6 |
| **dlouhé okno** – každá hodina okna nad 2 h (navíc k oknu) | 4 |
| **jídelna** – strávník nad kapacitu vlny obědů | 2 |

Jedna volná hodina v poledne před odpoledním vyučováním je **pauza na oběd** a nepočítá se
jako okno (lze vypnout v Nastavení).

Penalizace žáků se násobí počtem žáků profilu, penalizace učitele (okna, preferované třídy)
měřítkem 5; tvrdé porušení má váhu 50 000.

**Preferované třídy učitele** (návrh `docs/superpowers/specs/2026-09-25-profily-ucitelu-a-predmetu-design.md`):
řešič nerozhoduje, kdo učí kterou třídu (to je dané plánem), jen *kdy*. U jednotek, jejichž
třída je v `preferovane_tridy` jejich učitele, se hlídá zvláštní obsazenost učitele
(`teach_pref_occ`) a v `ucitel_den` se za každou takovou hodinu v 7:20 nebo odpoledne připočte
váha `ucitel_preference`. Brzké/pozdní hodiny učitele tak přednostně dostanou jeho
nepreferované třídy. Učitelé bez preferencí nemají žádnou režii ani efekt.
Ověřeno testem `tests/preference_ucitele.rs` (2 třídy, 1 učitel, 30 h): brzké/odpolední hodiny
preferované třídy 0 (nepreferované 6), bez preference 2 / 4.

### 7.3a Jídelna

Žák jde na oběd po první hodině od 11:40 dál, po které má volno (konec vyučování nebo volná
hodina): vlny **11:40** (po 5. hodině), **12:45**, **13:40**, **14:30**. Kdo má vyučování v kuse
až do 14:30, stihne oběd jen o přestávce v 11:40 (počítá se do vlny 11:40, v Reportu „Bez pauzy“).
Odhad strávníků vlny = žáci × „obědvá % žáků“ (výchozí 70 %). Řešič penalizuje každého strávníka
nad **kapacitu vlny** (výchozí 120, 0 = nehlídat) – posouvá tak konce vyučování a polední pauzy
tříd, aby nepřišli všichni najednou. Zatížení se udržuje inkrementálně (vlna profilu × den).
Na seed datech: bez hlídání nejvytíženější vlna 168 strávníků, s kapacitou 120 nejvýše 118,
stále 0 tvrdých problémů. Skutečnou kapacitu a podíl strávníků je třeba nastavit podle jídelny.

### 7.4 Algoritmus

1. **Greedy start** – jednotky dle tísně (málo povolených slotů, velké), každá na slot
   s nejmenším přírůstkem ceny (konflikty + tvrdá + měkká kritéria), dny v náhodném
   pořadí, v rámci dne preferuje dřívější sloty.
2. **Simulované žíhání** – výběr jednotek v konfliktu (80 %), tah = přesun
   (50 % nejlepší ze 6 náhodných slotů / 50 % náhodný) nebo **výměna** s jednotkou,
   která sdílí žáky (30 %); akceptace dle exp(−Δ/T), inkrementální výpočet ceny
   (jen dotčené profily/učitelé/dny). Teplota klesá exponenciálně s časem
   (T 2000 → 10 během časového limitu), shake každých 4000 iterací při zaseknutí
   s konflikty. Bez tvrdých porušení a bez zlepšení po ¼ limitu (min. 3 s) končí dřív.
   Na seed datech: ~300 000 iterací/s, 0 tvrdých problémů obvykle do 2 s.
3. **`slep_bloky`** – post-pass: stejné předměty téhož dne posouvá vedle sebe
   (souvislé dvojice), jen pokud se skóre zlepší; poté **dočištění** – každou jednotku
   zkusí přesunout na nejlepší povolený slot.
4. **Místnosti** – greedy volba (preferovaná → odborná → kmenová → přeliv do volné
   kmenové), respektuje kapacitu, s opravou konfliktů výměnou (uvolní místnost jiné
   lekci, která má volnou alternativu). Po ručním přesunu se místnosti přepočítají jen
   pro přesunuté lekce.
5. **Validace** (`validation.rs`) – nezávislá kontrola hotového rozvrhu, per žák a per
   týden L/S; tatáž se volá po každé ruční úpravě.

`seed` v Nastavení (🎲 jiná varianta) dává jiné varianty rozvrhu.

### 7.5 Report po řešení

Varování (auto-uvolnění, prázdné skupiny, nekompletní volby, neaplikované
piny, kapacity) + problémy (kolize, 7 h, překročené limity, maxima dnů,
přesná kontrola per student) + statistiky tříd (hodiny, dny s 7:20, odpolední hodiny,
nejdelší blok, průměrná okna) + krátké dny a dlouhá okna (třída, den, počet žáků)
+ 🍴 jídelna (strávníci po vlnách a dnech, přes kapacitu červeně; kdy chodí třídy na oběd).

---

## 8. Uživatelská příručka

### 8.1 Horní lišta

`📅 Rozvrh | 👥 Studenti | ☑ Volitelné | 📚 Učební plán | 🎓 Učitelé | 📖 Předměty | 📇 Číselníky | ⚙ Nastavení | 📊 Report`
+ **„✚ Vytvořit nový rok…“** (zelené) + 💾 Uložit + 📁 Otevřít; druhý řádek: rok,
**▶ Generovat** / průběh řešení (fáze, čas, iterace, skóre) + ⏹ Zastavit, stavová zpráva.

### 8.2 Průvodce novým rokem (4 kroky)

0. **Rok** – označení nového roku (automaticky další, např. 2027/28).
1. **Noví žáci** – jména po řádcích do textových polí vstupních tříd (prima, první A) nebo
   **📂 Načíst ze souboru…** (txt, csv, Excel, json – jména, případně pohlaví a skupina AJ; řádky
   jiných tříd se přeskočí);
   pohlaví se odhadne ze jména (-ová/-á = dívka), lze zapsat „Jméno Příjmení; D“ / „; CH“.
   „Načíst a rozdělit (15:15)“ vytvoří skupiny (dívky/chlapci střídavě s
   offsetem → vyrovnané poměry), ruční přesuny tlačítky →AJa/→AJb,
   pohlaví combem, „⚖ Přerozdělit 15:15“. Náhled: „15 žáků (8 D + 7 CH) · AJa 8 / AJb 7“.
2. **Změny** – tabulka všech žáků (filtr třídy): postupuje / **propadá** / **odchází**;
   sloupec „Nový rok“ ukazuje cílovou třídu, maturanti a už odchozí jsou zašedlí.
3. **Volitelné** – editor voleb nad novým rokem (katalogy zkopírované z minulého roku,
   pokračující volby – jazyk, VV/DV – přeneseny automaticky; import katalogu i voleb
   z minulého roku; kapacity; ✂ rozdělení skupiny; 📝 doplnit chybějící).
4. **Dokončení** – souhrn tříd (žáků, D/CH, AJa/AJb) a kompletnosti voleb →
   „✔ Vytvořit rok a vygenerovat rozvrh“ (starý rok do historie, piny se vymažou,
   spustí solver, přepne na Report).

### 8.3 Rozvrh (prohlížeč + editor)

- Režimy: **třída / učitel / místnost / student** (osobní rozvrh žáka).
- Tmavší buňky = mimo limity P1 dané třídy (po auto-uvolnění).
- **Editace**: klikni na hodinu (modrý rámeček, detail nad mřížkou) → v každé buňce
  „➡ sem“ = přesun celé jednotky; `Esc` zruší výběr, `Ctrl+Z` / „↩ Zpět“ vrací
  (zásobník). Po každém přesunu se přepočítají místnosti a proběhne kompletní
  validace; konfliktní hodiny ⚠ + červený výpis nad mřížkou.
- **📌 Připnout/Odepnout** – připnutá hodina se při přegenerování neposune
  (uloženo per klíč, přežije restart; ruční přesun připnuté hodiny pin aktualizuje).
- Prefixy v buňkách: `L·`/`S·` = střídavý týden, `📌` = pin, `⚠` = problém.
- Tlačítka: `⬇ Tisk tříd (HTML)`, `⬇ Učitelé (HTML)`, `⬇ Žáci (HTML)`, `⬇ CSV`, `⬇ XML`,
  `↻ Přegenerovat`, `↩ Zpět`.

### 8.4 Studenti

Filtr třídy; editace jmen, třídy, AJ skupiny, pohlaví, **stavu (aktivní / propadá /
odešel)**; počet menu s volbami (tooltip = volby); ruční přidání žáka (skupina AJ se
doplní do menší, pohlaví odhadem); ✖ smazat; „⚖ Rozdělit AJ 15:15“ pro vybranou třídu.

**Import a export seznamu žáků** (lišta „Seznam žáků“ nad tabulkou, podrobně kap. 10.1):
- **⬆ Import…** – txt, csv, Excel (xlsx / xls / ods) a json; soubor lze také **přetáhnout do okna**.
  Před importem se ukáže náhled (rozpoznané sloupce, počty podle tříd, problémové řádky, prvních
  12 žáků) a volba režimu: přidat/aktualizovat, nahradit třídy uvedené v souboru, nahradit vše.
- **⬇ Export** – CSV, JSON, Excel (.xlsx), TXT (volitelně jen jména); exportuje se výběr podle
  filtru třídy (nebo všichni žáci).

### 8.5 Volitelné

Menu selector (+ nové / ✖ smazat) → vlastnosti menu (název, h/týden, 2h bloky, voleb na
žáka, třídy) → katalog voleb (název, předmět, učitel, počet žáků + kapacita,
✂ rozdělit – druhá polovina žáků k vybranému učiteli) → tabulka žák × N voleb (comby)
→ obsazenost a validace („X žáků nemá kompletní volby“).
Tlačítka: ⬇ Katalog / ⬇ Volby z minulého roku (je-li historie), 📝 Doplnit chybějící.

**Učitelé po hodinách**: u volby s více hodinami týdně „po hodinách“ ➡ učitel pro 1., 2., 3. hodinu
(např. NJ: 1. a 2. hodina UHR, 3. hodina KOŠ). U menu s dvouhodinovými bloky platí pro celý blok.
Úvazky, profily učitelů i řešič počítají s rozdělením.

**⬆ Import skupin…** (lišta nad menu; soubor lze i přetáhnout do okna na této obrazovce) – načte
rozdělení žáků do seminářů/skupin ve formátu školního nástroje:

```
Ročník;Seminář;Mezitřídní;Třídy;Počet žáků;Jméno žáka;Třída žáka
Septima + třeťák;SBI1;;7.B;16;Novák Jan;7.B
;;;;;Dvořáková Eva;7.B
```

(csv, txt, Excel, DBF; řádek s kódem semináře začíná skupinu, další řádky jen se jménem do ní patří;
jména „Příjmení Jméno“ i „Jméno Příjmení“; třída podle aliasu; volitelný sloupec **Blok**; stejný kód na
více místech se sloučí). Náhled ukáže skupiny, počty (a nesouhlas s „Počet žáků“), menu se navrhne
podle tříd. Soubor je **rozhodující**: nové semináře se přidají do katalogu (předmět podle kódu – SBI1 ➡
SBI, jinak se založí; učitel ze sesterské volby nebo ze souboru), volby žáků tříd menu se nahradí.
Možnosti: přidat žáky, kteří v seznamu chybí; odebrat z menu volby, které v souboru nejsou.

### 8.5a Bloky seminářů (⊞ Bloky)

Odpověď na otázku „jak udělat bloky seminářů proti sobě / který blok může obsahovat jaké předměty“:
- **graf kolizí** – dva semináře nemohou být ve stejném bloku, když mají společného žáka, stejného
  učitele (i u jednotlivých hodin) nebo oba jen tutéž odbornou učebnu; matice kolizí s důvody;
- **🔀 Navrhnout bloky** – nejmenší počet bloků (dolní mez = největší skupina navzájem kolidujících
  seminářů, DSatur + přesné obarvení s časovým limitem), vyrovnané velikosti; ručně přiřazené bloky
  se zachovají; **Doplnit nepřiřazené**, **✖ Zrušit bloky**;
- ruční přesun semináře do bloku (u každého bloku ✔ / počet kolizí), „Kam může“ s důvody;
- upozornění, když se bloky nevejdou do týdne (hodiny třídy + bloky > 40 h);
- **Kdo brání menšímu počtu bloků** – kolize, které způsobuje jen 1–2 žáků, a tlačítko „Najít žáky…“:
  pořadí žáků, po jejichž změně jedné volby klesne počet bloků (např. 6 ➡ 4 po změně 22 žáků);
- **⬇ Export (CSV)** – `Blok;Seminář;Název;Učitel;Třídy;Počet žáků;Jméno žáka;Třída žáka`, jde znovu
  načíst importem skupin.

Řešič pak plánuje semináře jednoho bloku **paralelně** (jedna jednotka), bloky jako celky.

### 8.6 Učební plán

Per třída: předmět, hodin, 2h blok, rozdělení (celá třída / AJa-AJb /
dívky-chlapci / **L↔S třída / L↔S skupiny** s editací párů předmět+učitel),
+ přidat/✖; součet hodin plánu + volitelných.

### 8.7 Učitelé (profily)

Vlevo seznam učitelů s úvazkem (hledání podle zkratky/jména, semafor ≥26 / ≥31 h), vpravo
profil vybraného učitele:
- **úvazek** (h/týden, stejný výpočet jako v Reportu), max h/den, volno, preferovaná učebna,
  hodiny ve vygenerovaném rozvrhu;
- **📅 Zobrazit rozvrh** – přepne na Rozvrh v pohledu tohoto učitele;
- **preferované třídy** (editovatelné, „Upravit…“) – ovlivňují generování (viz 7.3);
- **vyučované předměty** a **vyučované třídy** (s hodinami; preferované modře);
- tabulka **Co učí**: předmět × třída × skupina × h/týden (+ menu voleb a počet žáků).

Zkratka, jméno, max h/den, volno a preferovaná učebna se dál upravují v Číselníkách.
Vše kromě preferovaných tříd je odvozené z Učebního plánu a katalogů voleb.

### 8.8 Předměty (profily)

Vlevo seznam předmětů s hodinami týdně (🔒 = povinné učebny, šedě = neučí se), vpravo profil:
- název (editovatelný), celkem hodin týdně;
- **učebny**: „Smí do kmenové učebny“ + odborné učebny (s kapacitami) a srozumitelné vysvětlení:
  **🔒 povinné** (odškrtnuto – např. INF jen v PIF, jinde se učit nesmí) nebo
  **⭐ preferované** (zaškrtnuto – přednostně odborná učebna, jinak kmenová / jiná volná kmenová);
  z rozvrhu kolik hodin skutečně je v odborné učebně;
- **vyučující**: třída × skupina × učitel × h/týden (+ volby);
- **třídy a skupiny**, kde se předmět učí.

U střídavých hodin (L/S) se počítá skutečný předmět páru, ne popisek (např. „DV/VV“).

### 8.9 Číselníky

Třídy (název, ročník, typ, kmenová, následník, výchozí limity), Místnosti (kapacita,
kmenová), Učitelé (jméno, max h/den, preferovaná učebna, **📅 volno – klikací mřížka
5×12**), Předměty (smí do kmenové, odborné místnosti). Přidání přes „nové ID“.

### 8.10 Nastavení

Časový limit solveru (logaritmický posuvník), semínko varianty (🎲), limity za den /
za týden, začátek odpoledne, auto-uvolnění, přehled pravidla 7 h, **váhy měkkých
kritérií** (posuvníky, vč. „preferovaná třída učitele“, krátký den, dlouhé okno, jídelna), polední pauza,
**předměty, které nesmí být ve stejný den** (dvojice, výchozí TV × PL), **🍴 jídelna** (kapacita vlny,
% strávníků), limity tříd (přepis raných / odpoledních; odškrtnuto = ∞), následníci tříd.

### 8.11 Report

**Zátěže učitelů** (vždy – i před generováním; semafor ≥26 oranžová, ≥31
červená), varování, problémy, statistiky tříd (hodiny, 7:20 dny, odpolední,
nejdelší blok, okna), **🍴 jídelna** (kap. 7.3a).

---

## 9. Instalace a spuštění

```bash
# Požadavky: Rust stable (testováno 1.94), edition 2021
git clone <repozitář> && cd rozvrhovnik-gfp
cargo run --release                  # GUI; první build ~2 min
cargo run --release -- muj_soubor.json   # GUI s jiným datovým souborem

# Generování bez GUI (report na stdout, návratový kód 0 = bez problémů, 2 = zbyly problémy)
cargo run --release -- --generuj --cas 60 --export export
#   --data <soubor.json>  --cas <s>  --seed <n>  --export <adresář>  --uloz
cargo run --release -- --help

cargo test --release                 # testy logiky (řešič na seed datech ~30 s)

# Demo data se smyšlenými jmény (seznam žáků, semináře ve formátu školy, projekt, Excel)
cargo run --release -- --demo-data demo [--seed 7]
```

- Data se ukládají do `skola_data.json` (automaticky při zavření, 💾 kdykoliv).
  Pokud soubor neexistuje, načtou se **výchozí data** (seed z rozvrhu 2026/27 se
  zástupními žáky „Žákyně 01 / Žák 02 …“), takže rozvrh jde hned vygenerovat.
- GUI okno 1500×950 (min. 900×600).
- **Windows / macOS**: otevření a uložení souborů přes nativní dialogy (rfd).
- **Linux**: potřebuje běžné desktopové knihovny (`libxkbcommon-x11-0`, OpenGL/Mesa);
  nativní dialogy se nepoužívají (rfd by vyžadovalo GTK) – exporty se ukládají do
  složky `./export`, „📁 Otevřít“ nabídne zadání cesty.
- Na Windows se release build spouští bez konzolového okna.

Python referenční verze (v0.1, OR-Tools) není součástí repozitáře.

---

## 10. Exporty

| Formát | Obsah | Poznámka |
|---|---|---|
| **HTML tisk tříd** | všechny třídy, A4 landscape, 1 třída/strana, `page-break` | otevřít → Ctrl+P |
| **HTML učitelé** | rozvrh každého učitele (1 strana) | |
| **HTML žáci** | osobní rozvrh každého žáka (1 strana) | třídním k distribuci |
| **CSV** | `trida;den;slot;cas;predmet;ucitel;mistnost;skupina;pocet_zaku;tyden` | UTF-8 BOM, středníky → Excel; řádek = třída × hodina |
| **XML** | `<rozvrh rok><trida id nazev><den index nazev><hodina slot cas predmet ucitel mistnost skupina tyden zaku/>` | vlastní schéma |

Z příkazové řádky `--export <adresář>` zapíše všech pět souborů najednou.

**Bakaláři**: pro přímý import je potřeba vzorový exportní/importní soubor
z instalace školy → dodá se konvertor „⬇ Bakaláři“. (Seznam žáků ve formátu Excel/CSV z Bakalářů
lze ale načíst už dnes – viz 10.1.)

### 10.1 Seznam žáků – import a export

**Import** (`src/studenti_io.rs`): `.txt`, `.csv` / `.tsv`, `.xlsx` / `.xls` / `.ods` (první list s daty)
a `.json`.

| Co | Jak se pozná |
|---|---|
| **Sloupce s hlavičkou** | Jméno, Příjmení (dohromady „Jméno Příjmení“), Třída, AJ / Skupina, Pohlaví, Stav, Volby; ostatní sloupce (např. „Č.“, „Poznámka“) se ignorují |
| **Bez hlavičky** | podle obsahu buňky: třída (`prima`, `kvarta A`, `1.A`), `AJa`/`AJb`, `D`/`CH`/`Ž`/`M`, stav, `menu=volba`; zbytek = jméno. Samotný seznam jmen (jedno na řádek) funguje také |
| **Třída** | id nebo název, bez ohledu na velikost písmen, mezery, tečky a diakritiku (`kvartaA`, `Kvarta A`, `1.A`, `první A`) |
| **Oddělovač** | `;` `,` nebo tabulátor – odhadne se |
| **Kódování** | UTF-8 (s BOM i bez), UTF-16 s BOM a **Windows-1250** (český Excel „CSV“) – při jiném než UTF-8 upozornění |
| **DBF** | dBase III (FoxPro/Bakaláři), smazané záznamy se vynechají; kódování podle hlavičky (CP852, Windows-1250), jinak se odhadne a ohlásí |
| **Alias třídy** | `7.A`, `3.A` … podle sloupce „Alias (import)“ v Číselníky ➡ Třídy; neznámá nebo nejednoznačná třída se ohlásí s radou |
| **Rodné číslo** | sloupec „Rodné číslo“ / „RČ“ ➡ pohlaví (měsíc 51–62 / 71–82 = dívka); samotné RČ se neukládá. Sloupec Pohlaví má přednost |
| **Jméno** | „Jméno Příjmení“ i „Příjmení Jméno“ – při aktualizaci se žák najde bez ohledu na pořadí |
| **Pohlaví** | `D`, `Ž`, `dívka`, `F` ➡ D; `CH`, `M`, `chlapec` ➡ CH; chybí-li, odhad ze jména |
| **Stav** | aktivní / propadá / odešel |
| **Volby** | `menu=V1,V2\|menu2=V3` (menu oddělená svislítkem, id nebo názvy); ověřují se proti katalogům (menu se musí týkat třídy žáka, počet voleb) |
| **JSON** | pole jmen, pole objektů, nebo `{"studenti": [...]}` (formát exportu) |

Řádky, které nejde zařadit (neznámá třída, chybějící třída bez zvolené výchozí), se přeskočí a
vypíšou se. Řádky bez třídy lze zařadit do „výchozí třídy“ zvolené v náhledu (předvyplněná podle filtru).

**Režimy importu:**
1. *Přidat nové a aktualizovat stávající* (výchozí) – žák se stejným jménem ve stejné třídě se
   aktualizuje (skupina, pohlaví, stav, volby podle souboru), ostatní se přidají; duplicita
   v souboru se přeskočí.
2. *Nahradit žáky tříd uvedených v souboru* – stávající žáci těchto tříd (i s volbami) se smažou.
3. *Nahradit všechny žáky.*

U režimů 2 a 3 náhled červeně uvede, kolik stávajících žáků se smaže. Noví žáci bez skupiny AJ:
celá nová třída se rozdělí 15:15 podle pohlaví, jinak se doplňují do menší skupiny. Po importu je
třeba rozvrh přegenerovat.

**Export** (obrazovka Studenti; exportuje se výběr podle filtru třídy, jinak všichni; pořadí tříd
podle číselníku):

| Formát | Obsah |
|---|---|
| **CSV** | `Jméno;Třída;AJ;Pohlaví;Stav;Volby` – UTF-8 s BOM, středníky, CRLF ➡ rovnou do Excelu |
| **Excel (.xlsx)** | list „Studenti“, tučná ukotvená hlavička, automatický filtr, šířky sloupců |
| **JSON** | `{"rok": "...", "studenti": [{jmeno, trida, trida_nazev, skupina_aj, pohlavi, stav, volby{menu:[volby]}}]}` |
| **TXT** | `Jméno; Třída; AJ; Pohlaví[; Stav][; Volby]`, jeden žák na řádek; volba „TXT jen jména“ ➡ jen jména |

Všechny formáty lze znovu naimportovat (ověřeno testem `export_a_zpetny_import_ve_vsech_formatech`).
Na Windows/macOS se soubor vybírá / ukládá nativním dialogem; na Linuxu se exporty ukládají do
`./export/` a v okně importu se nabízejí soubory z aktuální složky a z `./import/` (nebo zadejte cestu,
případně soubor přetáhněte do okna).

⚠ Seznamy žáků jsou osobní údaje – složky `import/` a `export/` jsou v `.gitignore`, aby se nedostaly
do repozitáře.

---

## 11. Stav

### Hotovo
✔ **v0.5**: import DBF, aliasy tříd, pohlaví z RČ, import rozdělení do seminářů, ⊞ bloky seminářů
(návrh, kontrola, kdo brání menšímu počtu bloků, export), paralelní plánování bloků, učitelé po hodinách,
TV × PL ne v jeden den, krátké dny a dlouhá okna, polední pauza, 🍴 jídelna, demo data.

✔ P1–P5 (s auto-uvolněním), L/S střídavé týdny, piny, kapacity+rozdělení
skupin, osobní rozvrhy, zátěže učitelů, tisk (třídy, učitelé, žáci), slepování bloků,
semináře dopoledne, 15:15 dle pohlaví, editace+undo+validace, průvodce novým rokem,
import voleb/katalogů z minulého roku, perzistence, vláknové generování se zastavením,
váhy v Nastavení, volno učitelů (mřížka 5×12), varianty přes semínko, režim příkazové
řádky, **profily učitelů a předmětů** (záložky Učitelé / Předměty), **preferované třídy
učitele** v řešiči, automatické testy (`tests/`).

Ověřeno na seed datech (17 tříd, 426 zástupních žáků, 698 lekcí): **0 tvrdých problémů**,
bez oken ve všech třídách, úvazky nejvýše 21 h; limit 30 s.

### Známá omezení
- seed (učitelé u hodin, dotace, velikosti tříd, volby žáků) je odhad – viz kap. 12;
- přesun v editoru je klik-klik (chybí drag&drop myší);
- místnosti se v řešiči hlídají jen pro odborné učebny „pouze pro“; jazykové a kmenové
  místnosti se dořeší až při přiřazení (případný nedostatek hlásí validace);
- oktávová AJ ve 4 skupinách napříč 8A+8B je zatím modelovaná jako AJa/AJb v každé třídě;
- na Linuxu bez nativních dialogů;
- chybí SQLite, diff roků, srovnání variant, konvertor do Bakalářů.

---

## 12. Otevřené otázky

1. **Prima–tercie**: jak vyřešit 28–30 h vs 25 slotů? (povolit 12:55 /
   snížit dotace / jinak)
2. Limity kvarty/kvinty – **za den, nebo za týden**?
3. 4leté třídy – první A ≈ kvinta (schváleno?), čtvrtá A existuje? Mají mít 2. jazyk
   odděleně (4 h), nebo se míchat s 8letými?
4. ~~**Semináře**: pošlete čísla voleb studentů~~ ✔ import výstupu nástroje na semináře + ⊞ Bloky.
   Pošlete skutečný soubor pro kontrolu formátu.
5. Potvrzení L/S interpretace (předpoklad: lichý/sudý týden).
6. **Velikosti tříd a skupin** (kapacity MJP 14, PAJ/PFJ 17!).
7. Jména učitelů (doplnit ze seznamu zaměstnanců) a jejich skutečné přiřazení k hodinám.
8. Přesné hodinové dotace (učební plán) – seed je odhad z PDF.
9. Vzorový soubor Bakalářů pro importní konvertor.
10. Dny, kdy jednotliví učitelé neučí (částečné úvazky) – lze zadat v Číselníky → Učitelé → 📅.
11. Je kvarta B dobíhající třída (škola přešla z 2 na 1 třídu v 8letém studiu)?
12. **Jídelna**: kolik strávníků zvládne jedna vlna (cca 20–30 min) a kolik % žáků obědvá? Otevírá
    jídelna před 11:40? Sdílí ji jiná škola (pevné časy)?
13. Další dvojice předmětů, které nesmí být ve stejný den (kromě TV × PL)?

---

## 13. Roadmapa

**Nejbližší (navržené pořadí):**
1. ⭐ Import skutečného rozvrhu z CSV jako výchozího stavu.
2. Více variant rozvrhu (3–5 × generování, srovnávací tabulka skóre) – *varianty přes
   semínko už jsou, chybí srovnání.*
3. ~~Jezdci vah penalizací v Nastavení.~~ ✔ hotovo
4. ~~Volné dny učitelů – klikací mřížka 5×12.~~ ✔ hotovo
5. Diff mezi roky (dotace, učitelé, učebny).
6. Drag & drop myší.
7. SQLite pro víceletou historii.
8. Konvertor do Bakalářů (až bude vzorek).

**Nápady na později:** PDF export přímo, validátor voleb proti kolizím před generováním,
návrh rozdělení přeplněných skupin s volbou učitele, „okolí“ hodiny (kontext
učitele/místnosti v tooltipu), pravidla sousednosti (TV hala ↔ šatny), automatické
vyrovnání týdenní zátěže učitelů, oktávová AJ ve 4 skupinách napříč třídami.

---

## 14. Technické poznámky pro vývojáře

- **egui 0.27**: `ComboBox::from_id_source` (0.28+ používá `from_id_salt`),
  `ScrollArea::id_source`, `DragValue::clamp_range`; automatické uložení v
  `eframe::App::on_exit` (`on_close_event` už v 0.27 není).
- **Glyfy**: výchozí fonty egui nemají např. 🗓 🗂 📋 ↶ ⇥ ✕ → ✎ – používají se 📅 📇 📊 ↩ ➡ ✖ 📝.
- **Textová pole v `Grid`** se smrskávají – používat `ui.add_sized([w, h], TextEdit…)`.
- **Borrow checker v egui**: pattern „fází“ – akce (výběr, přesun, pin…) se během výpůjčky
  `&self.p.rozvrh` sbírají do `Vec<Akce>` a aplikují po jejím uvolnění;
  disjoint field borrows (`self.p.piny` × `self.p.rozvrh`, destrukturování `Skola`)
  fungují díky precise capture (edition 2021).
- **Solver ve vlákně**: `JoinHandle::is_finished()` + `Arc<Mutex<Prubeh>>` (průběh)
  + `Arc<AtomicBool>` (zastavit) + `ctx.request_repaint_after(150 ms)`.
- **Konflikty jako bitset** jednotek `Vec<Vec<u64>>` – O(1) test.
- **Inkrementální cena**: obsazenost per profil/učitel/klíč předmětu/bazén místností ×
  60 slotů, cache ceny per (entita, den); přesun přepočítá jen dotčené dny.
- **Lekce vs. jednotky**: vše (sync, L/S, Dív/Chl) je řešeno jednotkami;
  L/S pár = 2 lekce s `par` (různé týdny → vzájemně nekolidují), každá má vlastního
  učitele a místnost.
- **Stabilní `klic`** lekce = `P|trida|predmet|skupina|ord` / `M|menu|volba|i`
  → piny přežijí přegenerování; při změně dat se neaplikované piny nahlásí.
- **rfd** dialogy jen v UI vlákně a jen na Windows/macOS (Linux → `./export`).
- CSV s BOM (`\u{FEFF}`) kvůli diakritice v Excelu.
- Formátování: `cargo fmt` (`rustfmt.toml`: max_width 120), `cargo clippy --all-targets` čisté.

---

## 15. Inventář zdrojových souborů

### Rust aplikace (v0.5)

| Soubor | Obsah |
|---|---|
| `Cargo.toml` | eframe 0.27, serde, serde_json, rand 0.8, calamine (čtení Excelu), rust_xlsxwriter (zápis .xlsx), encoding_rs (Windows-1250), rfd 0.14 (jen Windows/macOS) |
| `src/lib.rs` | kořen knihovny `rozvrhovnik` (bez GUI) |
| `src/main.rs` | bootstrap eframe + režim příkazové řádky `--generuj` |
| `src/napoveda.txt` | text `--help` |
| `src/data.rs` | konstanty, model, projekt (JSON), seed školy (třídy, místnosti, učitelé, předměty, plán, menu) |
| `src/rok.rs` | 15:15, odhad pohlaví, přechod do nového roku, import voleb/katalogů, doplnění voleb, ✂ rozdělení skupiny, zástupní žáci |
| `src/solver.rs` | LekceInfo/Vysledek, postav_lekce, model (profily, konflikty, bazény místností), generuj (greedy + žíhání + slep_bloky), piny, L/S, auto-uvolnění, místnosti, zátěže |
| `src/validation.rs` | zkontroluj() – plná validace per žák a týden + statistiky tříd |
| `src/export.rs` | HTML tisk (třídy, učitelé, žáci), CSV, XML, textový report |
| `src/studenti_io.rs` | import (txt, csv, xlsx/xls/ods, json, dbf) a export (csv, json, xlsx, txt) seznamu žáků, aliasy tříd, pohlaví z RČ, plán a aplikace importu (režimy) |
| `src/app/import_ui.rs` | GUI importu/exportu žáků: lišta, náhled, výběr souboru, přetažení, načtení do průvodce |
| `src/profily.rs` | odvozená data profilů: kdo co učí, ve kterých třídách, kolik hodin (plán + volby) |
| `src/app.rs` | UI (obrazovky vč. profilů učitelů a předmětů, průvodce, editace, exporty, piny, zátěže, rozdělení skupin, jídelna v Reportu) |
| `src/skupiny_io.rs` | import rozdělení do seminářů/skupin (formát školního nástroje), přenos do menu voleb |
| `src/bloky.rs` | bloky seminářů: graf kolizí, klika, DSatur, přesné obarvení, vyrovnání, kdo brání, export CSV |
| `src/jidelna.rs` | vlny obědů, polední pauza, zatížení jídelny v hotovém rozvrhu |
| `src/demo.rs` | demo data se smyšlenými jmény (`--demo-data`) |
| `src/app/skupiny_ui.rs` | GUI importu skupin a obrazovka ⊞ Bloky |
| `tests/zakladni.rs` | testy: 15:15, odhad pohlaví, seed, přechod roku, volby, validace, řešič, piny, uložení, profily |
| `tests/studenti_io.rs` | testy importu/exportu žáků: parsování, kódování, Excel, režimy, zpětný import všech formátů |
| `tests/preference_ucitele.rs` | preferované třídy učitele v řešiči, zpětná kompatibilita JSON |
| `tests/skupiny_io.rs` | import skupin: formát nástroje, slučování, přenos do menu, chybějící žáci a třídy |
| `tests/bloky.rs` | bloky seminářů: kolize, nejmenší počet bloků, kontrola, paralelní plánování, export ➡ import, učitelé po hodinách, kdo brání |
| `tests/pravidla.rs` | TV × PL, vlny obědů a polední pauza, jídelna v řešiči, krátké dny a dlouhá okna, profily s učiteli po hodinách |
| `tests/data/*.dbf` | malé DBF soubory se smyšlenými jmény (CP852, Windows-1250) |
| `docs/superpowers/specs/` | návrhy funkcí (profily učitelů a předmětů) |

---

*Dokument vygenerován z konverzace k projektu „Generátor rozvrhů GFP Neratovice“ a doplněn
podle implementace. Data v seedu jsou odvozená z předběžného rozvrhu 2026/27 a čekají na
ověření (viz kap. 12).*
