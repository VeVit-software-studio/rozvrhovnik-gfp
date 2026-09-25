//! Operace se školním rokem: rozdělení AJa/AJb 15:15, přechod do nového roku
//! (postup / propadnutí / odchod / maturita), import voleb a katalogů z minulého roku,
//! doplnění chybějících voleb a rozdělení přeplněné skupiny.

use crate::data::*;
use rand::{rngs::StdRng, seq::SliceRandom, SeedableRng};
use std::collections::BTreeMap;

/// Co se se žákem stane na konci roku (krok 2 průvodce).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Zmena {
    Postupuje,
    Propada,
    Odchazi,
}

impl Zmena {
    pub fn nazev(self) -> &'static str {
        match self {
            Zmena::Postupuje => "postupuje",
            Zmena::Propada => "propadá",
            Zmena::Odchazi => "odchází",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct NovyZak {
    pub jmeno: String,
    pub pohlavi: Pohlavi,
    pub skupina: SkupinaAj,
}

/// Odhad pohlaví z českého jména: příjmení na -ová / -á ➡ dívka, jinak křestní jméno na -a ➡ dívka.
pub fn odhad_pohlavi(jmeno: &str) -> Pohlavi {
    let slova: Vec<String> = jmeno.split_whitespace().map(|s| s.to_lowercase()).collect();
    if slova.iter().any(|s| s.ends_with("ová") || s.ends_with("ská") || s.ends_with("cká")) {
        return Pohlavi::D;
    }
    if let Some(prijmeni) = slova.last() {
        if slova.len() >= 2 && prijmeni.ends_with('á') {
            return Pohlavi::D;
        }
    }
    let krestni = slova.first().cloned().unwrap_or_default();
    // Ženská jména končí obvykle na -a/-e/-ie; výjimky jsou mužská jména na -a.
    let muzska_na_a = ["jirka", "honza", "míša", "saša", "nikola", "luka", "ilja", "kuba"];
    if (krestni.ends_with('a') && !muzska_na_a.contains(&krestni.as_str())) || krestni.ends_with("ie") {
        Pohlavi::D
    } else {
        Pohlavi::CH
    }
}

/// Jeden žák na řádek. Volitelně s pohlavím za středníkem: „Jana Nováková; D“.
pub fn parsuj_jmena(text: &str) -> Vec<(String, Pohlavi)> {
    text.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(|l| {
            let (jmeno, poh) = match l.rsplit_once(';') {
                Some((j, p)) => {
                    let p = p.trim().to_uppercase();
                    let poh = match p.as_str() {
                        "D" | "Ž" | "F" => Some(Pohlavi::D),
                        "CH" | "C" | "M" | "H" => Some(Pohlavi::CH),
                        _ => None,
                    };
                    (j.trim().to_string(), poh)
                }
                None => (l.to_string(), None),
            };
            let poh = poh.unwrap_or_else(|| odhad_pohlavi(&jmeno));
            (jmeno, poh)
        })
        .collect()
}

/// Rozdělení na AJa/AJb 15:15 s vyrovnaným poměrem pohlaví.
/// Vrací skupinu pro každý vstup (ve stejném pořadí).
/// Dívky a chlapci se střídají A/B; chlapci začínají tam, kde dívky skončily (offset),
/// takže velikosti skupin se liší nejvýše o 1 a poměr pohlaví je vyrovnaný.
pub fn rozdel_1515(zaci: &[(String, Pohlavi)]) -> Vec<SkupinaAj> {
    let mut poradi: Vec<usize> = (0..zaci.len()).collect();
    poradi.sort_by(|&a, &b| zaci[a].1.cmp(&zaci[b].1).then_with(|| zaci[a].0.cmp(&zaci[b].0)));
    let mut vysledek = vec![SkupinaAj::A; zaci.len()];
    let mut citac = 0usize;
    for i in poradi {
        vysledek[i] = if citac % 2 == 0 { SkupinaAj::A } else { SkupinaAj::B };
        citac += 1;
    }
    vysledek
}

/// Rozdělí existující žáky třídy (aktivní) na AJa/AJb 15:15.
pub fn rozdel_tridu(rok: &mut SkolniRok, trida: &str) {
    let idx: Vec<usize> =
        rok.studenti.iter().enumerate().filter(|(_, s)| s.trida == trida && s.v_rozvrhu()).map(|(i, _)| i).collect();
    let vstup: Vec<(String, Pohlavi)> =
        idx.iter().map(|&i| (rok.studenti[i].jmeno.clone(), rok.studenti[i].pohlavi)).collect();
    let skup = rozdel_1515(&vstup);
    for (k, &i) in idx.iter().enumerate() {
        rok.studenti[i].skupina_aj = skup[k];
    }
}

/// Načte jména a rozdělí je 15:15 (krok 1 průvodce).
pub fn novi_zaci_z_textu(text: &str) -> Vec<NovyZak> {
    let jmena = parsuj_jmena(text);
    let skup = rozdel_1515(&jmena);
    jmena.into_iter().zip(skup).map(|((jmeno, pohlavi), skupina)| NovyZak { jmeno, pohlavi, skupina }).collect()
}

/// Kam žák přejde v novém roce (None = odchází / maturita).
pub fn cilova_trida(skola: &Skola, s: &Student, zmena: Zmena) -> Option<String> {
    match zmena {
        Zmena::Odchazi => None,
        Zmena::Propada => Some(s.trida.clone()),
        Zmena::Postupuje => skola.trida(&s.trida).and_then(|t| t.naslednik.clone()),
    }
}

pub fn vychozi_zmena(s: &Student) -> Zmena {
    match s.status {
        StavStudenta::Aktivni => Zmena::Postupuje,
        StavStudenta::Propada => Zmena::Propada,
        StavStudenta::Odesel => Zmena::Odchazi,
    }
}

/// Vytvoří nový školní rok: převede žáky, přidá nové, zkopíruje katalogy voleb
/// a automaticky přenese pokračující volby (jazyk, VV/DV…).
pub fn novy_rok(
    skola: &Skola,
    stary: &SkolniRok,
    zmeny: &BTreeMap<u32, Zmena>,
    novi: &BTreeMap<String, Vec<NovyZak>>,
    label: &str,
) -> SkolniRok {
    let mut rok = SkolniRok {
        label: label.to_string(),
        studenti: Vec::new(),
        menu: stary.menu.clone(),
        dalsi_id: stary.dalsi_id,
    };
    for s in &stary.studenti {
        let zmena = zmeny.get(&s.id).copied().unwrap_or_else(|| vychozi_zmena(s));
        if s.status == StavStudenta::Odesel {
            continue;
        }
        if let Some(cil) = cilova_trida(skola, s, zmena) {
            let mut n = s.clone();
            n.trida = cil;
            n.status = StavStudenta::Aktivni;
            n.volby.clear();
            rok.studenti.push(n);
        }
    }
    for (trida, zaci) in novi {
        for z in zaci {
            let id = rok.nove_id();
            rok.studenti.push(Student {
                id,
                jmeno: z.jmeno.clone(),
                trida: trida.clone(),
                skupina_aj: z.skupina,
                pohlavi: z.pohlavi,
                volby: BTreeMap::new(),
                status: StavStudenta::Aktivni,
            });
        }
    }
    import_voleb(&mut rok, stary, None);
    rok
}

/// Přenese volby z minulého roku: pro každé menu, do kterého žák patří, převezme volby,
/// jejichž id existuje v novém menu a žák je měl vloni v libovolném menu.
/// `jen_menu` = omezit na jedno menu. Vrací počet doplněných voleb.
pub fn import_voleb(rok: &mut SkolniRok, stary: &SkolniRok, jen_menu: Option<&str>) -> usize {
    let mut pocet = 0;
    let menu = rok.menu.clone();
    for s in rok.studenti.iter_mut() {
        let Some(minuly) = stary.student(s.id) else { continue };
        for m in menu.iter().filter(|m| m.tridy.contains(&s.trida)) {
            if jen_menu.is_some_and(|j| j != m.id) {
                continue;
            }
            let aktualni = s.volby.entry(m.id.clone()).or_default();
            // semináře (více voleb) se nepřenášejí – žák volí nové
            if m.pocet_voleb != 1 && !minuly.volby.contains_key(&m.id) {
                continue;
            }
            for vol in minuly.volby.values().flatten() {
                if aktualni.len() >= m.pocet_voleb as usize {
                    break;
                }
                if m.volby.iter().any(|v| &v.id == vol) && !aktualni.contains(vol) {
                    aktualni.push(vol.clone());
                    pocet += 1;
                }
            }
        }
        s.volby.retain(|_, v| !v.is_empty());
    }
    pocet
}

/// Zkopíruje katalog voleb (menu) z minulého roku. Vrací false, pokud tam menu nebylo.
pub fn import_katalogu(rok: &mut SkolniRok, stary: &SkolniRok, menu_id: &str) -> bool {
    let Some(m) = stary.menu.iter().find(|m| m.id == menu_id) else { return false };
    match rok.menu.iter_mut().find(|x| x.id == menu_id) {
        Some(x) => *x = m.clone(),
        None => rok.menu.push(m.clone()),
    }
    true
}

/// Počet žáků v jednotlivých volbách menu.
pub fn obsazenost(rok: &SkolniRok, menu: &Menu) -> BTreeMap<String, usize> {
    let mut m: BTreeMap<String, usize> = menu.volby.iter().map(|v| (v.id.clone(), 0)).collect();
    for s in rok.studenti.iter().filter(|s| s.v_rozvrhu() && menu.tridy.contains(&s.trida)) {
        for v in s.volby.get(&menu.id).into_iter().flatten() {
            if let Some(c) = m.get_mut(v) {
                *c += 1;
            }
        }
    }
    m
}

/// Doplní chybějící volby: volby se rozdělí do `pocet_voleb` bloků (round-robin) a žák
/// dostane z každého bloku nejméně obsazenou volbu (respektuje kapacitu). Bloková
/// struktura drží semináře rozvrhnutelné (žák nemá dvě volby ze stejného bloku).
pub fn doplnit_volby(rok: &mut SkolniRok, menu_id: &str, seed: u64) -> usize {
    let Some(menu) = rok.menu.iter().find(|m| m.id == menu_id).cloned() else { return 0 };
    if menu.volby.is_empty() {
        return 0;
    }
    let k = (menu.pocet_voleb.max(1) as usize).min(menu.volby.len());
    let bloky: Vec<Vec<usize>> = (0..k).map(|b| (0..menu.volby.len()).filter(|i| i % k == b).collect()).collect();
    let mut obs = obsazenost(rok, &menu);
    let mut rng = StdRng::seed_from_u64(seed);
    let mut poradi: Vec<usize> = rok
        .studenti
        .iter()
        .enumerate()
        .filter(|(_, s)| s.v_rozvrhu() && menu.tridy.contains(&s.trida))
        .map(|(i, _)| i)
        .collect();
    poradi.shuffle(&mut rng);
    let mut pocet = 0;
    for i in poradi {
        let s = &mut rok.studenti[i];
        let aktualni = s.volby.entry(menu.id.clone()).or_default();
        aktualni.retain(|v| menu.volby.iter().any(|x| &x.id == v));
        for blok in &bloky {
            if aktualni.len() >= k {
                break;
            }
            if blok.iter().any(|&vi| aktualni.contains(&menu.volby[vi].id)) {
                continue;
            }
            let volna = blok.iter().copied().filter(|&vi| {
                let v = &menu.volby[vi];
                v.kapacita.map_or(true, |kap| obs[&v.id] < kap as usize)
            });
            let mut kandidati: Vec<usize> = volna.collect();
            kandidati.shuffle(&mut rng);
            if let Some(&vi) = kandidati.iter().min_by_key(|&&vi| obs[&menu.volby[vi].id]) {
                let id = menu.volby[vi].id.clone();
                *obs.get_mut(&id).unwrap() += 1;
                aktualni.push(id);
                pocet += 1;
            }
        }
    }
    for s in rok.studenti.iter_mut() {
        s.volby.retain(|_, v| !v.is_empty());
    }
    pocet
}

/// ✂ Rozdělí volbu na dvě skupiny: druhá polovina žáků (abecedně) přejde do nové
/// volby se stejným předmětem a zadaným učitelem. Vrací id nové volby.
pub fn rozdel_skupinu(rok: &mut SkolniRok, menu_id: &str, volba_id: &str, ucitel: &str) -> Option<String> {
    let mi = rok.menu.iter().position(|m| m.id == menu_id)?;
    let puvodni = rok.menu[mi].volby.iter().find(|v| v.id == volba_id)?.clone();
    let mut n = 2;
    let nove_id = loop {
        let kandidat = format!("{}-{}", volba_id, n);
        if !rok.menu[mi].volby.iter().any(|v| v.id == kandidat) {
            break kandidat;
        }
        n += 1;
    };
    let mut nova = puvodni.clone();
    nova.id = nove_id.clone();
    nova.nazev = format!("{} ({})", puvodni.nazev, n);
    nova.ucitel = ucitel.to_string();
    rok.menu[mi].volby.push(nova);
    let tridy = rok.menu[mi].tridy.clone();
    let mut clenove: Vec<usize> = rok
        .studenti
        .iter()
        .enumerate()
        .filter(|(_, s)| {
            s.v_rozvrhu()
                && tridy.contains(&s.trida)
                && s.volby.get(menu_id).is_some_and(|v| v.iter().any(|x| x == volba_id))
        })
        .map(|(i, _)| i)
        .collect();
    clenove.sort_by(|&a, &b| rok.studenti[a].jmeno.cmp(&rok.studenti[b].jmeno));
    let polovina = clenove.len() / 2;
    for &i in &clenove[polovina..] {
        if let Some(v) = rok.studenti[i].volby.get_mut(menu_id) {
            for x in v.iter_mut() {
                if x == volba_id {
                    *x = nove_id.clone();
                }
            }
        }
    }
    Some(nove_id)
}

/// Velikost tříd v seed datech (⚠ odhad 20–30).
fn odhad_velikosti(t: &Trida) -> usize {
    match (t.typ, t.rocnik) {
        (TypStudia::Osmilete, 1..=2) => 28,
        (TypStudia::Osmilete, 3) => 28,
        (TypStudia::Osmilete, 4..=5) => 26,
        (TypStudia::Osmilete, 6) => 25,
        (TypStudia::Osmilete, 7) => 24,
        (TypStudia::Osmilete, _) => 20,
        (TypStudia::Ctyrlete, 1) => 28,
        (TypStudia::Ctyrlete, 2) => 28,
        (TypStudia::Ctyrlete, 3) => 24,
        (TypStudia::Ctyrlete, _) => 20,
    }
}

/// Zástupní žáci pro seed („Žákyně 01“, „Žák 02“ …) s rozdělením 15:15 a doplněnými volbami,
/// aby šel rozvrh hned vygenerovat. Skutečné seznamy se zadají v průvodci / na obrazovce Studenti.
pub fn zastupni_zaci(skola: &Skola, rok: &mut SkolniRok) {
    for t in &skola.tridy {
        let n = odhad_velikosti(t);
        for i in 0..n {
            let pohlavi = if i % 2 == 0 { Pohlavi::D } else { Pohlavi::CH };
            let jmeno = match pohlavi {
                Pohlavi::D => format!("Žákyně {:02}", i + 1),
                Pohlavi::CH => format!("Žák {:02}", i + 1),
            };
            let id = rok.nove_id();
            rok.studenti.push(Student {
                id,
                jmeno,
                trida: t.id.clone(),
                skupina_aj: SkupinaAj::A,
                pohlavi,
                volby: BTreeMap::new(),
                status: StavStudenta::Aktivni,
            });
        }
        rozdel_tridu(rok, &t.id);
    }
    let ids: Vec<String> = rok.menu.iter().map(|m| m.id.clone()).collect();
    for (i, id) in ids.iter().enumerate() {
        doplnit_volby(rok, id, 1000 + i as u64);
    }
}
