//! Import a export seznamu žáků.
//!
//! Import: `.txt`, `.csv`/`.tsv`, `.xlsx`/`.xls`/`.ods` (Excel, první list s daty), `.dbf` (dBase/FoxPro)
//! a `.json`. Sloupce se poznají podle hlavičky (Jméno, Příjmení, Třída, AJ/Skupina, Pohlaví, Rodné číslo,
//! Stav, Volby),
//! bez hlavičky podle obsahu buněk (třída, AJa/AJb, D/CH, stav, zbytek = jméno).
//! Export: `.csv` (UTF-8 s BOM, středníky – rovnou do Excelu), `.json`, `.xlsx`, `.txt`.
//! Soubor z exportu lze znovu naimportovat (včetně voleb).

use crate::data::*;
use crate::rok;
use calamine::{open_workbook_auto, Data, Reader};
use rust_xlsxwriter::{Format as XlsxFormat, Workbook};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::path::Path;

// ───────────────────────────── formáty ─────────────────────────────

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format {
    Csv,
    Json,
    Xlsx,
    Txt,
    /// Jen import (dBase / FoxPro, export z MS SQL).
    Dbf,
}

impl Format {
    pub const VSECHNY: [Format; 4] = [Format::Csv, Format::Json, Format::Xlsx, Format::Txt];

    pub fn pripona(self) -> &'static str {
        match self {
            Format::Csv => "csv",
            Format::Json => "json",
            Format::Xlsx => "xlsx",
            Format::Txt => "txt",
            Format::Dbf => "dbf",
        }
    }
    pub fn nazev(self) -> &'static str {
        match self {
            Format::Csv => "CSV",
            Format::Json => "JSON",
            Format::Xlsx => "Excel (.xlsx)",
            Format::Txt => "TXT",
            Format::Dbf => "DBF",
        }
    }
}

/// Přípony, které umí import.
pub const PRIPONY_IMPORTU: [&str; 9] = ["txt", "csv", "tsv", "xlsx", "xls", "ods", "xlsm", "dbf", "json"];

pub fn je_soubor_importu(cesta: &Path) -> bool {
    cesta.extension().and_then(|e| e.to_str()).is_some_and(|e| PRIPONY_IMPORTU.contains(&e.to_lowercase().as_str()))
}

// ───────────────────────────── záznam z importu ─────────────────────────────

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Zaznam {
    /// Číslo řádku v souboru (pro hlášení chyb; u JSON pořadí záznamu).
    pub radek: usize,
    pub jmeno: String,
    /// Třída tak, jak je v souboru (id nebo název) – převede se až při plánování importu.
    pub trida: Option<String>,
    pub skupina: Option<SkupinaAj>,
    pub pohlavi: Option<Pohlavi>,
    pub stav: Option<StavStudenta>,
    /// menu → volby (id nebo názvy) – ověřují se při aplikaci.
    pub volby: BTreeMap<String, Vec<String>>,
}

#[derive(Clone, Debug)]
pub struct Nacteno {
    pub format: Format,
    pub zaznamy: Vec<Zaznam>,
    /// Stručný popis, jak byly sloupce rozpoznány.
    pub popis_sloupcu: String,
    /// Upozornění při čtení (kódování, nerozpoznané hodnoty, přeskočené řádky).
    pub upozorneni: Vec<String>,
}

// ───────────────────────────── pomocné funkce ─────────────────────────────

/// Bez diakritiky, malá písmena, jen písmena a číslice: „1.A“ → „1a“, „Příjmení“ → „prijmeni“.
pub fn norm(s: &str) -> String {
    s.chars()
        .flat_map(|c| c.to_lowercase())
        .filter_map(|c| match c {
            'á' | 'ä' => Some('a'),
            'č' => Some('c'),
            'ď' => Some('d'),
            'é' | 'ě' => Some('e'),
            'í' => Some('i'),
            'ň' => Some('n'),
            'ó' | 'ö' => Some('o'),
            'ř' => Some('r'),
            'š' => Some('s'),
            'ť' => Some('t'),
            'ú' | 'ů' | 'ü' => Some('u'),
            'ý' => Some('y'),
            'ž' => Some('z'),
            c if c.is_alphanumeric() => Some(c),
            _ => None,
        })
        .collect()
}

fn cisti_jmeno(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ").trim_matches('"').trim().to_string()
}

/// Najde třídu podle id, názvu nebo aliasu („kvarta A“, „kvartaA“, „7.A“), případně podle kmenové
/// učebny. Když zápisu odpovídá víc tříd (nejednoznačný alias), vrátí `None`.
pub fn najdi_tridu(skola: &Skola, text: &str) -> Option<String> {
    let n = norm(text);
    if n.is_empty() {
        return None;
    }
    let shody: Vec<&Trida> = skola
        .tridy
        .iter()
        .filter(|t| norm(&t.id) == n || norm(&t.nazev) == n || t.aliasy().any(|a| norm(a) == n))
        .collect();
    match shody.len() {
        1 => return Some(shody[0].id.clone()),
        0 => {}
        _ => return None,
    }
    let podle_ucebny: Vec<&Trida> = skola.tridy.iter().filter(|t| norm(&t.kmenova) == n).collect();
    (podle_ucebny.len() == 1).then(|| podle_ucebny[0].id.clone())
}

/// Třídy, kterým zápis odpovídá víc než jednou (pro srozumitelné hlášení nejednoznačnosti).
pub fn nejednoznacna_trida(skola: &Skola, text: &str) -> Vec<String> {
    let n = norm(text);
    let v: Vec<String> = skola
        .tridy
        .iter()
        .filter(|t| norm(&t.id) == n || norm(&t.nazev) == n || t.aliasy().any(|a| norm(a) == n))
        .map(|t| t.nazev.clone())
        .collect();
    if v.len() > 1 {
        v
    } else {
        vec![]
    }
}

/// Klíč jména nezávislý na pořadí a diakritice: „Brokl Marek“ = „Marek Brokl“ = „marek BROKL“.
pub fn klic_jmena(jmeno: &str) -> String {
    let mut slova: Vec<String> = jmeno.split_whitespace().map(norm).filter(|s| !s.is_empty()).collect();
    slova.sort();
    slova.join(" ")
}

/// Pohlaví z rodného čísla (tak ho určují Bakaláři): u žen je k měsíci přičteno 50 (od roku 2004
/// případně 70). Rodné číslo se nikam neukládá.
pub fn pohlavi_z_rc(rc: &str) -> Option<Pohlavi> {
    let cislice: Vec<u32> = rc.chars().filter_map(|c| c.to_digit(10)).collect();
    if cislice.len() < 9 {
        return None;
    }
    let mesic = cislice[2] * 10 + cislice[3];
    match mesic {
        1..=12 | 21..=32 => Some(Pohlavi::CH),
        51..=62 | 71..=82 => Some(Pohlavi::D),
        _ => None,
    }
}

pub fn parsuj_pohlavi(s: &str) -> Option<Pohlavi> {
    match norm(s).as_str() {
        "d" | "z" | "zena" | "divka" | "f" | "female" | "girl" | "dz" => Some(Pohlavi::D),
        "ch" | "m" | "muz" | "chlapec" | "male" | "boy" | "chl" => Some(Pohlavi::CH),
        _ => None,
    }
}

pub fn parsuj_skupinu(s: &str) -> Option<SkupinaAj> {
    match norm(s).as_str() {
        "aja" | "a" | "skupinaa" | "1" => Some(SkupinaAj::A),
        "ajb" | "b" | "skupinab" | "2" => Some(SkupinaAj::B),
        _ => None,
    }
}

pub fn parsuj_stav(s: &str) -> Option<StavStudenta> {
    match norm(s).as_str() {
        "aktivni" | "ano" | "studuje" => Some(StavStudenta::Aktivni),
        "propada" | "propadl" | "propadla" | "propadnuti" | "opakuje" => Some(StavStudenta::Propada),
        "odesel" | "odesla" | "odchazi" | "odchod" | "ukoncil" | "ukoncila" => Some(StavStudenta::Odesel),
        _ => None,
    }
}

/// „menu=V1,V2|menu2=V3“
fn parsuj_volby(s: &str) -> BTreeMap<String, Vec<String>> {
    let mut out = BTreeMap::new();
    for cast in s.split('|') {
        if let Some((m, v)) = cast.split_once('=') {
            let v: Vec<String> = v.split(',').map(|x| x.trim().to_string()).filter(|x| !x.is_empty()).collect();
            if !m.trim().is_empty() && !v.is_empty() {
                out.insert(m.trim().to_string(), v);
            }
        }
    }
    out
}

fn text_voleb(s: &Student) -> String {
    s.volby.iter().map(|(m, v)| format!("{}={}", m, v.join(","))).collect::<Vec<_>>().join("|")
}

// ───────────────────────────── čtení textu a CSV ─────────────────────────────

/// Dekóduje bajty souboru: UTF-8 (s BOM i bez), UTF-16 s BOM, jinak Windows-1250 (český Excel).
pub fn dekoduj(bajty: &[u8]) -> (String, Option<&'static str>) {
    if let Some(r) = bajty.strip_prefix(&[0xEF, 0xBB, 0xBF]) {
        return (String::from_utf8_lossy(r).into_owned(), None);
    }
    if bajty.starts_with(&[0xFF, 0xFE]) {
        return (encoding_rs::UTF_16LE.decode_without_bom_handling(&bajty[2..]).0.into_owned(), Some("UTF-16"));
    }
    if bajty.starts_with(&[0xFE, 0xFF]) {
        return (encoding_rs::UTF_16BE.decode_without_bom_handling(&bajty[2..]).0.into_owned(), Some("UTF-16"));
    }
    match std::str::from_utf8(bajty) {
        Ok(s) => (s.to_string(), None),
        Err(_) => (encoding_rs::WINDOWS_1250.decode_without_bom_handling(bajty).0.into_owned(), Some("Windows-1250")),
    }
}

/// Odhad oddělovače: ten, který je na většině neprázdných řádků a má na nich stejný počet výskytů.
fn odhadni_oddelovac(text: &str) -> Option<char> {
    let radky: Vec<&str> = text.lines().filter(|l| !l.trim().is_empty()).take(50).collect();
    if radky.is_empty() {
        return None;
    }
    let mut nejlepsi: Option<(char, usize)> = None;
    for d in ['\t', ';', ','] {
        let pocty: Vec<usize> = radky.iter().map(|l| l.matches(d).count()).collect();
        let s_delim = pocty.iter().filter(|&&p| p > 0).count();
        if s_delim * 10 < radky.len() * 8 {
            continue; // méně než 80 % řádků
        }
        let nejcastejsi = *pocty.iter().max().unwrap();
        // tabulka = skoro všude stejný počet oddělovačů (hlavička bez volby apod. toleruje ±)
        let shodne = pocty.iter().filter(|&&p| p == nejcastejsi || p + 1 >= nejcastejsi).count();
        if shodne * 10 < radky.len() * 8 {
            continue;
        }
        if nejlepsi.is_none_or(|(_, n)| s_delim > n) {
            nejlepsi = Some((d, s_delim));
        }
    }
    nejlepsi.map(|(d, _)| d)
}

/// Parser CSV s uvozovkami (`""` = uvozovka, víceřádkové buňky).
pub fn parsuj_csv(text: &str, oddelovac: char) -> Vec<Vec<String>> {
    let mut radky = Vec::new();
    let mut radek: Vec<String> = Vec::new();
    let mut bunka = String::new();
    let mut v_uvozovkach = false;
    let mut znaky = text.chars().peekable();
    while let Some(c) = znaky.next() {
        if v_uvozovkach {
            if c == '"' {
                if znaky.peek() == Some(&'"') {
                    bunka.push('"');
                    znaky.next();
                } else {
                    v_uvozovkach = false;
                }
            } else {
                bunka.push(c);
            }
        } else if c == '"' && bunka.trim().is_empty() {
            bunka.clear();
            v_uvozovkach = true;
        } else if c == oddelovac {
            radek.push(std::mem::take(&mut bunka));
        } else if c == '\n' || c == '\r' {
            if c == '\r' && znaky.peek() == Some(&'\n') {
                znaky.next();
            }
            radek.push(std::mem::take(&mut bunka));
            radky.push(std::mem::take(&mut radek));
        } else {
            bunka.push(c);
        }
    }
    if !bunka.is_empty() || !radek.is_empty() {
        radek.push(bunka);
        radky.push(radek);
    }
    radky
}

// ───────────────────────────── rozpoznání sloupců ─────────────────────────────

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Sloupec {
    Jmeno,
    Prijmeni,
    CeleJmeno,
    Trida,
    Skupina,
    Pohlavi,
    Stav,
    Volby,
    RodneCislo,
}

fn sloupec_z_hlavicky(h: &str) -> Option<Sloupec> {
    Some(match norm(h).as_str() {
        "jmeno" | "krestni" | "krestnijmeno" | "firstname" | "givenname" => Sloupec::Jmeno,
        "prijmeni" | "surname" | "lastname" | "familyname" => Sloupec::Prijmeni,
        "jmenoaprijmeni" | "prijmenijmeno" | "prijmeniajmeno" | "zak" | "zaci" | "zakyne" | "student" | "studenti"
        | "name" | "fullname" | "celejmeno" | "jmenozaka" | "zakjmeno" | "prijmeniajmenozaka" => Sloupec::CeleJmeno,
        "trida" | "class" | "tr" | "ztrida" | "tridaid" | "tridazaka" | "kmenovatrida" => Sloupec::Trida,
        "skupina" | "aj" | "skupinaaj" | "group" | "skupinaanglictiny" => Sloupec::Skupina,
        "pohlavi" | "pohl" | "gender" | "sex" => Sloupec::Pohlavi,
        "stav" | "status" | "stavzaka" => Sloupec::Stav,
        "volby" | "volitelne" | "volitelnepredmety" | "seminare" => Sloupec::Volby,
        "rodnecislo" | "rc" | "rodnec" | "rodcislo" | "birthnumber" => Sloupec::RodneCislo,
        _ => return None,
    })
}

/// Zpracuje tabulku (řádky buněk) – s hlavičkou i bez ní.
pub fn zpracuj_tabulku(skola: &Skola, format: Format, radky: Vec<Vec<String>>) -> Result<Nacteno, String> {
    let mut upozorneni = Vec::new();
    // čísla řádků podle původní pozice (1-based); prázdné řádky se přeskočí
    let radky: Vec<(usize, Vec<String>)> = radky
        .into_iter()
        .enumerate()
        .map(|(i, r)| (i + 1, r.into_iter().map(|b| b.trim().to_string()).collect::<Vec<_>>()))
        .filter(|(_, r)| r.iter().any(|b| !b.is_empty()))
        .collect();
    if radky.is_empty() {
        return Err("Soubor neobsahuje žádné řádky.".into());
    }
    let mapovani: Vec<Option<Sloupec>> = radky[0].1.iter().map(|h| sloupec_z_hlavicky(h)).collect();
    let ma_hlavicku = mapovani.iter().any(|m| m.is_some());
    let mut zaznamy = Vec::new();
    let popis;

    if ma_hlavicku {
        let ma = |s: Sloupec| mapovani.contains(&Some(s));
        if !(ma(Sloupec::Jmeno) || ma(Sloupec::CeleJmeno) || ma(Sloupec::Prijmeni)) {
            return Err(format!(
                "V hlavičce ({}) chybí sloupec se jménem – očekávám „Jméno“ (případně „Příjmení“).",
                radky[0].1.join(" | ")
            ));
        }
        let pouzite: Vec<String> = radky[0]
            .1
            .iter()
            .zip(&mapovani)
            .map(|(h, m)| match m {
                Some(_) => h.clone(),
                None => format!("{h} (ignorován)"),
            })
            .collect();
        popis = format!("Hlavička: {}", pouzite.join(" | "));
        for (cislo, bunky) in radky.iter().skip(1) {
            let mut z = Zaznam { radek: *cislo, ..Default::default() };
            let (mut krestni, mut prijmeni, mut cele) = (String::new(), String::new(), String::new());
            let mut pohlavi_rc: Option<Pohlavi> = None;
            for (i, b) in bunky.iter().enumerate() {
                let Some(Some(s)) = mapovani.get(i) else { continue };
                if b.is_empty() {
                    continue;
                }
                match s {
                    Sloupec::Jmeno => krestni = b.clone(),
                    Sloupec::Prijmeni => prijmeni = b.clone(),
                    Sloupec::CeleJmeno => cele = b.clone(),
                    Sloupec::Trida => z.trida = Some(b.clone()),
                    Sloupec::Skupina => match parsuj_skupinu(b) {
                        Some(g) => z.skupina = Some(g),
                        None => upozorneni.push(format!("Řádek {cislo}: neznámá skupina „{b}“ (čekám AJa / AJb).")),
                    },
                    Sloupec::Pohlavi => match parsuj_pohlavi(b) {
                        Some(p) => z.pohlavi = Some(p),
                        None => upozorneni.push(format!("Řádek {cislo}: neznámé pohlaví „{b}“ (čekám D / CH).")),
                    },
                    Sloupec::Stav => match parsuj_stav(b) {
                        Some(st) => z.stav = Some(st),
                        None => {
                            upozorneni.push(format!("Řádek {cislo}: neznámý stav „{b}“ (aktivní / propadá / odešel)."))
                        }
                    },
                    Sloupec::Volby => z.volby = parsuj_volby(b),
                    Sloupec::RodneCislo => match pohlavi_z_rc(b) {
                        Some(p) => pohlavi_rc = Some(p),
                        None => upozorneni.push(format!("Řádek {cislo}: z rodného čísla nelze určit pohlaví.")),
                    },
                }
            }
            // sloupec Pohlaví má přednost před rodným číslem
            if z.pohlavi.is_none() {
                z.pohlavi = pohlavi_rc;
            }
            // „Jméno“ + „Příjmení“ ve dvou sloupcích → „Jméno Příjmení“ (jako v aplikaci)
            z.jmeno = if !cele.is_empty() { cele } else { format!("{} {}", krestni, prijmeni) };
            z.jmeno = cisti_jmeno(&z.jmeno);
            if z.jmeno.is_empty() {
                upozorneni.push(format!("Řádek {cislo}: prázdné jméno – přeskočen."));
                continue;
            }
            zaznamy.push(z);
        }
    } else {
        popis = "Bez hlavičky – sloupce rozpoznány podle obsahu (třída, AJa/AJb, D/CH, stav, volby; zbytek = jméno)."
            .into();
        for (cislo, bunky) in &radky {
            let mut z = Zaznam { radek: *cislo, ..Default::default() };
            let mut jmena: Vec<&str> = Vec::new();
            for b in bunky.iter().filter(|b| !b.is_empty()) {
                if z.trida.is_none() && najdi_tridu(skola, b).is_some() {
                    z.trida = Some(b.clone());
                } else if z.skupina.is_none() && parsuj_skupinu(b).is_some() {
                    z.skupina = parsuj_skupinu(b);
                } else if z.pohlavi.is_none() && parsuj_pohlavi(b).is_some() {
                    z.pohlavi = parsuj_pohlavi(b);
                } else if z.stav.is_none() && parsuj_stav(b).is_some() {
                    z.stav = parsuj_stav(b);
                } else if z.volby.is_empty() && b.contains('=') && !parsuj_volby(b).is_empty() {
                    z.volby = parsuj_volby(b);
                } else {
                    jmena.push(b);
                }
            }
            z.jmeno = cisti_jmeno(&jmena.join(" "));
            if z.jmeno.is_empty() {
                upozorneni.push(format!("Řádek {cislo}: nenalezeno jméno – přeskočen."));
                continue;
            }
            zaznamy.push(z);
        }
    }
    if zaznamy.is_empty() {
        return Err("V souboru nebyl nalezen žádný žák.".into());
    }
    Ok(Nacteno { format, zaznamy, popis_sloupcu: popis, upozorneni })
}

// ───────────────────────────── načtení souboru ─────────────────────────────

/// Tabulka načtená z libovolného podporovaného souboru (bez interpretace sloupců).
pub struct Tabulka {
    pub format: Format,
    pub radky: Vec<Vec<String>>,
    /// Číslo řádku v souboru, které odpovídá prvnímu řádku `radky`, mínus 1 (Excel: začátek oblasti).
    pub posun_radku: usize,
    pub upozorneni: Vec<String>,
}

/// Rozdělí text na řádky a buňky (odhad oddělovače; bez oddělovače = jeden sloupec).
pub fn text_na_tabulku(format: Format, bajty: &[u8], koncovka_tsv: bool) -> Tabulka {
    let (text, kodovani) = dekoduj(bajty);
    let oddelovac = if koncovka_tsv { Some('\t') } else { odhadni_oddelovac(&text) };
    let radky: Vec<Vec<String>> = match oddelovac {
        Some(d) => parsuj_csv(&text, d),
        // jeden sloupec (seznam jmen): jeden řádek = jedna buňka, bez zpracování uvozovek
        None => text.lines().map(|l| vec![l.trim().trim_matches('"').to_string()]).collect(),
    };
    let upozorneni = kodovani.map(|k| format!("Kódování souboru není UTF-8, použito {k}.")).into_iter().collect();
    Tabulka { format, radky, posun_radku: 0, upozorneni }
}

pub fn nacti_text(skola: &Skola, format: Format, bajty: &[u8], koncovka_tsv: bool) -> Result<Nacteno, String> {
    let t = text_na_tabulku(format, bajty, koncovka_tsv);
    let mut n = zpracuj_tabulku(skola, format, t.radky)?;
    n.upozorneni.splice(0..0, t.upozorneni);
    Ok(n)
}

fn bunka_na_text(b: &Data) -> String {
    match b {
        Data::Empty => String::new(),
        Data::Float(f) if f.fract() == 0.0 && f.abs() < 1e15 => format!("{}", *f as i64),
        other => other.to_string(),
    }
}

/// První list sešitu, který obsahuje data.
pub fn excel_na_tabulku(cesta: &Path) -> Result<Tabulka, String> {
    let mut wb = open_workbook_auto(cesta).map_err(|e| format!("Nelze otevřít Excel {}: {e}", cesta.display()))?;
    let listy = wb.sheet_names();
    let mut upozorneni = Vec::new();
    for list in &listy {
        let range = match wb.worksheet_range(list) {
            Ok(r) => r,
            Err(e) => {
                upozorneni.push(format!("List „{list}“ nelze číst: {e}"));
                continue;
            }
        };
        let radky: Vec<Vec<String>> =
            range.rows().map(|r| r.iter().map(bunka_na_text).collect::<Vec<String>>()).collect();
        if !radky.iter().any(|r| r.iter().any(|b| !b.trim().is_empty())) {
            continue;
        }
        if listy.len() > 1 {
            upozorneni.push(format!("Sešit má {} listů – načten list „{}“.", listy.len(), list));
        }
        // čísla řádků v Excelu začínají na prvním neprázdném řádku oblasti
        let posun_radku = range.start().map_or(0, |(r, _)| r as usize);
        return Ok(Tabulka { format: Format::Xlsx, radky, posun_radku, upozorneni });
    }
    Err("Excel neobsahuje žádný list s daty.".into())
}

// ───────────────────────────── DBF (dBase / FoxPro) ─────────────────────────────

/// CP852 (DOS Latin 2) – horní polovina tabulky (vygenerováno z kodeku cp852).
const CP852: &str = "ÇüéâäůćçłëŐőîŹÄĆÉĹĺôöĽľŚśÖÜŤťŁ×čáíóúĄąŽžĘę¬źČş«»░▒▓│┤ÁÂĚŞ╣║╗╝Żż┐└┴┬├─┼Ăă╚╔╩╦╠═╬¤đĐĎËďŇÍÎě┘┌█▄ŢŮ▀ÓßÔŃńňŠšŔÚŕŰýÝţ´\u{AD}˝˛ˇ˘§÷¸°¨˙űŘř■\u{A0}";

fn dekoduj_cp852(b: &[u8]) -> String {
    let horni: Vec<char> = CP852.chars().collect();
    b.iter().map(|&x| if x < 0x80 { x as char } else { horni[(x - 0x80) as usize] }).collect()
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum KodovaniDbf {
    Utf8,
    Cp852,
    Cp1250,
    Cp1252,
}

impl KodovaniDbf {
    fn nazev(self) -> &'static str {
        match self {
            KodovaniDbf::Utf8 => "UTF-8",
            KodovaniDbf::Cp852 => "CP852 (DOS Latin 2)",
            KodovaniDbf::Cp1250 => "Windows-1250",
            KodovaniDbf::Cp1252 => "Windows-1252",
        }
    }
    fn dekoduj(self, b: &[u8]) -> String {
        match self {
            KodovaniDbf::Utf8 => String::from_utf8_lossy(b).into_owned(),
            KodovaniDbf::Cp852 => dekoduj_cp852(b),
            KodovaniDbf::Cp1250 => encoding_rs::WINDOWS_1250.decode_without_bom_handling(b).0.into_owned(),
            KodovaniDbf::Cp1252 => encoding_rs::WINDOWS_1252.decode_without_bom_handling(b).0.into_owned(),
        }
    }
}

/// Skóre „češtiny“: kolik znaků je běžná česká písmena s diakritikou (pro odhad kódování).
fn skore_cestiny(text: &str) -> i64 {
    const CESKE: &str = "áčďéěíňóřšťúůýžÁČĎÉĚÍŇÓŘŠŤÚŮÝŽ";
    text.chars()
        .map(|c| {
            if CESKE.contains(c) {
                2
            } else if c.is_alphanumeric() || c.is_whitespace() || ".,-'/()".contains(c) {
                0
            } else {
                -3
            }
        })
        .sum()
}

/// Načte tabulku z DBF (dBase III / FoxPro): hlavička = názvy polí, smazané záznamy se přeskočí.
/// Kódování podle značky jazyka v hlavičce, jinak odhad (UTF-8 / CP852 / Windows-1250).
pub fn dbf_na_tabulku(bajty: &[u8]) -> Result<Tabulka, String> {
    let chyba = || "Poškozený nebo nepodporovaný soubor DBF.".to_string();
    if bajty.len() < 33 {
        return Err(chyba());
    }
    let pocet = u32::from_le_bytes([bajty[4], bajty[5], bajty[6], bajty[7]]) as usize;
    let delka_hlavicky = u16::from_le_bytes([bajty[8], bajty[9]]) as usize;
    let delka_zaznamu = u16::from_le_bytes([bajty[10], bajty[11]]) as usize;
    let jazyk = bajty[29];
    if delka_hlavicky < 33 || delka_hlavicky > bajty.len() || delka_zaznamu < 2 {
        return Err(chyba());
    }
    // popisy polí po 32 bajtech od offsetu 32, ukončené 0x0D
    let mut pole: Vec<(Vec<u8>, u8, usize)> = Vec::new();
    let mut o = 32;
    while o + 32 <= delka_hlavicky && bajty[o] != 0x0D {
        let nazev: Vec<u8> = bajty[o..o + 11].iter().copied().take_while(|&b| b != 0).collect();
        pole.push((nazev, bajty[o + 11], bajty[o + 16] as usize));
        o += 32;
    }
    if pole.is_empty() || pole.iter().map(|p| p.2).sum::<usize>() + 1 > delka_zaznamu {
        return Err(chyba());
    }
    // syrové textové hodnoty
    let mut syrove: Vec<Vec<Vec<u8>>> = Vec::new();
    for i in 0..pocet {
        let zacatek = delka_hlavicky + i * delka_zaznamu;
        let Some(z) = bajty.get(zacatek..zacatek + delka_zaznamu) else { break };
        if z[0] == b'*' {
            continue; // smazaný záznam
        }
        let mut p = 1;
        let mut radek = Vec::new();
        for (_, typ, delka) in &pole {
            let hodnota = &z[p..p + delka];
            p += delka;
            radek.push(match typ {
                b'C' | b'N' | b'F' | b'D' | b'L' | b'V' => hodnota.to_vec(),
                _ => Vec::new(), // memo/binární pole se nečtou
            });
        }
        syrove.push(radek);
    }
    let mut kodovani = match jazyk {
        0x64 | 0x87 => Some(KodovaniDbf::Cp852),
        0xC8 => Some(KodovaniDbf::Cp1250),
        0x03 | 0x57 => Some(KodovaniDbf::Cp1252),
        _ => None,
    };
    let mut upozorneni = Vec::new();
    if kodovani.is_none() {
        let vse: Vec<u8> = syrove.iter().flatten().flatten().copied().collect();
        kodovani = Some(if std::str::from_utf8(&vse).is_ok() {
            KodovaniDbf::Utf8
        } else {
            [KodovaniDbf::Cp852, KodovaniDbf::Cp1250]
                .into_iter()
                .max_by_key(|k| skore_cestiny(&k.dekoduj(&vse)))
                .unwrap()
        });
        let k = kodovani.unwrap();
        if k != KodovaniDbf::Utf8 {
            upozorneni.push(format!("DBF bez značky kódování – odhadnuto {}.", k.nazev()));
        }
    }
    let k = kodovani.unwrap();
    let mut radky: Vec<Vec<String>> = vec![pole.iter().map(|(n, _, _)| k.dekoduj(n).trim().to_string()).collect()];
    for r in syrove {
        radky.push(r.iter().map(|b| k.dekoduj(b).trim().to_string()).collect());
    }
    Ok(Tabulka { format: Format::Dbf, radky, posun_radku: 0, upozorneni })
}

/// Načte tabulku ze souboru podle přípony (txt, csv, tsv, xlsx, xls, ods, dbf).
pub fn nacti_tabulku(cesta: &Path) -> Result<Tabulka, String> {
    let pripona = cesta.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
    let cti = || std::fs::read(cesta).map_err(|e| format!("Nelze číst {}: {e}", cesta.display()));
    match pripona.as_str() {
        "xlsx" | "xls" | "ods" | "xlsm" | "xlsb" => excel_na_tabulku(cesta),
        "dbf" => dbf_na_tabulku(&cti()?),
        "csv" => Ok(text_na_tabulku(Format::Csv, &cti()?, false)),
        "tsv" => Ok(text_na_tabulku(Format::Csv, &cti()?, true)),
        "txt" => Ok(text_na_tabulku(Format::Txt, &cti()?, false)),
        "" => Err(format!("Soubor {} nemá příponu – podporuji txt, csv, xlsx, xls, ods, dbf.", cesta.display())),
        p => Err(format!("Nepodporovaný formát „.{p}“ – podporuji txt, csv, tsv, xlsx, xls, ods, dbf.")),
    }
}

fn text_z_hodnoty(v: &Value) -> Option<String> {
    match v {
        Value::String(s) => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        _ => None,
    }
}

fn nacti_json(bajty: &[u8]) -> Result<Nacteno, String> {
    let (text, _) = dekoduj(bajty);
    let v: Value = serde_json::from_str(&text).map_err(|e| format!("Chybný JSON: {e}"))?;
    let pole = match &v {
        Value::Array(a) => a.clone(),
        Value::Object(o) => o
            .get("studenti")
            .or_else(|| o.get("zaci"))
            .and_then(|x| x.as_array())
            .cloned()
            .ok_or("JSON musí být pole žáků nebo objekt s polem „studenti“.")?,
        _ => return Err("JSON musí být pole žáků nebo objekt s polem „studenti“.".into()),
    };
    let mut upozorneni = Vec::new();
    let mut zaznamy = Vec::new();
    for (i, x) in pole.iter().enumerate() {
        let radek = i + 1;
        let mut z = Zaznam { radek, ..Default::default() };
        match x {
            Value::String(s) => z.jmeno = cisti_jmeno(s),
            Value::Object(o) => {
                let pole_text = |klice: &[&str]| klice.iter().find_map(|k| o.get(*k).and_then(text_z_hodnoty));
                z.jmeno = cisti_jmeno(&pole_text(&["jmeno", "name", "zak"]).unwrap_or_default());
                z.trida = pole_text(&["trida", "class"]).filter(|s| !s.is_empty());
                if let Some(s) = pole_text(&["skupina_aj", "skupina", "aj"]) {
                    z.skupina = parsuj_skupinu(&s);
                    if z.skupina.is_none() && !s.is_empty() {
                        upozorneni.push(format!("Záznam {radek}: neznámá skupina „{s}“."));
                    }
                }
                if let Some(s) = pole_text(&["pohlavi", "gender"]) {
                    z.pohlavi = parsuj_pohlavi(&s);
                    if z.pohlavi.is_none() && !s.is_empty() {
                        upozorneni.push(format!("Záznam {radek}: neznámé pohlaví „{s}“."));
                    }
                }
                if let Some(s) = pole_text(&["stav", "status"]) {
                    z.stav = parsuj_stav(&s);
                    if z.stav.is_none() && !s.is_empty() {
                        upozorneni.push(format!("Záznam {radek}: neznámý stav „{s}“."));
                    }
                }
                match o.get("volby") {
                    Some(Value::Object(m)) => {
                        for (menu, vol) in m {
                            let v: Vec<String> = match vol {
                                Value::Array(a) => a.iter().filter_map(text_z_hodnoty).collect(),
                                Value::String(s) => vec![s.clone()],
                                _ => vec![],
                            };
                            if !v.is_empty() {
                                z.volby.insert(menu.clone(), v);
                            }
                        }
                    }
                    Some(Value::String(s)) => z.volby = parsuj_volby(s),
                    _ => {}
                }
            }
            _ => {
                upozorneni.push(format!("Záznam {radek}: není text ani objekt – přeskočen."));
                continue;
            }
        }
        if z.jmeno.is_empty() {
            upozorneni.push(format!("Záznam {radek}: prázdné jméno – přeskočen."));
            continue;
        }
        zaznamy.push(z);
    }
    if zaznamy.is_empty() {
        return Err("V souboru nebyl nalezen žádný žák.".into());
    }
    Ok(Nacteno { format: Format::Json, zaznamy, popis_sloupcu: "JSON (pole žáků)".into(), upozorneni })
}

/// Načte soubor podle přípony.
pub fn nacti(skola: &Skola, cesta: &Path) -> Result<Nacteno, String> {
    let pripona = cesta.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
    if pripona == "json" {
        let bajty = std::fs::read(cesta).map_err(|e| format!("Nelze číst {}: {e}", cesta.display()))?;
        return nacti_json(&bajty);
    }
    let t = nacti_tabulku(cesta).map_err(|e| e.replace("dbf.", "dbf, json."))?;
    let mut n = zpracuj_tabulku(skola, t.format, t.radky)?;
    for z in &mut n.zaznamy {
        z.radek += t.posun_radku;
    }
    n.upozorneni.splice(0..0, t.upozorneni);
    Ok(n)
}

// ───────────────────────────── plán a aplikace importu ─────────────────────────────

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rezim {
    /// Žák se stejným jménem ve stejné třídě se aktualizuje, ostatní se přidají.
    PridatAktualizovat,
    /// Všichni žáci tříd, které jsou v souboru, se nahradí žáky ze souboru.
    NahraditTridy,
    /// Všichni žáci školy se nahradí žáky ze souboru.
    NahraditVse,
}

impl Rezim {
    pub const VSECHNY: [Rezim; 3] = [Rezim::PridatAktualizovat, Rezim::NahraditTridy, Rezim::NahraditVse];

    pub fn nazev(self) -> &'static str {
        match self {
            Rezim::PridatAktualizovat => "Přidat nové a aktualizovat stávající",
            Rezim::NahraditTridy => "Nahradit žáky tříd uvedených v souboru",
            Rezim::NahraditVse => "Nahradit všechny žáky",
        }
    }
    pub fn popis(self) -> &'static str {
        match self {
            Rezim::PridatAktualizovat => {
                "Stejné jméno ve stejné třídě = aktualizace (skupina, pohlaví, stav, volby podle souboru); ostatní se přidají."
            }
            Rezim::NahraditTridy => "Stávající žáci těchto tříd se smažou (včetně jejich voleb) a nahradí se souborem.",
            Rezim::NahraditVse => "Smažou se všichni stávající žáci (včetně voleb) – rok bude přesně podle souboru.",
        }
    }
}

#[derive(Clone, Debug)]
pub struct Polozka {
    pub zaznam: Zaznam,
    /// id třídy
    pub trida: String,
}

#[derive(Clone, Debug, Default)]
pub struct Plan {
    pub polozky: Vec<Polozka>,
    /// Řádky, které nelze zařadit (neznámá / chybějící třída).
    pub problemy: Vec<String>,
}

impl Plan {
    /// Počet žáků podle tříd (v pořadí číselníku).
    pub fn podle_trid(&self, skola: &Skola) -> Vec<(String, usize)> {
        let mut m: BTreeMap<&str, usize> = BTreeMap::new();
        for p in &self.polozky {
            *m.entry(p.trida.as_str()).or_default() += 1;
        }
        skola.tridy.iter().filter_map(|t| m.get(t.id.as_str()).map(|n| (t.id.clone(), *n))).collect()
    }
}

/// Přiřadí záznamům třídy. `vychozi_trida` (id nebo prázdné) se použije u řádků bez třídy.
pub fn naplanuj(skola: &Skola, zaznamy: &[Zaznam], vychozi_trida: &str) -> Plan {
    let mut plan = Plan::default();
    for z in zaznamy {
        let trida = match &z.trida {
            Some(t) => match najdi_tridu(skola, t) {
                Some(id) => id,
                None => {
                    let vic = nejednoznacna_trida(skola, t);
                    if vic.is_empty() {
                        plan.problemy.push(format!(
                            "Řádek {}: {} – neznámá třída „{}“ (doplňte alias v Číselníky ➡ Třídy), přeskočen.",
                            z.radek, z.jmeno, t
                        ));
                    } else {
                        plan.problemy.push(format!(
                            "Řádek {}: {} – třída „{}“ je nejednoznačná ({}), přeskočen.",
                            z.radek,
                            z.jmeno,
                            t,
                            vic.join(", ")
                        ));
                    }
                    continue;
                }
            },
            None if skola.trida(vychozi_trida).is_some() => vychozi_trida.to_string(),
            None => {
                plan.problemy
                    .push(format!("Řádek {}: {} – bez třídy (zvolte výchozí třídu), přeskočen.", z.radek, z.jmeno));
                continue;
            }
        };
        plan.polozky.push(Polozka { zaznam: z.clone(), trida });
    }
    plan
}

#[derive(Clone, Debug, Default)]
pub struct ImportVysledek {
    pub pridano: usize,
    pub aktualizovano: usize,
    pub odstraneno: usize,
    pub preskoceno: usize,
    pub varovani: Vec<String>,
}

impl ImportVysledek {
    pub fn shrnuti(&self) -> String {
        let mut s = format!("Import: přidáno {}, aktualizováno {}", self.pridano, self.aktualizovano);
        if self.odstraneno > 0 {
            s.push_str(&format!(", odstraněno {}", self.odstraneno));
        }
        if self.preskoceno > 0 {
            s.push_str(&format!(", přeskočeno {}", self.preskoceno));
        }
        s.push('.');
        s
    }
}

/// Ověří volby proti katalogům menu (id nebo název menu/volby, menu se musí týkat třídy žáka).
fn over_volby(
    rok: &SkolniRok,
    trida: &str,
    surove: &BTreeMap<String, Vec<String>>,
    zaznam: &Zaznam,
    varovani: &mut Vec<String>,
) -> BTreeMap<String, Vec<String>> {
    let mut out = BTreeMap::new();
    for (menu_text, volby_text) in surove {
        let n = norm(menu_text);
        let Some(m) = rok.menu.iter().find(|m| norm(&m.id) == n || norm(&m.nazev) == n) else {
            varovani.push(format!("Řádek {}: {} – neznámé menu voleb „{}“.", zaznam.radek, zaznam.jmeno, menu_text));
            continue;
        };
        if !m.tridy.iter().any(|t| t == trida) {
            varovani.push(format!(
                "Řádek {}: {} – menu „{}“ se netýká jeho třídy, přeskočeno.",
                zaznam.radek, zaznam.jmeno, m.nazev
            ));
            continue;
        }
        let mut vybrane: Vec<String> = Vec::new();
        for v in volby_text {
            let nv = norm(v);
            match m.volby.iter().find(|x| norm(&x.id) == nv || norm(&x.nazev) == nv) {
                Some(x) if !vybrane.contains(&x.id) => vybrane.push(x.id.clone()),
                Some(_) => {}
                None => varovani
                    .push(format!("Řádek {}: {} – menu „{}“ nemá volbu „{}“.", zaznam.radek, zaznam.jmeno, m.nazev, v)),
            }
        }
        if vybrane.len() > m.pocet_voleb as usize {
            varovani.push(format!(
                "Řádek {}: {} – menu „{}“ povoluje {} voleb, v souboru je {}.",
                zaznam.radek,
                zaznam.jmeno,
                m.nazev,
                m.pocet_voleb,
                vybrane.len()
            ));
        }
        if !vybrane.is_empty() {
            out.insert(m.id.clone(), vybrane);
        }
    }
    out
}

/// Provede import do školního roku.
pub fn aplikuj(skola: &Skola, rok: &mut SkolniRok, plan: &Plan, rezim: Rezim) -> ImportVysledek {
    let _ = skola;
    let mut v = ImportVysledek { preskoceno: plan.problemy.len(), ..Default::default() };
    match rezim {
        Rezim::NahraditVse => {
            v.odstraneno = rok.studenti.len();
            rok.studenti.clear();
        }
        Rezim::NahraditTridy => {
            let tridy: BTreeSet<&str> = plan.polozky.iter().map(|p| p.trida.as_str()).collect();
            let pred = rok.studenti.len();
            rok.studenti.retain(|s| !tridy.contains(s.trida.as_str()));
            v.odstraneno = pred - rok.studenti.len();
        }
        Rezim::PridatAktualizovat => {}
    }
    let mut videno: HashSet<(String, String)> = HashSet::new();
    let mut nove_bez_skupiny: Vec<u32> = Vec::new();
    for p in &plan.polozky {
        let z = &p.zaznam;
        let klic = (klic_jmena(&z.jmeno), p.trida.clone());
        if !videno.insert(klic.clone()) {
            v.preskoceno += 1;
            v.varovani.push(format!("Řádek {}: duplicitní žák {} ve třídě {} – přeskočen.", z.radek, z.jmeno, p.trida));
            continue;
        }
        let volby = over_volby(rok, &p.trida, &z.volby, z, &mut v.varovani);
        if let Some(i) = rok.studenti.iter().position(|s| s.trida == p.trida && klic_jmena(&s.jmeno) == klic.0) {
            let s = &mut rok.studenti[i];
            if let Some(g) = z.skupina {
                s.skupina_aj = g;
            }
            if let Some(ph) = z.pohlavi {
                s.pohlavi = ph;
            }
            if let Some(st) = z.stav {
                s.status = st;
            }
            s.volby.extend(volby);
            v.aktualizovano += 1;
            continue;
        }
        let id = rok.nove_id();
        if z.skupina.is_none() {
            nove_bez_skupiny.push(id);
        }
        rok.studenti.push(Student {
            id,
            jmeno: z.jmeno.clone(),
            trida: p.trida.clone(),
            skupina_aj: z.skupina.unwrap_or(SkupinaAj::A),
            pohlavi: z.pohlavi.unwrap_or_else(|| rok::odhad_pohlavi(&z.jmeno)),
            volby,
            status: z.stav.unwrap_or(StavStudenta::Aktivni),
        });
        v.pridano += 1;
    }
    // Skupiny AJ pro žáky, které soubor nezařadil: celá nová třída → 15:15 podle pohlaví,
    // jinak se doplňují do menší skupiny.
    let tridy: BTreeSet<String> =
        nove_bez_skupiny.iter().filter_map(|id| rok.student(*id).map(|s| s.trida.clone())).collect();
    for t in tridy {
        let vsichni_nezarazeni =
            rok.studenti.iter().filter(|s| s.trida == t && s.v_rozvrhu()).all(|s| nove_bez_skupiny.contains(&s.id));
        if vsichni_nezarazeni {
            rok::rozdel_tridu(rok, &t);
            continue;
        }
        for id in nove_bez_skupiny.iter().copied() {
            let Some(idx) = rok.studenti.iter().position(|s| s.id == id && s.trida == t) else { continue };
            let a = rok
                .studenti
                .iter()
                .filter(|s| s.trida == t && s.v_rozvrhu() && s.id != id && s.skupina_aj == SkupinaAj::A)
                .count();
            let b = rok
                .studenti
                .iter()
                .filter(|s| s.trida == t && s.v_rozvrhu() && s.id != id && s.skupina_aj == SkupinaAj::B)
                .count();
            rok.studenti[idx].skupina_aj = if a <= b { SkupinaAj::A } else { SkupinaAj::B };
        }
    }
    v
}

// ───────────────────────────── export ─────────────────────────────

pub const HLAVICKA: [&str; 6] = ["Jméno", "Třída", "AJ", "Pohlaví", "Stav", "Volby"];

#[derive(Clone, Copy, Debug, Default)]
pub struct Moznosti {
    /// TXT: jen jména, jeden žák na řádek.
    pub txt_jen_jmena: bool,
}

fn radky_exportu(skola: &Skola, studenti: &[&Student]) -> Vec<[String; 6]> {
    studenti
        .iter()
        .map(|s| {
            [
                s.jmeno.clone(),
                skola.nazev_tridy(&s.trida),
                s.skupina_aj.nazev().to_string(),
                s.pohlavi.zkratka().to_string(),
                s.status.nazev().to_string(),
                text_voleb(s),
            ]
        })
        .collect()
}

fn csv_pole(s: &str) -> String {
    if s.contains([';', '"', '\n', '\r']) || s != s.trim() {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

pub fn csv_text(skola: &Skola, studenti: &[&Student]) -> String {
    let mut s = String::from("\u{FEFF}");
    s.push_str(&HLAVICKA.join(";"));
    s.push_str("\r\n");
    for r in radky_exportu(skola, studenti) {
        s.push_str(&r.iter().map(|x| csv_pole(x)).collect::<Vec<_>>().join(";"));
        s.push_str("\r\n");
    }
    s
}

pub fn json_text(skola: &Skola, rok: &SkolniRok, studenti: &[&Student]) -> String {
    let zaci: Vec<Value> = studenti
        .iter()
        .map(|s| {
            json!({
                "jmeno": s.jmeno,
                "trida": s.trida,
                "trida_nazev": skola.nazev_tridy(&s.trida),
                "skupina_aj": s.skupina_aj.nazev(),
                "pohlavi": s.pohlavi.zkratka(),
                "stav": match s.status {
                    StavStudenta::Aktivni => "aktivni",
                    StavStudenta::Propada => "propada",
                    StavStudenta::Odesel => "odesel",
                },
                "volby": s.volby,
            })
        })
        .collect();
    serde_json::to_string_pretty(&json!({ "rok": rok.label, "studenti": zaci })).unwrap_or_default()
}

pub fn txt_text(skola: &Skola, studenti: &[&Student], m: Moznosti) -> String {
    let mut s = String::new();
    for r in radky_exportu(skola, studenti) {
        if m.txt_jen_jmena {
            s.push_str(r[0].trim());
        } else {
            // jméno; třída; AJ; pohlaví (+ stav, je-li jiný než aktivní; + volby, jsou-li)
            let mut casti = vec![r[0].clone(), r[1].clone(), r[2].clone(), r[3].clone()];
            if r[4] != StavStudenta::Aktivni.nazev() || !r[5].is_empty() {
                casti.push(r[4].clone());
            }
            if !r[5].is_empty() {
                casti.push(r[5].clone());
            }
            s.push_str(&casti.iter().map(|c| csv_pole(c)).collect::<Vec<_>>().join("; "));
        }
        s.push('\n');
    }
    s
}

fn zapis_xlsx(skola: &Skola, studenti: &[&Student], cesta: &Path) -> Result<(), String> {
    let chyba = |e: rust_xlsxwriter::XlsxError| format!("Excel: {e}");
    let mut wb = Workbook::new();
    let ws = wb.add_worksheet();
    ws.set_name("Studenti").map_err(chyba)?;
    let tucne = XlsxFormat::new().set_bold();
    for (c, h) in HLAVICKA.iter().enumerate() {
        ws.write_string_with_format(0, c as u16, *h, &tucne).map_err(chyba)?;
    }
    let radky = radky_exportu(skola, studenti);
    for (i, r) in radky.iter().enumerate() {
        for (c, b) in r.iter().enumerate() {
            ws.write_string((i + 1) as u32, c as u16, b).map_err(chyba)?;
        }
    }
    for (c, w) in [32.0, 14.0, 7.0, 10.0, 10.0, 40.0].iter().enumerate() {
        ws.set_column_width(c as u16, *w).map_err(chyba)?;
    }
    ws.set_freeze_panes(1, 0).map_err(chyba)?;
    ws.autofilter(0, 0, radky.len() as u32, (HLAVICKA.len() - 1) as u16).map_err(chyba)?;
    wb.save(cesta).map_err(chyba)
}

/// Uloží seznam žáků ve zvoleném formátu.
pub fn exportuj(
    skola: &Skola,
    rok: &SkolniRok,
    studenti: &[&Student],
    format: Format,
    cesta: &Path,
    m: Moznosti,
) -> Result<(), String> {
    let zapis =
        |obsah: String| std::fs::write(cesta, obsah).map_err(|e| format!("Nelze zapsat {}: {e}", cesta.display()));
    match format {
        Format::Csv => zapis(csv_text(skola, studenti)),
        Format::Json => zapis(json_text(skola, rok, studenti)),
        Format::Txt => zapis(txt_text(skola, studenti, m)),
        Format::Xlsx => zapis_xlsx(skola, studenti, cesta),
        Format::Dbf => Err("Export do DBF není podporován – použijte CSV nebo Excel.".into()),
    }
}
