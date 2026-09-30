//! Import rozdělení žáků do skupin (semináře, jazykové skupiny AJa–AJd …) ve formátu,
//! který produkuje školní nástroj na kontrolu seminářů:
//!
//! | Ročník | Seminář | Mezitřídní | Třídy | Počet žáků | Jméno žáka | Třída žáka | (Blok) | (Učitel) |
//!
//! Skupina začíná řádkem s vyplněným kódem („SBI1“); další řádky mají vyplněné jen jméno a třídu žáka.
//! Stejný kód na více místech souboru se sloučí. Volitelný sloupec „Blok“ určí blok seminářů,
//! „Učitel“ zkratku vyučujícího. Soubor může být csv, txt, xlsx/xls/ods nebo dbf.

use crate::data::*;
use crate::rok;
use crate::studenti_io::{self, klic_jmena, najdi_tridu, norm};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ZakVeSkupine {
    pub radek: usize,
    pub jmeno: String,
    pub trida: String,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct SkupinaZeSouboru {
    /// Kód skupiny (id volby), např. „SBI1“.
    pub kod: String,
    pub rocnik: String,
    pub tridy: String,
    /// „Počet žáků“ uvedený v souboru (kontrola).
    pub pocet_uvedeny: Option<usize>,
    pub blok: Option<u8>,
    pub ucitel: Option<String>,
    pub zaci: Vec<ZakVeSkupine>,
}

#[derive(Clone, Debug, Default)]
pub struct NactenoSkupiny {
    pub skupiny: Vec<SkupinaZeSouboru>,
    pub popis_sloupcu: String,
    pub upozorneni: Vec<String>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Sl {
    Rocnik,
    Kod,
    Tridy,
    Pocet,
    Jmeno,
    TridaZaka,
    Blok,
    Ucitel,
}

fn sloupec(h: &str) -> Option<Sl> {
    Some(match norm(h).as_str() {
        "rocnik" | "rocniky" => Sl::Rocnik,
        "seminar" | "seminare" | "skupina" | "kod" | "kodskupiny" | "volba" | "predmet" | "kurz" => Sl::Kod,
        "tridy" | "tridyskupiny" => Sl::Tridy,
        "pocetzaku" | "pocet" | "zaku" => Sl::Pocet,
        "jmenozaka" | "jmeno" | "zak" | "student" | "prijmeniajmeno" | "jmenoaprijmeni" | "name" => Sl::Jmeno,
        "tridazaka" | "trida" | "class" => Sl::TridaZaka,
        "blok" | "bloky" | "blokseminare" => Sl::Blok,
        "ucitel" | "vyucujici" | "teacher" => Sl::Ucitel,
        _ => return None,
    })
}

/// Blok „1“, „2“ … nebo „A“, „B“ … → 1, 2 …
pub fn parsuj_blok(s: &str) -> Option<u8> {
    let t = s.trim().trim_start_matches(|c: char| !c.is_alphanumeric());
    let n = norm(t);
    let n = n.strip_prefix("blok").unwrap_or(&n);
    if let Ok(x) = n.parse::<u8>() {
        return (x > 0).then_some(x);
    }
    let mut z = n.chars();
    match (z.next(), z.next()) {
        (Some(c), None) if c.is_ascii_lowercase() => Some(c as u8 - b'a' + 1),
        _ => None,
    }
}

/// Zpracuje tabulku s rozdělením do skupin.
pub fn zpracuj(radky: Vec<Vec<String>>, posun_radku: usize) -> Result<NactenoSkupiny, String> {
    let radky: Vec<(usize, Vec<String>)> = radky
        .into_iter()
        .enumerate()
        .map(|(i, r)| (i + 1 + posun_radku, r.into_iter().map(|b| b.trim().to_string()).collect::<Vec<_>>()))
        .filter(|(_, r)| r.iter().any(|b| !b.is_empty()))
        .collect();
    let Some((_, hlavicka)) = radky.first() else { return Err("Soubor neobsahuje žádné řádky.".into()) };
    let mapa: Vec<Option<Sl>> = hlavicka.iter().map(|h| sloupec(h)).collect();
    let idx = |s: Sl| mapa.iter().position(|m| *m == Some(s));
    let (Some(i_kod), Some(i_jmeno)) = (idx(Sl::Kod), idx(Sl::Jmeno)) else {
        return Err(format!(
            "Hlavička ({}) musí obsahovat sloupec se skupinou („Seminář“) a se jménem žáka („Jméno žáka“).",
            hlavicka.join(" | ")
        ));
    };
    let popis = hlavicka
        .iter()
        .zip(&mapa)
        .map(|(h, m)| if m.is_some() { h.clone() } else { format!("{h} (ignorován)") })
        .collect::<Vec<_>>()
        .join(" | ");
    let bunka = |r: &Vec<String>, s: Sl| idx(s).and_then(|i| r.get(i)).cloned().unwrap_or_default();

    let mut upozorneni = Vec::new();
    let mut skupiny: Vec<SkupinaZeSouboru> = Vec::new();
    let mut aktualni: Option<usize> = None;
    for (cislo, r) in radky.iter().skip(1) {
        let kod = r.get(i_kod).cloned().unwrap_or_default();
        if !kod.is_empty() {
            let klic = norm(&kod);
            let i = match skupiny.iter().position(|g| norm(&g.kod) == klic) {
                Some(i) => i,
                None => {
                    skupiny.push(SkupinaZeSouboru { kod: kod.clone(), ..Default::default() });
                    skupiny.len() - 1
                }
            };
            let g = &mut skupiny[i];
            for (pole, sl) in [(&mut g.rocnik, Sl::Rocnik), (&mut g.tridy, Sl::Tridy)] {
                let v = bunka(r, sl);
                if !v.is_empty() {
                    *pole = v;
                }
            }
            let pocet = bunka(r, Sl::Pocet);
            if !pocet.is_empty() {
                match pocet.parse::<f64>() {
                    Ok(x) if x >= 0.0 => g.pocet_uvedeny = Some(x as usize),
                    _ => upozorneni.push(format!("Řádek {cislo}: „Počet žáků“ není číslo („{pocet}“).")),
                }
            }
            let blok = bunka(r, Sl::Blok);
            if !blok.is_empty() {
                match parsuj_blok(&blok) {
                    Some(b) => g.blok = Some(b),
                    None => {
                        upozorneni.push(format!("Řádek {cislo}: neznámý blok „{blok}“ (čekám 1, 2 … nebo A, B …)."))
                    }
                }
            }
            let uc = bunka(r, Sl::Ucitel);
            if !uc.is_empty() {
                g.ucitel = Some(uc);
            }
            aktualni = Some(i);
        }
        let jmeno = r.get(i_jmeno).map(|j| j.split_whitespace().collect::<Vec<_>>().join(" ")).unwrap_or_default();
        if jmeno.is_empty() {
            continue;
        }
        let Some(i) = aktualni else {
            upozorneni.push(format!("Řádek {cislo}: žák {jmeno} je před první skupinou – přeskočen."));
            continue;
        };
        skupiny[i].zaci.push(ZakVeSkupine { radek: *cislo, jmeno, trida: bunka(r, Sl::TridaZaka) });
    }
    if skupiny.is_empty() {
        return Err("V souboru nebyla nalezena žádná skupina (sloupec „Seminář“ je prázdný).".into());
    }
    for g in &skupiny {
        if let Some(p) = g.pocet_uvedeny {
            if p != g.zaci.len() {
                upozorneni.push(format!("Skupina {}: uvedeno {} žáků, v seznamu je {}.", g.kod, p, g.zaci.len()));
            }
        }
        let mut videni = BTreeSet::new();
        for z in &g.zaci {
            if !videni.insert((klic_jmena(&z.jmeno), norm(&z.trida))) {
                upozorneni.push(format!("Skupina {}: žák {} je uveden dvakrát (řádek {}).", g.kod, z.jmeno, z.radek));
            }
        }
    }
    Ok(NactenoSkupiny { skupiny, popis_sloupcu: format!("Hlavička: {popis}"), upozorneni })
}

pub fn nacti(cesta: &Path) -> Result<NactenoSkupiny, String> {
    let t = studenti_io::nacti_tabulku(cesta)?;
    let mut n = zpracuj(t.radky, t.posun_radku)?;
    n.upozorneni.splice(0..0, t.upozorneni);
    Ok(n)
}

// ───────────────────────────── přenesení do menu ─────────────────────────────

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Moznosti {
    /// Žáky ze souboru, kteří v roce nejsou, přidat (jinak se nahlásí).
    pub pridat_chybejici_zaky: bool,
    /// Z katalogu menu odebrat volby, které v souboru nejsou.
    pub odebrat_volby_mimo_soubor: bool,
}

#[derive(Clone, Debug, Default)]
pub struct Vysledek {
    pub volby_nove: Vec<String>,
    pub volby_aktualizovane: usize,
    pub volby_odebrane: Vec<String>,
    pub predmety_nove: Vec<String>,
    pub tridy_pridane: Vec<String>,
    pub zaci_prirazeni: usize,
    pub zaci_pridani: usize,
    /// Žáci tříd menu, kteří v souboru nejsou (nemají žádnou skupinu).
    pub zaci_bez_skupiny: usize,
    pub problemy: Vec<String>,
    pub varovani: Vec<String>,
}

impl Vysledek {
    pub fn shrnuti(&self) -> String {
        let mut s = format!(
            "Skupiny: nových voleb {}, aktualizovaných {}, přiřazeno žáků {}",
            self.volby_nove.len(),
            self.volby_aktualizovane,
            self.zaci_prirazeni
        );
        if self.zaci_pridani > 0 {
            s.push_str(&format!(" (z toho nově přidáno {})", self.zaci_pridani));
        }
        if !self.problemy.is_empty() {
            s.push_str(&format!(", problémů {}", self.problemy.len()));
        }
        s.push('.');
        s
    }
}

/// Základ kódu skupiny bez čísla: „SBI1“ → „SBI“, „AJ-c“ → „AJ-c“.
pub fn zaklad_kodu(kod: &str) -> String {
    kod.trim().trim_end_matches(|c: char| c.is_ascii_digit() || c == ' ' || c == '-' || c == '_').to_string()
}

/// Menu, jehož třídy nejlépe odpovídají třídám žáků ze souboru.
pub fn navrhni_menu(skola: &Skola, rok: &SkolniRok, n: &NactenoSkupiny) -> Option<String> {
    let tridy: BTreeSet<String> =
        n.skupiny.iter().flat_map(|g| g.zaci.iter()).filter_map(|z| najdi_tridu(skola, &z.trida)).collect();
    let kody: BTreeSet<String> = n.skupiny.iter().map(|g| norm(&g.kod)).collect();
    rok.menu
        .iter()
        .map(|m| {
            let spolecne_tridy = m.tridy.iter().filter(|t| tridy.contains(*t)).count() as i64;
            let spolecne_volby = m.volby.iter().filter(|v| kody.contains(&norm(&v.id))).count() as i64;
            let navic = m.tridy.iter().filter(|t| !tridy.contains(*t)).count() as i64;
            (spolecne_volby * 10 + spolecne_tridy * 5 - navic + if m.pocet_voleb > 1 { 1 } else { 0 }, m.id.clone())
        })
        .filter(|(skore, _)| *skore > 0)
        .max_by_key(|(skore, _)| *skore)
        .map(|(_, id)| id)
}

/// Přenese skupiny do menu `menu_id`: vytvoří/aktualizuje volby, přiřadí žákům volby
/// (volby tohoto menu u žáků jeho tříd se nahradí obsahem souboru).
pub fn aplikuj(skola: &mut Skola, rok: &mut SkolniRok, menu_id: &str, n: &NactenoSkupiny, m: Moznosti) -> Vysledek {
    let mut v = Vysledek::default();
    let Some(mi) = rok.menu.iter().position(|x| x.id == menu_id) else {
        v.problemy.push(format!("Menu {menu_id} neexistuje."));
        return v;
    };

    // 1) volby (katalog)
    let mut kod_na_volbu: BTreeMap<String, String> = BTreeMap::new();
    for g in &n.skupiny {
        let klic = norm(&g.kod);
        let menu = &mut rok.menu[mi];
        let existujici = menu
            .volby
            .iter()
            .position(|x| norm(&x.id) == klic)
            .or_else(|| menu.volby.iter().position(|x| norm(&x.nazev) == klic));
        let ucitel_ze_souboru = g.ucitel.as_ref().and_then(|u| {
            let nu = norm(u);
            let nalez = skola.ucitele.iter().find(|x| norm(&x.id) == nu || norm(&x.jmeno) == nu).map(|x| x.id.clone());
            if nalez.is_none() {
                v.varovani.push(format!("Skupina {}: neznámý učitel „{}“.", g.kod, u));
            }
            nalez
        });
        let id = match existujici {
            Some(i) => {
                let x = &mut menu.volby[i];
                if g.blok.is_some() {
                    x.blok = g.blok;
                }
                if let Some(u) = ucitel_ze_souboru {
                    x.ucitel = u;
                }
                v.volby_aktualizovane += 1;
                x.id.clone()
            }
            None => {
                // předmět a učitel: podle sesterské volby (SBI1 → SBI2), předmětu se stejným kódem, jinak nový předmět
                let zaklad = zaklad_kodu(&g.kod);
                let nz = norm(&zaklad);
                let sestra = menu.volby.iter().find(|x| norm(&zaklad_kodu(&x.id)) == nz).cloned();
                let predmet = match (&sestra, skola.predmety.iter().find(|p| norm(&p.id) == nz)) {
                    (Some(s), _) => s.predmet.clone(),
                    (None, Some(p)) => p.id.clone(),
                    (None, None) => {
                        skola.predmety.push(Predmet {
                            id: zaklad.clone(),
                            nazev: zaklad.clone(),
                            specialni_mistnosti: vec![],
                            kmenova_ok: true,
                        });
                        v.predmety_nove.push(zaklad.clone());
                        zaklad.clone()
                    }
                };
                let nazev = skola.predmety.iter().find(|p| p.id == predmet).map_or(g.kod.clone(), |p| {
                    if p.nazev == p.id {
                        g.kod.clone()
                    } else {
                        format!("{} ({})", p.nazev, g.kod)
                    }
                });
                let ucitel =
                    ucitel_ze_souboru.or_else(|| sestra.as_ref().map(|s| s.ucitel.clone())).unwrap_or_default();
                if ucitel.is_empty() {
                    v.varovani.push(format!("Skupina {}: nemá učitele – doplňte ho v katalogu voleb.", g.kod));
                }
                let menu = &mut rok.menu[mi];
                menu.volby.push(Volba {
                    id: g.kod.trim().to_string(),
                    nazev,
                    predmet,
                    ucitel,
                    blok: g.blok,
                    ..Default::default()
                });
                v.volby_nove.push(g.kod.trim().to_string());
                g.kod.trim().to_string()
            }
        };
        kod_na_volbu.insert(klic, id);
    }

    // 2) žáci
    let mut prirazeni: BTreeMap<u32, Vec<String>> = BTreeMap::new();
    let mut tridy_souboru: BTreeSet<String> = BTreeSet::new();
    for g in &n.skupiny {
        let volba = kod_na_volbu[&norm(&g.kod)].clone();
        for z in &g.zaci {
            let Some(trida) = najdi_tridu(skola, &z.trida) else {
                let proc = if z.trida.is_empty() {
                    "chybí třída".to_string()
                } else {
                    format!("neznámá třída „{}“", z.trida)
                };
                v.problemy.push(format!("Řádek {}: {} ({}) – {proc}.", z.radek, z.jmeno, g.kod));
                continue;
            };
            tridy_souboru.insert(trida.clone());
            let klic = klic_jmena(&z.jmeno);
            let id =
                match rok.studenti.iter().find(|s| s.v_rozvrhu() && s.trida == trida && klic_jmena(&s.jmeno) == klic) {
                    Some(s) => s.id,
                    None if m.pridat_chybejici_zaky => {
                        let id = rok.nove_id();
                        rok.studenti.push(Student {
                            id,
                            jmeno: z.jmeno.clone(),
                            trida: trida.clone(),
                            skupina_aj: SkupinaAj::A,
                            pohlavi: rok::odhad_pohlavi(&z.jmeno),
                            volby: BTreeMap::new(),
                            status: StavStudenta::Aktivni,
                        });
                        v.zaci_pridani += 1;
                        id
                    }
                    None => {
                        v.problemy.push(format!(
                            "Řádek {}: žák {} ({}) není ve třídě {} – přeskočen.",
                            z.radek,
                            z.jmeno,
                            g.kod,
                            skola.nazev_tridy(&trida)
                        ));
                        continue;
                    }
                };
            let seznam = prirazeni.entry(id).or_default();
            if !seznam.contains(&volba) {
                seznam.push(volba.clone());
            }
        }
    }

    // 3) třídy menu, volby mimo soubor
    let menu = &mut rok.menu[mi];
    for t in &tridy_souboru {
        if !menu.tridy.contains(t) {
            menu.tridy.push(t.clone());
            v.tridy_pridane.push(skola.nazev_tridy(t));
        }
    }
    if m.odebrat_volby_mimo_soubor {
        let ponechat: BTreeSet<String> = kod_na_volbu.values().cloned().collect();
        let pred: Vec<String> = menu.volby.iter().map(|x| x.id.clone()).collect();
        menu.volby.retain(|x| ponechat.contains(&x.id));
        v.volby_odebrane = pred.into_iter().filter(|id| !ponechat.contains(id)).collect();
    }
    let menu = menu.clone();

    // 4) volby žáků: soubor je úplný seznam pro dané menu
    let platne: BTreeSet<&str> = menu.volby.iter().map(|x| x.id.as_str()).collect();
    for s in rok.studenti.iter_mut().filter(|s| menu.tridy.contains(&s.trida)) {
        match prirazeni.get(&s.id) {
            Some(volby) => {
                let volby: Vec<String> = volby.iter().filter(|x| platne.contains(x.as_str())).cloned().collect();
                if volby.len() > menu.pocet_voleb as usize {
                    v.varovani.push(format!(
                        "{} ({}): {} skupin, menu počítá s {}.",
                        s.jmeno,
                        skola.nazev_tridy(&s.trida),
                        volby.len(),
                        menu.pocet_voleb
                    ));
                }
                s.volby.insert(menu.id.clone(), volby);
                v.zaci_prirazeni += 1;
            }
            None => {
                if s.v_rozvrhu() {
                    v.zaci_bez_skupiny += 1;
                }
                s.volby.remove(&menu.id);
            }
        }
    }
    if v.zaci_bez_skupiny > 0 {
        v.varovani.push(format!(
            "{} žáků tříd menu v souboru není – jejich volby v menu „{}“ byly vymazány.",
            v.zaci_bez_skupiny, menu.nazev
        ));
    }
    v
}
