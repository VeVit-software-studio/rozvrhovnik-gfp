# Profily učitelů a předmětů + preference učitele v solveru

Datum: 2026-09-25

## Záměr

Přidat do aplikace dvě nové obrazovky (záložky) v hlavní navigaci:

1. **Učitelé** – profil každého učitele: co učí, jaké třídy, jaký úvazek,
   jeho rozvrh, a nově editovatelné **preferované třídy**.
2. **Předměty** – profil každého předmětu: kdo ho učí (a v kterých
   třídách), a preferovaná/povinná učebna (existující koncept
   `kmenova_ok` + `specialni_mistnosti`, jen přehledně na jednom místě).

Preferované třídy učitele navíc **reálně ovlivňují generování rozvrhu**:
solver bude preferovaným třídám daného učitele přednostně dávat lepší
(ne brzké/pozdní) hodiny na úkor tříd, které učitel nepreferuje.

Vše je odvozeno z existujících dat (`Skola.plan`, `SkolniRok.menu`,
`Vysledek`) kromě jednoho nového pole (`Ucitel.preferovane_tridy`) a
jedné nové váhy (`Vahy.ucitel_preference`). Číselníky (existující
obrazovka pro přidávání/mazání položek) zůstávají beze změny.

## Rozhodnutá zadání (z brainstormingu)

- "Preferovaná/povinná třída" u **předmětu** = učebna (místnost), ne
  školní třída. Řeší se stávajícími poli `Predmet.kmenova_ok` a
  `Predmet.specialni_mistnosti` – žádné nové pole.
- "Preferovaná třída" u **učitele** = skutečná školní třída a **má
  reálně ovlivnit generování rozvrhu** (ne jen informační poznámka).
- Nové profily jdou do **dvou nových samostatných záložek** v horní
  navigaci, vedle stávajících (Rozvrh, Studenti, Volitelné, Učební
  plán, Číselníky, Nastavení, Report).

## 1. Datový model (`src/data.rs`)

```rust
pub struct Ucitel {
    pub id: String,
    pub jmeno: String,
    pub volno: Vec<u8>,
    pub max_den: u8,
    pub preferovana_mistnost: Option<String>,
    /// Třídy, které učitel preferuje – solver jim dává přednostně
    /// lepší (ne brzké/pozdní) hodiny na úkor ostatních tříd učitele.
    #[serde(default)]
    pub preferovane_tridy: Vec<String>,
}
```

```rust
pub struct Vahy {
    // ... stávající pole ...
    /// Penalizace za brzkou/pozdní hodinu preferované třídy učitele.
    #[serde(default)]
    pub ucitel_preference: u32,
}
```

- `#[serde(default)]` na obou → staré `skola_data.json` se načtou beze
  změny (prázdný `Vec` / `0`), žádná migrace není potřeba.
- `Vahy` implementuje `Default` ručně (viz `src/data.rs:196`) – přidat
  `ucitel_preference: 3` do výchozí instance. `#[serde(default)]` na
  poli navíc pokryje případ deserializace starého JSON, kde by celé
  pole `vahy` mohlo chybět (to už dnes řeší `#[serde(default)]` na
  `Nastaveni.vahy`, viz řádek 222).
- Místo přidání: `Ucitel` struct v `src/data.rs:109-118`, `Vahy` struct
  v `src/data.rs:185-194` (výchozí hodnota v `impl Default for Vahy`,
  `src/data.rs:196-208`).
- V číselníku (`obrazovka_ciselniky`, `src/app.rs:924-930`) se při
  vytváření nového učitele doplní `preferovane_tridy: vec![]`.

## 2. Solver – mechanismus preference (`src/solver.rs`)

### Kontext

Solver nikdy nerozhoduje, který učitel učí kterou třídu – to je fixní
z Učebního plánu / Menu. Rozhoduje jen **kdy** (den/slot) a **kde**
(učebna) se každá už fixní dvojice (učitel, třída, předmět) odehraje.
Cena řešení (`Stav::obj`) = `hard * H + soft`, kde `soft` se udržuje
inkrementálně při každém přesunu jednotky (`presun`, řádek 1013) přes
denní náklady po entitách: `prof_den` (žáci/třída), `ucitel_den`
(učitel), `klic_den` (předmět).

### Návrh

Zrcadlí se existující `prof_den` mechanismus (penalizace brzké/pozdní
hodiny třídy, `src/solver.rs:889-908`), ale na straně učitele a jen
pro jednotky, jejichž třída je v `preferovane_tridy` daného učitele.

1. **Model** (nové pole `uteach_pref: Vec<Vec<u32>>` ve `struct Model`,
   `src/solver.rs:492-514`; naplnění ve stavbě modelu vedle `uteach`,
   `src/solver.rs:625-640`): pro každou jednotku `u` a každého jejího
   učitele `t` (index do `teach_max`) spočítat, zda třída(y) jednotky
   (`lekce[i].tridy`) protínají `skola.ucitel(id).preferovane_tridy`.
   Pokud ano a `preferovane_tridy` není prázdné, zapsat `t` do
   `uteach_pref` (stejný tvar jako `uteach`, ale jen podmnožina).
   Učitelé bez preferencí do `uteach_pref` nikdy nepřidají nic → nulová
   režie i efekt.

2. **Stav** (nové pole `teach_pref_occ: Vec<[u8; N_SLOTU]>` ve
   `struct Stav`, `src/solver.rs:823-837`; inicializace v `Stav::new`,
   `src/solver.rs:866-883`, na nulu stejně jako `teach_occ`).

3. **`vloz`/`odeber`** (`src/solver.rs:944-1010`): vedle stávající
   smyčky `for &x in &m.uteach[u] { self.teach_occ[x][t] += 1/-= 1 }`
   přidat analogickou smyčku `for &x in &m.uteach_pref[u] { self.teach_pref_occ[x][t] += 1/-= 1 }`.

4. **`ucitel_den`** (`src/solver.rs:928-933`): vedle stávající penalizace
   oken (`ucitel_okno`) spočítat z `teach_pref_occ[t]` pro daný den:
   `pref_rane` (je slot 0 obsazen preferovanou třídou?) a `pref_odpo`
   (kolik odpoledních slotů je obsazeno preferovanou třídou), stejně
   jako to `prof_den` počítá z `occ`. Přidat do vraceného `soft`:
   `m.vahy.ucitel_preference as i64 * (pref_rane as i64 + pref_odpo as i64)`.

   Efekt: pokud hodina preferované třídy padne na brzkou/pozdní dobu,
   připočte se penalizace navíc → solveru se vyplatí takové hodiny
   přesouvat na "lepší" časy a na okraj dne dávat raději hodiny
   nepreferovaných tříd téhož učitele.

5. **Nastavení** (`src/app.rs`, `obrazovka_nastaveni`, cca řádek 1158-1169,
   sekce "Váhy měkkých kritérií"): přidat
   `ui.add(egui::Slider::new(&mut v.ucitel_preference, 0..=20).text("preferovaná třída učitele"));`

### Zvažované alternativy (zamítnuty)

- **Priorita v hladovém/počátečním rozřazení** – jednoduché, ale
  simulované žíhání (`zihani`, řádek 1195) efekt s velkou
  pravděpodobností smaže, protože žíhání zná jen aktuální cenu stavu,
  ne historii/pořadí vkládání. Nespolehlivé, zamítnuto.
- **Obecné "komfortní skóre" přes celý den** (nejen brzo/pozdě, i okna
  apod.) – obecnější, ale výrazně invazivnější zásah do `prof_den`-like
  logiky a vyšší riziko nechtěných interakcí se stávajícími váhami.
  Nad rámec zadání, zamítnuto.

## 3. UI – nové obrazovky (`src/app.rs`)

### Navigace

Rozšířit `enum Obrazovka` (řádek 26-34) o `Ucitele` a `Predmety`,
zařadit do pole `taby` v `horni_lista` (`src/app.rs:337-344`) mezi
"Učební plán" a "Číselníky":

```
Rozvrh | Studenti | Volitelné | Učební plán | Učitelé | Předměty | Číselníky | Nastavení | Report
```

Nový stav v `RozvrhApp`: `sel_profil_ucitel: String`,
`sel_profil_predmet: String` (inicializace stejně jako `sel_ucitel`
řádek 125 – první položka ze seznamu).

### Obrazovka „Učitelé"

Master-detail layout (`egui::SidePanel` nebo dvousloupcové
`ui.columns`): vlevo scrollovací seznam učitelů (`id` – `jméno`,
klikací `selectable_label`), vpravo profil vybraného:

- Jméno, zkratka (needitovatelné zde – editace zůstává v Číselníkách).
- **Úvazek**: `solver::zateze(&self.p.skola, &self.p.rok)` (stejná
  funkce jako v Reportu, `src/app.rs:1203`), vyfiltrováno na
  vybraného učitele, zobrazit jako "X h/týden".
- **Vyučované předměty**: sjednocení (`BTreeSet`) z
  `p.skola.plan.iter().filter(|h| h.struktura.ucitele().contains(&id)).map(|h| h.predmet.clone())`
  a z `p.rok.menu.iter().flat_map(|m| &m.volby).filter(|v| v.ucitel == id).map(|v| v.predmet.clone())`.
- **Vyučované třídy**: obdobně – `h.trida` z plánu (kde učitel
  vyučuje) + `m.tridy` z menu, kde se učitel objevuje v nějaké volbě.
  Zobrazit jako názvy (`skola.nazev_tridy`).
- Tlačítko **„📅 Zobrazit rozvrh"** – nastaví
  `self.obr = Obrazovka::Rozvrh; self.druh = Druh::Ucitel; self.sel_ucitel = id.clone();`
  a přepne na existující obrazovku Rozvrh (žádná duplikace vykreslovací
  logiky – ta je v `Druh::Ucitel` větvi, `src/app.rs:415-416, 613`).
- **Preferované třídy** (editovatelné): `ui.menu_button` se seznamem
  všech tříd a checkboxy, stejný vzor jako `specialni_mistnosti` u
  předmětu v Číselníkách (`src/app.rs:1098-1109`), zapisuje přímo do
  `ucitel.preferovane_tridy`.

### Obrazovka „Předměty"

Stejný master-detail vzor, vlevo seznam předmětů. Vpravo profil:

- Název (editovatelný, `TextEdit`).
- „Smí do kmenové" (checkbox) + odborné místnosti (`menu_button` s
  checkboxy) – **stejná pole a stejné widgety** jako dnes v
  Číselníkách (`src/app.rs:1092-1109`), jen na jednom místě spolu s
  odvozeným přehledem níže.
- **Vyučující**: seznam řádků "třída – učitel" odvozený z
  `p.skola.plan` (položky s tímto předmětem, přes
  `Struktura::ucitele()` a `PlanovaHodina.trida`) a z `p.rok.menu`
  (volby s tímto předmětem, `menu.tridy` + `volba.ucitel`).
- **Třídy/skupiny, kde se předmět učí**: odvozeno stejně, zobrazeno
  jako prostý seznam názvů tříd/menu.

Obě obrazovky jsou čistě odvozené z existujících dat (kromě
`preferovane_tridy`), takže nehrozí nekonzistence se zbytkem aplikace
(Číselníky, Učební plán) – žádná data se needuplikují.

## 4. Testování

- Nový unit test v `tests/zakladni.rs` (nebo nový soubor
  `tests/preference_ucitele.rs`): malá uměle sestavená škola o 2
  třídách a 1 učiteli, který učí oba, s `preferovane_tridy` nastavenou
  na jednu z nich. Po vygenerování rozvrhu (`solver::vygeneruj` /
  ekvivalent) ověřit, že preferovaná třída dostává v součtu méně
  brzkých/pozdních hodin téhož učitele než nepreferovaná (statistická
  kontrola na malém, deterministickém seedu – ne nutně 100% v každém
  slotu, ale měřitelný posun oproti běhu s `ucitel_preference = 0`).
- Existující testy v `tests/zakladni.rs` a `cargo test` musí projít
  beze změny (ověřuje zpětnou kompatibilitu `#[serde(default)]`).
- Ruční ověření v UI: otevřít obě nové záložky, zkontrolovat, že
  zobrazená data (úvazek, předměty, třídy) odpovídají tomu, co je vidět
  v Učebním plánu / Reportu pro stejného učitele/předmět.

## Rozsah / co záměrně NENÍ součástí

- Číselníky se nemění (zůstávají jako dnes pro přidávání/mazání).
- Žádné nové pole u `Predmet` (řeší se existujícími poli).
- Preference učitele neovlivňuje nic jiného než brzké/pozdní hodiny
  (ne okna, ne rozložení předmětu) – rozšíření na obecnější "komfortní
  skóre" je vědomě mimo rozsah (viz zamítnuté alternativy výše).
