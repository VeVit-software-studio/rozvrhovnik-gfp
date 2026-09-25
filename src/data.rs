//! Datový model školy, školního roku a výchozí (seed) data odvozená
//! z předběžného rozvrhu 2026/27 (viz README, kap. 2 – položky s ⚠ je nutno ověřit).

use crate::solver::Vysledek;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;

// ───────────────────────────── konstanty ─────────────────────────────

pub const DNY: usize = 5;
pub const SLOTU: usize = 12;
pub const N_SLOTU: usize = DNY * SLOTU;
pub const NAZVY_DNU: [&str; DNY] = ["Po", "Út", "St", "Čt", "Pá"];
pub const CASY: [&str; SLOTU] =
    ["7:20", "8:10", "9:00", "10:00", "10:55", "12:00", "12:55", "13:45", "14:35", "15:25", "16:15", "17:05"];
pub const CASY_KONEC: [&str; SLOTU] =
    ["8:05", "8:55", "9:45", "10:45", "11:40", "12:45", "13:40", "14:30", "15:20", "16:10", "17:00", "17:50"];
/// Pravidlo P5: nejvýše 7 vyučovacích hodin v kuse, pak nutná pauza ≥ 1 hodina.
pub const MAX_RUN: usize = 7;
/// Nejvýše 8 hodin za den pro žáka / třídu.
pub const MAX_DEN_TRIDA: usize = 8;
/// Nejvýše 2 hodiny stejného předmětu (a skupiny) za den.
pub const MAX_STEJNY_PREDMET_DEN: usize = 2;

pub fn den_a_slot(s: usize) -> (usize, usize) {
    (s / SLOTU, s % SLOTU)
}

// ───────────────────────────── škola ─────────────────────────────

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum TypStudia {
    #[default]
    Osmilete,
    Ctyrlete,
}

impl TypStudia {
    pub fn nazev(self) -> &'static str {
        match self {
            TypStudia::Osmilete => "8leté",
            TypStudia::Ctyrlete => "4leté",
        }
    }
}

/// Limity raných (7:20) a odpoledních hodin. `None` = bez limitu.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Omezeni {
    pub rane: Option<u8>,
    pub odpoledni: Option<u8>,
}

impl Omezeni {
    /// Požadavek P1 podle efektivního ročníku.
    pub fn pro_rocnik(efektivni_rocnik: u8) -> Omezeni {
        match efektivni_rocnik {
            0..=3 => Omezeni { rane: Some(0), odpoledni: Some(0) },
            4 => Omezeni { rane: Some(1), odpoledni: Some(1) },
            5 => Omezeni { rane: Some(2), odpoledni: Some(2) },
            _ => Omezeni { rane: None, odpoledni: None },
        }
    }

    pub fn popis(&self) -> String {
        let f = |o: Option<u8>| o.map(|x| x.to_string()).unwrap_or_else(|| "∞".into());
        format!("raných {} / odpoledních {}", f(self.rane), f(self.odpoledni))
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Trida {
    pub id: String,
    pub nazev: String,
    pub rocnik: u8,
    pub typ: TypStudia,
    pub kmenova: String,
    pub naslednik: Option<String>,
    #[serde(default)]
    pub omezeni_prepsat: Option<Omezeni>,
}

impl Trida {
    /// 4leté třídy se pro omezení a volitelné předměty chovají jako vyšší gymnázium: 1A→5 … 4A→8.
    pub fn efektivni_rocnik(&self) -> u8 {
        match self.typ {
            TypStudia::Osmilete => self.rocnik,
            TypStudia::Ctyrlete => self.rocnik + 4,
        }
    }

    pub fn omezeni(&self) -> Omezeni {
        self.omezeni_prepsat.unwrap_or_else(|| Omezeni::pro_rocnik(self.efektivni_rocnik()))
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Mistnost {
    pub id: String,
    pub nazev: String,
    pub kapacita: u32,
    /// Kmenová učebna – může sloužit libovolnému předmětu, který smí do kmenové.
    #[serde(default)]
    pub kmenova: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Ucitel {
    pub id: String,
    pub jmeno: String,
    /// Sloty (den*12+slot), kdy učitel neučí.
    #[serde(default)]
    pub volno: Vec<u8>,
    pub max_den: u8,
    #[serde(default)]
    pub preferovana_mistnost: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Predmet {
    pub id: String,
    pub nazev: String,
    /// Odborné místnosti v pořadí preference.
    pub specialni_mistnosti: Vec<String>,
    /// Smí se učit i v kmenové učebně (případně jiné volné kmenové – „přeliv“).
    pub kmenova_ok: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Pol {
    pub predmet: String,
    pub ucitel: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Struktura {
    /// Celá třída, jeden učitel.
    CelouTridou(String),
    /// Skupiny AJa / AJb, každá svůj učitel (hodiny skupin jsou nezávislé).
    Rozdeleno { a: String, b: String },
    /// Dívky / chlapci – vynuceně paralelní slot (TV, PL).
    Pohlavi { div: String, chl: String },
    /// Lichý/sudý týden celou třídou (např. DV ↔ VV).
    Stridave { l: Pol, s: Pol },
    /// Lichý/sudý týden zvlášť pro AJa a AJb (např. praktika). Pole = [lichý, sudý].
    StridaveSkupiny { a: [Pol; 2], b: [Pol; 2] },
}

impl Struktura {
    pub fn nazev_druhu(&self) -> &'static str {
        match self {
            Struktura::CelouTridou(_) => "celá třída",
            Struktura::Rozdeleno { .. } => "AJa / AJb",
            Struktura::Pohlavi { .. } => "dívky / chlapci",
            Struktura::Stridave { .. } => "L↔S třída",
            Struktura::StridaveSkupiny { .. } => "L↔S skupiny",
        }
    }

    pub fn ucitele(&self) -> Vec<&str> {
        match self {
            Struktura::CelouTridou(u) => vec![u],
            Struktura::Rozdeleno { a, b } => vec![a, b],
            Struktura::Pohlavi { div, chl } => vec![div, chl],
            Struktura::Stridave { l, s } => vec![&l.ucitel, &s.ucitel],
            Struktura::StridaveSkupiny { a, b } => {
                vec![&a[0].ucitel, &a[1].ucitel, &b[0].ucitel, &b[1].ucitel]
            }
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PlanovaHodina {
    pub trida: String,
    /// Předmět (u střídavých struktur jen popisek, např. „DV/VV“).
    pub predmet: String,
    pub hodin: u8,
    pub struktura: Struktura,
    /// Vyučovat ve dvouhodinových blocích.
    pub dvoj: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Vahy {
    pub okno: u32,
    pub rana: u32,
    pub odpoledne_nizsi: u32,
    pub odpoledne_vyssi: u32,
    pub seminar_odpoledne: u32,
    pub rozlozeni: u32,
    pub ucitel_okno: u32,
}

impl Default for Vahy {
    fn default() -> Self {
        Vahy {
            okno: 5,
            rana: 3,
            odpoledne_nizsi: 1,
            odpoledne_vyssi: 2,
            seminar_odpoledne: 2,
            rozlozeni: 2,
            ucitel_okno: 1,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Nastaveni {
    /// Časový limit řešiče v sekundách.
    pub casovy_limit_s: u32,
    /// true = limity P1 platí za den, false = za týden.
    pub limity_za_den: bool,
    /// První odpolední slot (6 = 12:55).
    pub odpoledne_od: u8,
    /// Když limity P1 nestačí na hodinovou dotaci, minimálně je zvýšit (a nahlásit).
    pub auto_uvolneni: bool,
    /// Semínko náhody – jiné číslo = jiná varianta rozvrhu.
    pub seed: u64,
    #[serde(default)]
    pub vahy: Vahy,
}

impl Default for Nastaveni {
    fn default() -> Self {
        Nastaveni {
            casovy_limit_s: 30,
            limity_za_den: true,
            odpoledne_od: 6,
            auto_uvolneni: true,
            seed: 1,
            vahy: Vahy::default(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Skola {
    pub tridy: Vec<Trida>,
    pub mistnosti: Vec<Mistnost>,
    pub ucitele: Vec<Ucitel>,
    pub predmety: Vec<Predmet>,
    pub plan: Vec<PlanovaHodina>,
    #[serde(default)]
    pub nastaveni: Nastaveni,
}

impl Skola {
    pub fn trida(&self, id: &str) -> Option<&Trida> {
        self.tridy.iter().find(|t| t.id == id)
    }
    pub fn ucitel(&self, id: &str) -> Option<&Ucitel> {
        self.ucitele.iter().find(|t| t.id == id)
    }
    pub fn predmet(&self, id: &str) -> Option<&Predmet> {
        self.predmety.iter().find(|t| t.id == id)
    }
    pub fn mistnost(&self, id: &str) -> Option<&Mistnost> {
        self.mistnosti.iter().find(|t| t.id == id)
    }
    pub fn nazev_tridy(&self, id: &str) -> String {
        self.trida(id).map(|t| t.nazev.clone()).unwrap_or_else(|| id.to_string())
    }
    /// Třídy, do kterých nastupují noví žáci: 1. ročník (prima, první A), který není
    /// následníkem žádné jiné třídy. (Třída bez předchůdce ve vyšším ročníku – např. kvarta B
    /// při přechodu z 2 tříd na 1 – je dobíhající, noví žáci do ní nepřicházejí.)
    pub fn vstupni_tridy(&self) -> Vec<String> {
        self.tridy
            .iter()
            .filter(|t| t.rocnik == 1 && !self.tridy.iter().any(|o| o.naslednik.as_deref() == Some(&t.id)))
            .map(|t| t.id.clone())
            .collect()
    }
}

// ───────────────────────────── školní rok ─────────────────────────────

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Pohlavi {
    D,
    CH,
}

impl Pohlavi {
    pub fn zkratka(self) -> &'static str {
        match self {
            Pohlavi::D => "D",
            Pohlavi::CH => "CH",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum SkupinaAj {
    A,
    B,
}

impl SkupinaAj {
    pub fn nazev(self) -> &'static str {
        match self {
            SkupinaAj::A => "AJa",
            SkupinaAj::B => "AJb",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum StavStudenta {
    #[default]
    Aktivni,
    /// Na konci roku propadá – v novém roce zůstává ve stejné třídě.
    Propada,
    /// Odešel ze školy – nezařazuje se do rozvrhu.
    Odesel,
}

impl StavStudenta {
    pub fn nazev(self) -> &'static str {
        match self {
            StavStudenta::Aktivni => "aktivní",
            StavStudenta::Propada => "propadá",
            StavStudenta::Odesel => "odešel",
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Student {
    pub id: u32,
    pub jmeno: String,
    pub trida: String,
    pub skupina_aj: SkupinaAj,
    pub pohlavi: Pohlavi,
    /// menu ➡ zvolené volby
    #[serde(default)]
    pub volby: BTreeMap<String, Vec<String>>,
    #[serde(default)]
    pub status: StavStudenta,
}

impl Student {
    pub fn v_rozvrhu(&self) -> bool {
        self.status != StavStudenta::Odesel
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Volba {
    pub id: String,
    pub nazev: String,
    pub predmet: String,
    pub ucitel: String,
    #[serde(default)]
    pub kapacita: Option<u32>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Menu {
    pub id: String,
    pub nazev: String,
    pub tridy: Vec<String>,
    pub volby: Vec<Volba>,
    pub hodin_tydne: u8,
    pub dvojhodina: bool,
    /// Kolik voleb si žák vybírá. 1 = všechny volby běží paralelně ve stejném slotu.
    pub pocet_voleb: u8,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SkolniRok {
    pub label: String,
    pub studenti: Vec<Student>,
    pub menu: Vec<Menu>,
    pub dalsi_id: u32,
}

impl SkolniRok {
    pub fn student(&self, id: u32) -> Option<&Student> {
        self.studenti.iter().find(|s| s.id == id)
    }
    pub fn nove_id(&mut self) -> u32 {
        let id = self.dalsi_id;
        self.dalsi_id += 1;
        id
    }
}

// ───────────────────────────── projekt (soubor) ─────────────────────────────

/// Obsah souboru `skola_data.json`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Projekt {
    pub skola: Skola,
    pub rok: SkolniRok,
    #[serde(default)]
    pub historie: Vec<SkolniRok>,
    /// klíč lekce ➡ začátek (den*12+slot)
    #[serde(default)]
    pub piny: BTreeMap<String, u8>,
    #[serde(default)]
    pub rozvrh: Option<Vysledek>,
}

impl Projekt {
    pub fn nacti(cesta: &Path) -> Result<Projekt, String> {
        let text = std::fs::read_to_string(cesta).map_err(|e| format!("Nelze číst {}: {e}", cesta.display()))?;
        serde_json::from_str(&text).map_err(|e| format!("Chybný formát {}: {e}", cesta.display()))
    }

    pub fn uloz(&self, cesta: &Path) -> Result<(), String> {
        let text = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        let tmp = cesta.with_extension("json.tmp");
        std::fs::write(&tmp, text).map_err(|e| format!("Nelze zapsat {}: {e}", tmp.display()))?;
        std::fs::rename(&tmp, cesta).map_err(|e| format!("Nelze zapsat {}: {e}", cesta.display()))
    }
}

// ───────────────────────────── seed dat ─────────────────────────────

/// Výchozí projekt: škola, učební plán, menu voleb a zástupní žáci (Žák/Žákyně NN),
/// aby šlo rozvrh hned vygenerovat. Skutečné žáky zadá průvodce „Vytvořit nový rok“.
pub fn vychozi() -> Projekt {
    let (skola, menu) = vychozi_skola_a_menu();
    let mut rok = SkolniRok { label: "2026/27".into(), studenti: vec![], menu, dalsi_id: 1 };
    crate::rok::zastupni_zaci(&skola, &mut rok);
    Projekt { skola, rok, historie: vec![], piny: BTreeMap::new(), rozvrh: None }
}

fn t(id: &str, nazev: &str, rocnik: u8, typ: TypStudia, kmenova: &str, nasl: Option<&str>) -> Trida {
    Trida {
        id: id.into(),
        nazev: nazev.into(),
        rocnik,
        typ,
        kmenova: kmenova.into(),
        naslednik: nasl.map(|s| s.into()),
        omezeni_prepsat: None,
    }
}

fn vychozi_tridy() -> Vec<Trida> {
    use TypStudia::*;
    vec![
        t("prima", "prima", 1, Osmilete, "1G", Some("sekunda")),
        t("sekunda", "sekunda", 2, Osmilete, "2G", Some("tercie")),
        t("tercie", "tercie", 3, Osmilete, "3G", Some("kvartaA")),
        t("kvartaA", "kvarta A", 4, Osmilete, "kvartaA", Some("kvintaA")),
        t("kvartaB", "kvarta B", 4, Osmilete, "kvartaB", Some("kvintaB")),
        t("kvintaA", "kvinta A", 5, Osmilete, "kvintaA", Some("sextaA")),
        t("kvintaB", "kvinta B", 5, Osmilete, "kvintaB", Some("sextaB")),
        t("sextaA", "sexta A", 6, Osmilete, "sextaA", Some("septimaA")),
        t("sextaB", "sexta B", 6, Osmilete, "sextaB", Some("septimaB")),
        t("septimaA", "septima A", 7, Osmilete, "septimaA", Some("oktavaA")),
        t("septimaB", "septima B", 7, Osmilete, "septimaB", Some("oktavaB")),
        t("oktavaA", "oktáva A", 8, Osmilete, "oktavaA", None),
        t("oktavaB", "oktáva B", 8, Osmilete, "oktavaB", None),
        t("1A", "první A", 1, Ctyrlete, "1A", Some("2A")),
        t("2A", "druhá A", 2, Ctyrlete, "2A", Some("3A")),
        t("3A", "třetí A", 3, Ctyrlete, "3A", Some("4A")),
        t("4A", "čtvrtá A", 4, Ctyrlete, "4A", None),
    ]
}

fn vychozi_mistnosti(tridy: &[Trida]) -> Vec<Mistnost> {
    let mut m: Vec<Mistnost> = tridy
        .iter()
        .map(|t| Mistnost {
            id: t.kmenova.clone(), nazev: format!("kmenová {}", t.nazev), kapacita: 30, kmenova: true
        })
        .collect();
    let odborne: [(&str, &str, u32); 22] = [
        ("PSV", "pracovna společenských věd", 30),
        ("LCH", "laboratoř chemie", 24),
        ("PCH", "posluchárna chemie", 28),
        ("PŠJ", "pracovna španělštiny", 20),
        ("PAJ", "pracovna angličtiny", 17),
        ("PNJ", "pracovna němčiny", 24),
        ("PFJ", "pracovna francouzštiny", 17),
        ("VJP", "velká jazyková pracovna", 24),
        ("MJP", "malá jazyková pracovna", 14),
        ("PFB", "posluchárna fyziky a biologie", 28),
        ("PVV", "pracovna výtvarné výchovy", 28),
        ("LFY", "laboratoř fyziky", 24),
        ("PIF", "pracovna informatiky", 24),
        ("AULA", "aula", 80),
        ("HAL1", "hala 1", 30),
        ("HAL2", "hala 2", 30),
        ("BAZ1", "bazén – dráha 1", 20),
        ("BAZ2", "bazén – dráha 2", 20),
        ("BAZ3", "bazén – dráha 3", 20),
        ("BAZ4", "bazén – dráha 4", 20),
        ("PHV", "pracovna hudební výchovy", 28),
        ("DisV", "distanční výuka", 8),
    ];
    for (id, nazev, kap) in odborne {
        m.push(Mistnost { id: id.into(), nazev: nazev.into(), kapacita: kap, kmenova: false });
    }
    m
}

fn p(id: &str, nazev: &str, mistnosti: &[&str], kmenova_ok: bool) -> Predmet {
    Predmet {
        id: id.into(),
        nazev: nazev.into(),
        specialni_mistnosti: mistnosti.iter().map(|s| s.to_string()).collect(),
        kmenova_ok,
    }
}

const JAZYKOVE: [&str; 3] = ["VJP", "MJP", "PAJ"];

fn vychozi_predmety() -> Vec<Predmet> {
    vec![
        p("ČJL", "Český jazyk a literatura", &["PSV"], true),
        p("MAT", "Matematika", &[], true),
        p("AJ", "Anglický jazyk", &["PAJ", "PNJ", "VJP", "MJP"], true),
        p("NJ2", "Německý jazyk", &["PNJ", "VJP", "MJP", "PAJ"], true),
        p("ŠJ2", "Španělský jazyk", &["PŠJ", "VJP", "MJP", "PAJ"], true),
        p("FJ2", "Francouzský jazyk", &["PFJ", "VJP", "MJP", "PAJ"], true),
        p("LAT", "Latina", &["PFJ"], true),
        p("DĚ", "Dějepis", &["PSV"], true),
        p("ZE", "Zeměpis", &["PSV"], true),
        p("OV", "Občanská výchova", &["PSV"], true),
        p("ZSV", "Základy společenských věd", &["PSV"], true),
        p("FY", "Fyzika", &["PFB"], true),
        p("CHE", "Chemie", &["PCH", "PFB"], true),
        p("BIO", "Biologie", &["PFB", "PCH"], true),
        p("INF", "Informatika", &["PIF"], false),
        p("TV", "Tělesná výchova", &["HAL1", "HAL2"], false),
        p("PL", "Plavání", &["BAZ1", "BAZ2", "BAZ3", "BAZ4"], false),
        p("HV", "Hudební výchova", &["PHV"], false),
        p("VV", "Výtvarná výchova", &["PVV"], false),
        p("DV", "Dramatická výchova", &["AULA"], true),
        p("EV", "Estetická výchova", &["AULA"], true),
        p("PCBI", "Praktikum z biologie", &["PCH"], false),
        p("PCFY", "Praktikum z fyziky", &["LFY"], false),
        p("PCCH", "Praktikum z chemie", &["LCH"], false),
        p("IS", "Individuální studium", &["DisV"], false),
        p("AS", "Asistované studium", &["DisV"], false),
        // semináře septimy/oktávy (názvy ⚠ odhad podle zkratek z rozvrhu)
        p("SMA", "Seminář z matematiky", &[], true),
        p("SDG", "Deskriptivní geometrie", &[], true),
        p("SFY", "Seminář z fyziky", &["PFB"], true),
        p("SCHE", "Seminář z chemie", &["PCH"], true),
        p("SBCH", "Seminář z biochemie", &["PCH"], true),
        p("SPCH", "Praktická chemie", &["LCH"], false),
        p("SBR", "Seminář z biologie", &["PFB", "PCH"], true),
        p("SČD", "Seminář z českých dějin", &["PSV"], true),
        p("SLR", "Literární seminář", &["PSV"], true),
        p("SPP", "Seminář psychologie", &["PSV"], true),
        p("SFAV", "Filmová a audiovizuální výchova", &["PSV", "AULA"], true),
        p("SZSV", "Seminář ze společenských věd", &["PSV"], true),
        p("SZE", "Seminář ze zeměpisu", &["PSV"], true),
        p("SAJ", "Konverzace v angličtině", &JAZYKOVE, true),
        p("PRG", "Programování", &["PIF"], false),
        p("SEK", "Seminář ekonomie", &["PIF"], false),
    ]
}

/// (zkratka, jméno, aprobace). Jména známe jen u třídních – ostatní „(doplnit)“ ⚠.
const UCITELE: [(&str, &str, &[&str]); 42] = [
    ("SOJ", "Sojková", &["ČJL", "DĚ"]),
    ("ŠAF", "Šafránková", &["MAT", "FY"]),
    ("SLO", "Sloupová", &["AJ", "ČJL", "FJ2"]),
    ("HAM", "Hamerníková", &["MAT", "ZE"]),
    ("UHR", "Uhrinčaťová", &["NJ2", "AJ"]),
    ("PAŘ", "Pařík", &["DĚ", "ZSV", "OV"]),
    ("LIP", "Lipovská", &["ČJL", "ZSV", "SFAV"]),
    ("ŠMK", "(doplnit)", &["MAT", "INF", "SDG"]),
    ("TES", "Tesař", &["FY", "MAT", "PCFY"]),
    ("ČER", "Červenková", &["BIO", "CHE", "PCBI", "SBCH"]),
    ("TRO", "Trojanová", &["AJ", "ZE", "SAJ", "ŠJ2"]),
    ("VAI", "(doplnit)", &["TV", "BIO"]),
    ("HOR", "Horáková", &["DĚ", "ČJL", "SČD"]),
    ("SAU", "Sauerová", &["AJ", "ŠJ2"]),
    ("FID", "Fidlerová", &["ČJL", "AJ", "SLR"]),
    ("PAL", "(doplnit)", &["ZSV", "OV", "DĚ", "SPP"]),
    ("ŠMD", "(doplnit)", &["CHE", "BIO", "PCCH", "SCHE"]),
    ("LEB", "(doplnit)", &["TV", "ZE"]),
    ("ŠVE", "(doplnit)", &["MAT", "CHE", "SMA"]),
    ("CHOU", "(doplnit)", &["INF", "MAT", "PRG"]),
    ("BER", "(doplnit)", &["AJ", "ŠJ2"]),
    ("VYS", "(doplnit)", &["FJ2", "AJ"]),
    ("LIH", "(doplnit)", &["VV"]),
    ("KOŠ", "(doplnit)", &["NJ2", "DĚ"]),
    ("VEJ", "(doplnit)", &["TV", "PL"]),
    ("PALB", "(doplnit)", &["HV", "ČJL"]),
    ("NEŠ", "(doplnit)", &["FY", "INF", "PCFY", "SEK"]),
    ("SKŘ", "(doplnit)", &["ZE", "BIO", "PCBI", "SZE"]),
    ("PET", "(doplnit)", &["TV", "OV"]),
    ("HRK", "(doplnit)", &["ČJL", "OV", "SZSV", "FJ2"]),
    ("JIN", "(doplnit)", &["AJ", "NJ2"]),
    ("MAN", "(doplnit)", &["MAT", "ZE"]),
    ("SMO", "Smolková", &["BIO", "ZE", "PCBI"]),
    ("FAL", "(doplnit)", &["ŠJ2", "AJ"]),
    ("SCHO", "(doplnit)", &["FY", "MAT", "SFY"]),
    ("PAV", "(doplnit)", &["DV", "ČJL"]),
    ("BRD", "(doplnit)", &["CHE", "MAT", "PCCH", "SPCH"]),
    ("SVI", "(doplnit)", &["TV", "PL"]),
    ("WIE", "(doplnit)", &["NJ2"]),
    ("ČEM", "(doplnit)", &["LAT", "ČJL", "FJ2"]),
    ("FOG", "(doplnit)", &["BIO", "CHE", "SBR"]),
    ("VAIS", "Vaisová", &["MAT", "AJ"]),
];

const PREFEROVANE: [(&str, &str); 10] = [
    ("SAU", "PŠJ"),
    ("FAL", "PŠJ"),
    ("VYS", "PFJ"),
    ("UHR", "PNJ"),
    ("BER", "PAJ"),
    ("LIH", "PVV"),
    ("PALB", "PHV"),
    ("CHOU", "PIF"),
    ("ČER", "PCH"),
    ("TES", "PFB"),
];

/// Přiřazuje učitele z aprobací tak, aby byly úvazky vyrovnané.
struct Prirazovac {
    zatez: Vec<(String, f32, Vec<&'static str>)>,
}

impl Prirazovac {
    fn vyber(&mut self, predmet: &str, hodin: f32, krome: &[&str]) -> String {
        let mut best: Option<usize> = None;
        for pruchod in 0..2 {
            for (i, (id, z, apr)) in self.zatez.iter().enumerate() {
                let umi = pruchod == 1 || apr.contains(&predmet);
                if umi && !krome.contains(&id.as_str()) && best.map_or(true, |b| *z < self.zatez[b].1) {
                    best = Some(i);
                }
            }
            if best.is_some() {
                break;
            }
        }
        let i = best.expect("aspoň jeden učitel");
        self.zatez[i].1 += hodin;
        self.zatez[i].0.clone()
    }
}

#[derive(Clone, Copy)]
enum K {
    C,
    S,
    P,
    Str(&'static str, &'static str),
    StrSk(&'static str, &'static str),
}

/// Učební plán podle efektivního ročníku (⚠ odhad z PDF – ověřit proti učebnímu plánu).
fn sablona(ef: u8) -> Vec<(&'static str, u8, K, bool)> {
    use K::*;
    match ef {
        1 => vec![
            ("ČJL", 4, C, false),
            ("MAT", 4, C, false),
            ("AJ", 4, S, false),
            ("DĚ", 2, C, false),
            ("ZE", 2, C, false),
            ("BIO", 2, C, false),
            ("FY", 1, C, false),
            ("OV", 1, C, false),
            ("INF", 1, S, false),
            ("TV", 2, P, false),
            ("PL", 1, P, false),
            ("HV", 1, C, false),
            ("DV/VV", 2, Str("DV", "VV"), true),
            ("PCBI/PCFY", 1, StrSk("PCBI", "PCFY"), false),
        ],
        2 => vec![
            ("ČJL", 4, C, false),
            ("MAT", 4, C, false),
            ("AJ", 3, S, false),
            ("DĚ", 2, C, false),
            ("ZE", 2, C, false),
            ("BIO", 2, C, false),
            ("FY", 2, C, false),
            ("OV", 1, C, false),
            ("TV", 2, P, false),
            ("HV", 1, C, false),
            ("DV/VV", 2, Str("DV", "VV"), true),
            ("PCBI/PCFY", 1, StrSk("PCBI", "PCFY"), false),
        ],
        3 => vec![
            ("ČJL", 4, C, false),
            ("MAT", 4, C, false),
            ("AJ", 3, S, false),
            ("DĚ", 2, C, false),
            ("ZE", 1, C, false),
            ("BIO", 2, C, false),
            ("FY", 2, C, false),
            ("CHE", 2, C, false),
            ("OV", 1, C, false),
            ("TV", 2, P, false),
            ("HV", 1, C, false),
            ("DV/VV", 2, Str("DV", "VV"), true),
            ("PCCH/PCFY", 1, StrSk("PCCH", "PCFY"), false),
        ],
        4 => vec![
            ("ČJL", 4, C, false),
            ("MAT", 4, C, false),
            ("AJ", 3, S, false),
            ("DĚ", 2, C, false),
            ("ZE", 2, C, false),
            ("BIO", 2, C, false),
            ("FY", 2, C, false),
            ("CHE", 2, C, false),
            ("OV", 1, C, false),
            ("TV", 2, P, false),
            ("HV", 1, C, false),
            ("VV", 1, C, false),
            ("INF", 1, S, false),
            ("LAT", 2, C, false),
            ("PCBI/PCCH", 1, StrSk("PCBI", "PCCH"), false),
        ],
        5 => vec![
            ("ČJL", 3, C, false),
            ("MAT", 4, C, false),
            ("AJ", 3, S, false),
            ("DĚ", 2, C, false),
            ("ZE", 2, C, false),
            ("ZSV", 1, C, false),
            ("FY", 2, C, false),
            ("CHE", 2, C, false),
            ("BIO", 2, C, false),
            ("INF", 2, S, false),
            ("TV", 2, P, false),
            ("PCBI/PCCH", 1, StrSk("PCBI", "PCCH"), false),
        ],
        6 => vec![
            ("ČJL", 3, C, false),
            ("MAT", 4, C, false),
            ("AJ", 3, S, false),
            ("DĚ", 2, C, false),
            ("ZE", 2, C, false),
            ("ZSV", 2, C, false),
            ("FY", 2, C, false),
            ("CHE", 2, C, false),
            ("BIO", 2, C, false),
            ("INF", 1, S, false),
            ("TV", 2, P, false),
            ("PCFY/PCCH", 1, StrSk("PCFY", "PCCH"), false),
        ],
        7 => vec![
            ("ČJL", 4, C, false),
            ("MAT", 4, C, false),
            ("AJ", 3, S, false),
            ("DĚ", 2, C, false),
            ("ZE", 2, C, false),
            ("ZSV", 2, C, false),
            ("FY", 2, C, false),
            ("CHE", 2, C, false),
            ("BIO", 2, C, false),
            ("TV", 2, P, false),
        ],
        _ => vec![
            ("ČJL", 4, C, false),
            ("MAT", 3, C, false),
            ("AJ", 4, S, false),
            ("DĚ", 2, C, false),
            ("ZSV", 2, C, false),
            ("TV", 2, P, false),
        ],
    }
}

fn pol(predmet: &str, ucitel: String) -> Pol {
    Pol { predmet: predmet.into(), ucitel }
}

fn volba(id: &str, nazev: &str, predmet: &str, ucitel: String) -> Volba {
    // kapacita podle jediné odborné pracovny (PVV, PHV = 28 míst)
    let kapacita = matches!(predmet, "VV" | "HV").then_some(28);
    Volba { id: id.into(), nazev: nazev.into(), predmet: predmet.into(), ucitel, kapacita }
}

const SEMINARE_7: [&str; 14] =
    ["SMA", "SFY", "SCHE", "SBR", "SČD", "SLR", "SPP", "SFAV", "SZSV", "SAJ", "PRG", "SEK", "SDG", "SPCH"];
const SEMINARE_8: [&str; 15] =
    ["SMA", "SFY", "SCHE", "SBCH", "SBR", "SČD", "SLR", "SPP", "SFAV", "SZSV", "SAJ", "SZE", "PRG", "SEK", "SDG"];

pub fn vychozi_skola_a_menu() -> (Skola, Vec<Menu>) {
    let tridy = vychozi_tridy();
    let mistnosti = vychozi_mistnosti(&tridy);
    let predmety = vychozi_predmety();
    let mut ucitele: Vec<Ucitel> = UCITELE
        .iter()
        .map(|(id, jmeno, _)| Ucitel {
            id: id.to_string(),
            jmeno: jmeno.to_string(),
            volno: vec![],
            max_den: 7,
            preferovana_mistnost: None,
        })
        .collect();
    for (u, m) in PREFEROVANE {
        if let Some(uc) = ucitele.iter_mut().find(|x| x.id == u) {
            uc.preferovana_mistnost = Some(m.into());
        }
    }
    let mut pr = Prirazovac { zatez: UCITELE.iter().map(|(id, _, apr)| (id.to_string(), 0.0, apr.to_vec())).collect() };
    let nazev_predmetu =
        |id: &str| predmety.iter().find(|p| p.id == id).map(|p| p.nazev.clone()).unwrap_or_else(|| id.into());

    // Semináře se známým vyučujícím (z rozvrhu) – přiřadit jako první.
    let mut menu = Vec::new();
    let sem_menu = |id: &str, nazev: &str, tridy: &[&str], seznam: &[&str], voleb: u8, pr: &mut Prirazovac| Menu {
        id: id.into(),
        nazev: nazev.into(),
        tridy: tridy.iter().map(|s| s.to_string()).collect(),
        volby: seznam.iter().map(|s| volba(s, &nazev_predmetu(s), s, pr.vyber(s, 2.0, &[]))).collect(),
        hodin_tydne: 2,
        dvojhodina: true,
        pocet_voleb: voleb,
    };
    menu.push(sem_menu(
        "sem-7",
        "Semináře septima (3 × 2 h)",
        &["septimaA", "septimaB", "3A"],
        &SEMINARE_7,
        3,
        &mut pr,
    ));
    menu.push(sem_menu("sem-8", "Semináře oktáva (6 × 2 h)", &["oktavaA", "oktavaB", "4A"], &SEMINARE_8, 6, &mut pr));

    // Volitelné předměty (zadání a–f)
    menu.insert(
        0,
        Menu {
            id: "sek-volba".into(),
            nazev: "Sekunda: informatika / čeština".into(),
            tridy: vec!["sekunda".into()],
            volby: vec![
                volba("INF+", "Informatika", "INF", pr.vyber("INF", 2.0, &[])),
                volba("ČJL+", "Čeština", "ČJL", pr.vyber("ČJL", 2.0, &[])),
            ],
            hodin_tydne: 2,
            dvojhodina: false,
            pocet_voleb: 1,
        },
    );
    let jazyky = |pr: &mut Prirazovac, h: f32| {
        vec![
            volba("NJ2", "Němčina", "NJ2", pr.vyber("NJ2", h, &[])),
            volba("ŠJ2", "Španělština", "ŠJ2", pr.vyber("ŠJ2", h, &[])),
            volba("FJ2", "Francouzština", "FJ2", pr.vyber("FJ2", h, &[])),
        ]
    };
    for r in 3..=8u8 {
        let tridy_r: Vec<String> =
            tridy.iter().filter(|t| t.typ == TypStudia::Osmilete && t.rocnik == r).map(|t| t.id.clone()).collect();
        menu.push(Menu {
            id: format!("3j-{r}"),
            nazev: format!("3. jazyk – {}. ročník (8leté)", r),
            tridy: tridy_r,
            volby: jazyky(&mut pr, 3.0),
            hodin_tydne: 3,
            dvojhodina: false,
            pocet_voleb: 1,
        });
    }
    for tr in tridy.iter().filter(|t| t.typ == TypStudia::Ctyrlete) {
        menu.push(Menu {
            id: format!("2j-{}", tr.id),
            nazev: format!("2. jazyk – {} (4leté, 4 h)", tr.nazev),
            tridy: vec![tr.id.clone()],
            volby: jazyky(&mut pr, 4.0),
            hodin_tydne: 4,
            dvojhodina: false,
            pocet_voleb: 1,
        });
    }
    menu.push(Menu {
        id: "vol-5".into(),
        nazev: "Kvinta: výtvarka / dramatka".into(),
        tridy: vec!["kvintaA".into(), "kvintaB".into(), "1A".into()],
        volby: vec![
            volba("VV2", "Výtvarná výchova", "VV", pr.vyber("VV", 2.0, &[])),
            volba("DV2", "Dramatická výchova", "DV", pr.vyber("DV", 2.0, &[])),
        ],
        hodin_tydne: 2,
        dvojhodina: true,
        pocet_voleb: 1,
    });
    menu.push(Menu {
        id: "vol-6".into(),
        nazev: "Sexta: výtvarka / dramatka / hudebka".into(),
        tridy: vec!["sextaA".into(), "sextaB".into(), "2A".into()],
        volby: vec![
            volba("VV2", "Výtvarná výchova", "VV", pr.vyber("VV", 2.0, &[])),
            volba("DV2", "Dramatická výchova", "DV", pr.vyber("DV", 2.0, &[])),
            volba("HV2", "Hudební výchova", "HV", pr.vyber("HV", 2.0, &[])),
        ],
        hodin_tydne: 2,
        dvojhodina: true,
        pocet_voleb: 1,
    });
    // pořadí menu podle (efektivního) ročníku
    menu.sort_by_key(|m| {
        let r = m
            .tridy
            .iter()
            .filter_map(|t| tridy.iter().find(|x| &x.id == t))
            .map(|t| (t.efektivni_rocnik(), t.typ == TypStudia::Ctyrlete))
            .min()
            .unwrap_or((9, false));
        (r, m.pocet_voleb, m.id.clone())
    });

    // Učební plán (až po volitelných – úvazky se vyrovnají i s jazyky a semináři)
    let mut plan = Vec::new();
    for tr in &tridy {
        for (predmet, hodin, k, dvoj) in sablona(tr.efektivni_rocnik()) {
            let h = hodin as f32;
            let struktura = match k {
                K::C => Struktura::CelouTridou(pr.vyber(predmet, h, &[])),
                K::S => {
                    let a = pr.vyber(predmet, h, &[]);
                    let b = pr.vyber(predmet, h, &[&a]);
                    Struktura::Rozdeleno { a, b }
                }
                K::P => {
                    let div = pr.vyber(predmet, h, &[]);
                    let chl = pr.vyber(predmet, h, &[&div]);
                    Struktura::Pohlavi { div, chl }
                }
                K::Str(l, s) => {
                    Struktura::Stridave { l: pol(l, pr.vyber(l, h / 2.0, &[])), s: pol(s, pr.vyber(s, h / 2.0, &[])) }
                }
                K::StrSk(p1, p2) => {
                    let t1 = pr.vyber(p1, h, &[]);
                    let t2 = pr.vyber(p2, h, &[&t1]);
                    Struktura::StridaveSkupiny {
                        a: [pol(p1, t1.clone()), pol(p2, t2.clone())],
                        b: [pol(p2, t2), pol(p1, t1)],
                    }
                }
            };
            plan.push(PlanovaHodina { trida: tr.id.clone(), predmet: predmet.into(), hodin, struktura, dvoj });
        }
    }

    let skola = Skola { tridy, mistnosti, ucitele, predmety, plan, nastaveni: Nastaveni::default() };
    (skola, menu)
}
