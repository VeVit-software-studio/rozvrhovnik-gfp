# Generátor rozvrhů – Gymnázium Františka Palackého, Neratovice

**Kompletní dokumentace projektu** – konsoliduje celou konverzaci (zadání, analýzu, architekturu,
všechny verze programu, otevřené otázky a plán do budoucna) a popisuje aplikaci v tomto repozitáři.

- Verze dokumentu: 1.1
- Stav aplikace: Rust desktop v0.4 – **implementováno v tomto repozitáři** (viz kap. 4 – Verze)
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
| v0.4 | Rust | **L/S střídavé týdny, 📌 piny, kapacity+✂ rozdělení skupin, osobní rozvrhy žáků, zátěže učitelů, tisk celé školy, slepování bloků + semináře dopoledne, 15:15 dle pohlaví** + režim příkazové řádky, váhy v Nastavení, volno učitelů, testy | **tento repozitář** |

Aplikace v repozitáři je nová, ucelená implementace funkcí v0.2–v0.4 (inventář viz kap. 15).

---

## 5. Architektura

```
┌───────────────────────────── aplikace (eframe/egui) ─────────────────────┐
│  hlavní panel: Rozvrh | Studenti | Volitelné | Plán | Číselníky |         │
│                Nastavení | Report      + „✚ Vytvořit nový rok…“          │
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
- `Ucitel { id, jmeno, volno[], max_den, preferovana_mistnost }` – `volno` = sloty (den*12+slot)
- `Predmet { id, nazev, specialni_mistnosti[], kmenova_ok }`
- `PlanovaHodina { trida, predmet, hodin, struktura, dvoj }`
  - `Struktura`:
    - `CelouTridou(učitel)`
    - `Rozdeleno { a, b }` – AJa/AJb (hodiny skupin jsou nezávislé; řešič je páruje přes okna)
    - `Pohlavi { div, chl }` – TV/PL (vynuceně paralelní slot)
    - `Stridave { l: Pol, s: Pol }` – L/S celou třídou
    - `StridaveSkupiny { a: [Pol;2], b: [Pol;2] }` – L/S per AJa/AJb
    - `Pol { predmet, ucitel }`
- `Nastaveni { casovy_limit_s, limity_za_den, odpoledne_od, auto_uvolneni, seed, vahy }`

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
| sek-volba | sekunda | INF / ČJL | 2 | ne | 1 |
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

Penalizace žáků se násobí počtem žáků profilu; tvrdé porušení má váhu 50 000.

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
nejdelší blok, průměrná okna).

---

## 8. Uživatelská příručka

### 8.1 Horní lišta

`📅 Rozvrh | 👥 Studenti | ☑ Volitelné | 📚 Učební plán | 📇 Číselníky | ⚙ Nastavení | 📊 Report`
+ **„✚ Vytvořit nový rok…“** (zelené) + 💾 Uložit + 📁 Otevřít; druhý řádek: rok,
**▶ Generovat** / průběh řešení (fáze, čas, iterace, skóre) + ⏹ Zastavit, stavová zpráva.

### 8.2 Průvodce novým rokem (4 kroky)

0. **Rok** – označení nového roku (automaticky další, např. 2027/28).
1. **Noví žáci** – jména po řádcích do textových polí vstupních tříd (prima, první A);
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

### 8.5 Volitelné

Menu selector (+ nové / ✖ smazat) → vlastnosti menu (název, h/týden, 2h bloky, voleb na
žáka, třídy) → katalog voleb (název, předmět, učitel, počet žáků + kapacita,
✂ rozdělit – druhá polovina žáků k vybranému učiteli) → tabulka žák × N voleb (comby)
→ obsazenost a validace („X žáků nemá kompletní volby“).
Tlačítka: ⬇ Katalog / ⬇ Volby z minulého roku (je-li historie), 📝 Doplnit chybějící.

### 8.6 Učební plán

Per třída: předmět, hodin, 2h blok, rozdělení (celá třída / AJa-AJb /
dívky-chlapci / **L↔S třída / L↔S skupiny** s editací párů předmět+učitel),
+ přidat/✖; součet hodin plánu + volitelných.

### 8.7 Číselníky

Třídy (název, ročník, typ, kmenová, následník, výchozí limity), Místnosti (kapacita,
kmenová), Učitelé (jméno, max h/den, preferovaná učebna, **📅 volno – klikací mřížka
5×12**), Předměty (smí do kmenové, odborné místnosti). Přidání přes „nové ID“.

### 8.8 Nastavení

Časový limit solveru (logaritmický posuvník), semínko varianty (🎲), limity za den /
za týden, začátek odpoledne, auto-uvolnění, přehled pravidla 7 h, **váhy měkkých
kritérií** (posuvníky), limity tříd (přepis raných / odpoledních; odškrtnuto = ∞),
následníci tříd.

### 8.9 Report

**Zátěže učitelů** (vždy – i před generováním; semafor ≥26 oranžová, ≥31
červená), varování, problémy, statistiky tříd (hodiny, 7:20 dny, odpolední,
nejdelší blok, okna).

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
z instalace školy → dodá se konvertor „⬇ Bakaláři“.

---

## 11. Stav

### Hotovo
✔ P1–P5 (s auto-uvolněním), L/S střídavé týdny, piny, kapacity+rozdělení
skupin, osobní rozvrhy, zátěže učitelů, tisk (třídy, učitelé, žáci), slepování bloků,
semináře dopoledne, 15:15 dle pohlaví, editace+undo+validace, průvodce novým rokem,
import voleb/katalogů z minulého roku, perzistence, vláknové generování se zastavením,
váhy v Nastavení, volno učitelů (mřížka 5×12), varianty přes semínko, režim příkazové
řádky, automatické testy (`tests/zakladni.rs`).

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
4. **Semináře**: pošlete čísla voleb studentů → přesné plánování bez překryvů.
5. Potvrzení L/S interpretace (předpoklad: lichý/sudý týden).
6. **Velikosti tříd a skupin** (kapacity MJP 14, PAJ/PFJ 17!).
7. Jména učitelů (doplnit ze seznamu zaměstnanců) a jejich skutečné přiřazení k hodinám.
8. Přesné hodinové dotace (učební plán) – seed je odhad z PDF.
9. Vzorový soubor Bakalářů pro importní konvertor.
10. Dny, kdy jednotliví učitelé neučí (částečné úvazky) – lze zadat v Číselníky → Učitelé → 📅.
11. Je kvarta B dobíhající třída (škola přešla z 2 na 1 třídu v 8letém studiu)?

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

### Rust aplikace (v0.4)

| Soubor | Obsah |
|---|---|
| `Cargo.toml` | eframe 0.27, serde, serde_json, rand 0.8, rfd 0.14 (jen Windows/macOS) |
| `src/lib.rs` | kořen knihovny `rozvrhovnik` (bez GUI) |
| `src/main.rs` | bootstrap eframe + režim příkazové řádky `--generuj` |
| `src/napoveda.txt` | text `--help` |
| `src/data.rs` | konstanty, model, projekt (JSON), seed školy (třídy, místnosti, učitelé, předměty, plán, menu) |
| `src/rok.rs` | 15:15, odhad pohlaví, přechod do nového roku, import voleb/katalogů, doplnění voleb, ✂ rozdělení skupiny, zástupní žáci |
| `src/solver.rs` | LekceInfo/Vysledek, postav_lekce, model (profily, konflikty, bazény místností), generuj (greedy + žíhání + slep_bloky), piny, L/S, auto-uvolnění, místnosti, zátěže |
| `src/validation.rs` | zkontroluj() – plná validace per žák a týden + statistiky tříd |
| `src/export.rs` | HTML tisk (třídy, učitelé, žáci), CSV, XML, textový report |
| `src/app.rs` | UI (obrazovky, průvodce, editace, exporty, piny, zátěže, rozdělení skupin) |
| `tests/zakladni.rs` | testy: 15:15, odhad pohlaví, seed, přechod roku, volby, validace, řešič, piny, uložení |

---

*Dokument vygenerován z konverzace k projektu „Generátor rozvrhů GFP Neratovice“ a doplněn
podle implementace. Data v seedu jsou odvozená z předběžného rozvrhu 2026/27 a čekají na
ověření (viz kap. 12).*
