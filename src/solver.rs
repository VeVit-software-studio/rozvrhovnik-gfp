//! Řešič rozvrhu (vlastní, bez externího solveru):
//!
//! lekce ➡ jednotky (sync) ➡ konflikty (bitset) ➡ greedy ➡ simulované žíhání
//! ➡ slep_bloky + dočištění ➡ místnosti ➡ validace a report.
//!
//! Tvrdá omezení se počítají přesně pro každý „profil“ žáků (žáci se stejnou sadou
//! hodin mají stejný rozvrh), takže pravidlo 7 hodin, limity P1 i okna platí per žák.

use crate::data::*;
use crate::validation;
use rand::{rngs::StdRng, seq::SliceRandom, Rng, SeedableRng};
use serde::{Deserialize, Serialize};
use std::cmp::Reverse;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::Instant;

/// Neumístěná jednotka.
pub const NENI: u8 = u8::MAX;
/// Váha jednoho tvrdého porušení vůči měkkým penalizacím (měkké jsou váženy počtem žáků).
const H: i64 = 50_000;
/// Měřítko měkkých penalizací učitele (okna, preferované třídy) vůči penalizacím žáků,
/// které se násobí počtem žáků profilu.
const MERITKO_UCITELE: i64 = 5;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Tyden {
    #[default]
    Oba,
    L,
    S,
}

impl Tyden {
    pub fn prekryv(self, o: Tyden) -> bool {
        self == Tyden::Oba || o == Tyden::Oba || self == o
    }
    pub fn prefix(self) -> &'static str {
        match self {
            Tyden::Oba => "",
            Tyden::L => "L·",
            Tyden::S => "S·",
        }
    }
    /// Indexy týdnů (0 = lichý, 1 = sudý), ve kterých lekce probíhá.
    pub fn tydny(self) -> &'static [usize] {
        match self {
            Tyden::Oba => &[0, 1],
            Tyden::L => &[0],
            Tyden::S => &[1],
        }
    }
    fn maska(self) -> u8 {
        match self {
            Tyden::Oba => 3,
            Tyden::L => 1,
            Tyden::S => 2,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LekceInfo {
    pub predmet: String,
    pub ucitel: String,
    pub tridy: Vec<String>,
    pub skupina: String,
    pub zaci: Vec<u32>,
    pub len: u8,
    /// Odborné místnosti v pořadí preference.
    pub kandidati: Vec<String>,
    pub kmenova_ok: bool,
    /// Stabilní ID lekce – pro piny (přežije přegenerování).
    pub klic: String,
    /// Klíč předmětu+skupiny (bez pořadí) – pro „max 2 h/den“ a slepování.
    pub klic_predmetu: String,
    /// Partner v L/S páru.
    pub par: Option<usize>,
    pub tyden: Tyden,
    /// Preferovat dopoledne (semináře).
    pub dopoledne: bool,
    pub jednotka: usize,
    pub menu: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct Vysledek {
    pub rok: String,
    pub lekce: Vec<LekceInfo>,
    /// Začátek lekce: den*12+slot.
    pub slot: Vec<Option<u8>>,
    pub mistnost: Vec<Option<String>>,
    pub varovani: Vec<String>,
    pub problemy: Vec<String>,
    pub problemove_lekce: Vec<usize>,
    /// (tvrdé, měkké)
    pub skore: (i64, i64),
    pub iteraci: u64,
    pub cas_s: f32,
    /// Efektivní limity P1 po auto-uvolnění.
    pub limity: BTreeMap<String, Omezeni>,
    pub zaci_tridy: BTreeMap<u32, String>,
    pub virtualni_zaci: Vec<u32>,
}

impl Vysledek {
    pub fn clenove_jednotky(&self, j: usize) -> Vec<usize> {
        (0..self.lekce.len()).filter(|&i| self.lekce[i].jednotka == j).collect()
    }
    pub fn pocet_realnych_zaku(&self, i: usize) -> usize {
        self.lekce[i].zaci.iter().filter(|z| !self.virtualni_zaci.contains(z)).count()
    }
}

/// Průběh řešení pro GUI.
#[derive(Clone, Debug, Default)]
pub struct Prubeh {
    pub faze: String,
    pub iterace: u64,
    pub tvrde: i64,
    pub mekke: i64,
    pub cas_s: f32,
}

// ───────────────────────────── stavba lekcí ─────────────────────────────

pub struct Vstup {
    pub lekce: Vec<LekceInfo>,
    pub jednotky: Vec<Vec<usize>>,
    pub varovani: Vec<String>,
    pub virtualni: Vec<u32>,
    pub zaci_tridy: BTreeMap<u32, String>,
}

struct Zk {
    id: u32,
    aj: SkupinaAj,
    poh: Pohlavi,
}

struct Nova {
    predmet: String,
    ucitel: String,
    tridy: Vec<String>,
    skupina: String,
    zaci: Vec<u32>,
    len: u8,
    klic: String,
    klic_predmetu: String,
    tyden: Tyden,
    dopoledne: bool,
    menu: Option<String>,
}

fn pridej_lekci(skola: &Skola, lekce: &mut Vec<LekceInfo>, n: Nova) -> Option<usize> {
    if n.zaci.is_empty() {
        return None;
    }
    let (mut kandidati, kmenova_ok) = match skola.predmet(&n.predmet) {
        Some(p) => (p.specialni_mistnosti.clone(), p.kmenova_ok),
        None => (vec![], true),
    };
    kandidati.retain(|m| skola.mistnost(m).is_some());
    if let Some(pref) = skola.ucitel(&n.ucitel).and_then(|u| u.preferovana_mistnost.as_ref()) {
        if let Some(pos) = kandidati.iter().position(|m| m == pref) {
            let m = kandidati.remove(pos);
            kandidati.insert(0, m);
        }
    }
    lekce.push(LekceInfo {
        predmet: n.predmet,
        ucitel: n.ucitel,
        tridy: n.tridy,
        skupina: n.skupina,
        zaci: n.zaci,
        len: n.len,
        kandidati,
        kmenova_ok,
        klic: n.klic,
        klic_predmetu: n.klic_predmetu,
        par: None,
        tyden: n.tyden,
        dopoledne: n.dopoledne,
        jednotka: usize::MAX,
        menu: n.menu,
    });
    Some(lekce.len() - 1)
}

/// Rozpis hodin na délky lekcí: 3 h ve dvojhodinách = [2, 1].
pub fn rozpis(hodin: u8, dvoj: bool) -> Vec<u8> {
    let dvojic = if dvoj { hodin / 2 } else { 0 };
    let mut v = vec![2u8; dvojic as usize];
    v.extend(std::iter::repeat(1u8).take((hodin - 2 * dvojic) as usize));
    v
}

/// Sestaví lekce (z učebního plánu a z menu voleb) a jednotky (lekce, které se hýbou společně).
pub fn postav_lekce(skola: &Skola, rok: &SkolniRok) -> Vstup {
    let mut varovani = Vec::new();
    let mut zaci_tridy = BTreeMap::new();
    let mut podle_tridy: BTreeMap<String, Vec<Zk>> = BTreeMap::new();
    for s in rok.studenti.iter().filter(|s| s.v_rozvrhu()) {
        if skola.trida(&s.trida).is_none() {
            varovani.push(format!("Žák {} má neznámou třídu „{}“ – vynechán.", s.jmeno, s.trida));
            continue;
        }
        podle_tridy.entry(s.trida.clone()).or_default().push(Zk { id: s.id, aj: s.skupina_aj, poh: s.pohlavi });
        zaci_tridy.insert(s.id, s.trida.clone());
    }
    let mut virtualni = Vec::new();
    let mut vid = u32::MAX;
    let vstupni = skola.vstupni_tridy();
    let mut vynechane: Vec<String> = Vec::new();
    for t in &skola.tridy {
        let v = podle_tridy.entry(t.id.clone()).or_default();
        if v.is_empty() && !vstupni.contains(&t.id) {
            varovani.push(format!("Třída {} nemá žádné žáky – vynechána z rozvrhu (dobíhající třída?).", t.nazev));
            vynechane.push(t.id.clone());
            continue;
        }
        if v.is_empty() {
            varovani
                .push(format!("Vstupní třída {} zatím nemá žáky – pro plánování použiti 4 zástupní žáci.", t.nazev));
            for (aj, poh) in [
                (SkupinaAj::A, Pohlavi::D),
                (SkupinaAj::A, Pohlavi::CH),
                (SkupinaAj::B, Pohlavi::D),
                (SkupinaAj::B, Pohlavi::CH),
            ] {
                v.push(Zk { id: vid, aj, poh });
                zaci_tridy.insert(vid, t.id.clone());
                virtualni.push(vid);
                vid -= 1;
            }
        }
    }

    let mut lekce: Vec<LekceInfo> = Vec::new();
    let mut jednotky: Vec<Vec<usize>> = Vec::new();
    let mut poradi: HashMap<String, usize> = HashMap::new();

    for ph in &skola.plan {
        if vynechane.contains(&ph.trida) {
            continue;
        }
        if skola.trida(&ph.trida).is_none() {
            varovani.push(format!("Plán: neznámá třída „{}“ ({}).", ph.trida, ph.predmet));
            continue;
        }
        let zaci = &podle_tridy[&ph.trida];
        let vsichni: Vec<u32> = zaci.iter().map(|z| z.id).collect();
        let aj = |g: SkupinaAj| zaci.iter().filter(|z| z.aj == g).map(|z| z.id).collect::<Vec<_>>();
        let po = |g: Pohlavi| zaci.iter().filter(|z| z.poh == g).map(|z| z.id).collect::<Vec<_>>();
        let tridy = vec![ph.trida.clone()];
        for len in rozpis(ph.hodin, ph.dvoj) {
            let zakl = format!("{}|{}", ph.trida, ph.predmet);
            let ord = poradi.entry(zakl.clone()).or_insert(0);
            let o = *ord;
            *ord += 1;
            let mk = |predmet: &str, ucitel: &str, skupina: &str, klic_sk: &str, zaci: Vec<u32>, tyden: Tyden| Nova {
                predmet: predmet.to_string(),
                ucitel: ucitel.to_string(),
                tridy: tridy.clone(),
                skupina: skupina.to_string(),
                zaci,
                len,
                klic: format!("P|{}|{}|{}", zakl, klic_sk, o),
                klic_predmetu: format!("P|{}|{}", zakl, skupina),
                tyden,
                dopoledne: false,
                menu: None,
            };
            let vlozit = |lekce: &mut Vec<LekceInfo>, jed: &mut Vec<usize>, n: Nova| -> Option<usize> {
                let i = pridej_lekci(skola, lekce, n)?;
                jed.push(i);
                Some(i)
            };
            match &ph.struktura {
                Struktura::CelouTridou(u) => {
                    let mut j = vec![];
                    vlozit(&mut lekce, &mut j, mk(&ph.predmet, u, "", "", vsichni.clone(), Tyden::Oba));
                    jednotky.push(j);
                }
                Struktura::Rozdeleno { a, b } => {
                    for (g, u) in [(SkupinaAj::A, a), (SkupinaAj::B, b)] {
                        let mut j = vec![];
                        vlozit(&mut lekce, &mut j, mk(&ph.predmet, u, g.nazev(), g.nazev(), aj(g), Tyden::Oba));
                        jednotky.push(j);
                    }
                }
                Struktura::Pohlavi { div, chl } => {
                    let mut j = vec![];
                    vlozit(&mut lekce, &mut j, mk(&ph.predmet, div, "D", "D", po(Pohlavi::D), Tyden::Oba));
                    vlozit(&mut lekce, &mut j, mk(&ph.predmet, chl, "CH", "CH", po(Pohlavi::CH), Tyden::Oba));
                    jednotky.push(j);
                }
                Struktura::Stridave { l, s } => {
                    let mut j = vec![];
                    let a = vlozit(&mut lekce, &mut j, mk(&l.predmet, &l.ucitel, "", "L", vsichni.clone(), Tyden::L));
                    let b = vlozit(&mut lekce, &mut j, mk(&s.predmet, &s.ucitel, "", "S", vsichni.clone(), Tyden::S));
                    if let (Some(a), Some(b)) = (a, b) {
                        lekce[a].par = Some(b);
                        lekce[b].par = Some(a);
                    }
                    jednotky.push(j);
                }
                Struktura::StridaveSkupiny { a, b } => {
                    for (g, pary) in [(SkupinaAj::A, a), (SkupinaAj::B, b)] {
                        let mut j = vec![];
                        let x = vlozit(
                            &mut lekce,
                            &mut j,
                            mk(
                                &pary[0].predmet,
                                &pary[0].ucitel,
                                g.nazev(),
                                &format!("{}-L", g.nazev()),
                                aj(g),
                                Tyden::L,
                            ),
                        );
                        let y = vlozit(
                            &mut lekce,
                            &mut j,
                            mk(
                                &pary[1].predmet,
                                &pary[1].ucitel,
                                g.nazev(),
                                &format!("{}-S", g.nazev()),
                                aj(g),
                                Tyden::S,
                            ),
                        );
                        if let (Some(x), Some(y)) = (x, y) {
                            lekce[x].par = Some(y);
                            lekce[y].par = Some(x);
                        }
                        jednotky.push(j);
                    }
                }
            }
        }
    }

    // ── volitelné předměty ──
    for m in &rok.menu {
        let clenove: Vec<&Student> = rok
            .studenti
            .iter()
            .filter(|s| s.v_rozvrhu() && m.tridy.contains(&s.trida) && zaci_tridy.contains_key(&s.id))
            .collect();
        let nekompletni = clenove
            .iter()
            .filter(|s| {
                s.volby.get(&m.id).map_or(0, |v| v.iter().filter(|x| m.volby.iter().any(|y| &y.id == *x)).count())
                    < m.pocet_voleb as usize
            })
            .count();
        if nekompletni > 0 {
            varovani.push(format!("{}: {} žáků nemá kompletní volby.", m.nazev, nekompletni));
        }
        let delky = rozpis(m.hodin_tydne, m.dvojhodina);
        let seminar = m.pocet_voleb > 1;
        let mut serie: Vec<Vec<usize>> = Vec::new(); // pro každou volbu: lekce v pořadí
        for v in &m.volby {
            let zaci: Vec<u32> = clenove
                .iter()
                .filter(|s| s.volby.get(&m.id).is_some_and(|x| x.contains(&v.id)))
                .map(|s| s.id)
                .collect();
            if zaci.is_empty() {
                varovani.push(format!("{}: volba {} nemá žádné žáky – nevytvořena.", m.nazev, v.nazev));
                serie.push(vec![]);
                continue;
            }
            if let Some(k) = v.kapacita {
                if zaci.len() > k as usize {
                    varovani.push(format!(
                        "{}: volba {} má {} žáků > kapacita {} (✂ rozdělit).",
                        m.nazev,
                        v.nazev,
                        zaci.len(),
                        k
                    ));
                }
            }
            let tridy: BTreeSet<String> = zaci.iter().map(|z| zaci_tridy[z].clone()).collect();
            let mut s = vec![];
            for (i, &len) in delky.iter().enumerate() {
                let n = Nova {
                    predmet: v.predmet.clone(),
                    ucitel: v.ucitel.clone(),
                    tridy: tridy.iter().cloned().collect(),
                    skupina: v.id.clone(),
                    zaci: zaci.clone(),
                    len,
                    klic: format!("M|{}|{}|{}", m.id, v.id, i),
                    klic_predmetu: format!("M|{}|{}", m.id, v.id),
                    tyden: Tyden::Oba,
                    dopoledne: seminar && m.dvojhodina,
                    menu: Some(m.id.clone()),
                };
                if let Some(x) = pridej_lekci(skola, &mut lekce, n) {
                    s.push(x);
                }
            }
            serie.push(s);
        }
        if m.pocet_voleb == 1 {
            // „vyber 1“ ➡ všechny volby paralelně ve stejném slotu
            for i in 0..delky.len() {
                let j: Vec<usize> = serie.iter().filter_map(|s| s.get(i).copied()).collect();
                if !j.is_empty() {
                    jednotky.push(j);
                }
            }
        } else {
            for s in serie {
                for x in s {
                    jednotky.push(vec![x]);
                }
            }
        }
    }
    for (j, cl) in jednotky.iter().enumerate() {
        for &i in cl {
            lekce[i].jednotka = j;
        }
    }
    jednotky.retain(|j| !j.is_empty());
    for (j, cl) in jednotky.iter().enumerate() {
        for &i in cl {
            lekce[i].jednotka = j;
        }
    }
    Vstup { lekce, jednotky, varovani, virtualni, zaci_tridy }
}

/// Úvazky učitelů (h/týden, L/S hodina = 0,5) – spočítáno z plánu a voleb, bez generování.
pub fn zateze(skola: &Skola, rok: &SkolniRok) -> Vec<(String, f32)> {
    let vstup = postav_lekce(skola, rok);
    let mut z: BTreeMap<String, f32> = skola.ucitele.iter().map(|u| (u.id.clone(), 0.0)).collect();
    for l in &vstup.lekce {
        let f = if l.tyden == Tyden::Oba { 1.0 } else { 0.5 };
        *z.entry(l.ucitel.clone()).or_default() += l.len as f32 * f;
    }
    let mut v: Vec<(String, f32)> = z.into_iter().collect();
    v.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap().then_with(|| a.0.cmp(&b.0)));
    v
}

// ───────────────────────────── limity P1 a auto-uvolnění ─────────────────────────────

fn kapacita_tydne(lim: Omezeni, za_den: bool, odpo_od: usize) -> usize {
    let stred = odpo_od.saturating_sub(1);
    let odpo_max = SLOTU - odpo_od;
    let cap = if za_den {
        let e = lim.rane.map_or(1, |x| (x as usize).min(1));
        let a = lim.odpoledni.map_or(odpo_max, |x| (x as usize).min(odpo_max));
        DNY * (stred + e + a)
    } else {
        let e = lim.rane.map_or(DNY, |x| (x as usize).min(DNY));
        let a = lim.odpoledni.map_or(DNY * odpo_max, |x| (x as usize).min(DNY * odpo_max));
        DNY * stred + e + a
    };
    cap.min(DNY * MAX_DEN_TRIDA)
}

/// Povolené sloty dne pro třídu s daným limitem.
pub fn maska_tridy(lim: Omezeni, odpo_od: usize) -> [bool; SLOTU] {
    let mut m = [true; SLOTU];
    if lim.rane == Some(0) {
        m[0] = false;
    }
    if let Some(a) = lim.odpoledni {
        for (k, x) in m.iter_mut().enumerate().skip(odpo_od) {
            *x = k < odpo_od + a as usize;
        }
    }
    m
}

// ───────────────────────────── model pro řešič ─────────────────────────────

#[derive(Clone)]
struct ProfInfo {
    size: i64,
    rane: Option<u8>,
    odpo: Option<u8>,
    w_odpo: i64,
}

struct Model {
    n_u: usize,
    ulen: Vec<u8>,
    allowed: Vec<u64>,
    conf: Vec<Vec<u64>>,
    uprof: Vec<Vec<u32>>,
    uteach: Vec<Vec<u32>>,
    /// Podmnožina `uteach`: učitelé jednotky, pro které je některá třída jednotky preferovaná.
    uteach_pref: Vec<Vec<u32>>,
    ukeys: Vec<Vec<u32>>,
    upools: Vec<Vec<(u32, u8)>>,
    uplace: Vec<[i64; N_SLOTU]>,
    usize_: Vec<i64>,
    prof_units: Vec<Vec<u32>>,
    prof: Vec<ProfInfo>,
    teach_max: Vec<u8>,
    key_len: Vec<u8>,
    key_size: Vec<i64>,
    pool_cap: Vec<u8>,
    za_den: bool,
    odpo_od: usize,
    vahy: Vahy,
    strukturalni: i64,
    limity: BTreeMap<String, Omezeni>,
}

impl Model {
    #[inline]
    fn konflikt(&self, u: usize, v: usize) -> bool {
        (self.conf[u][v >> 6] >> (v & 63)) & 1 == 1
    }

    fn postav(skola: &Skola, vs: &Vstup, piny: &BTreeMap<String, u8>, varovani: &mut Vec<String>) -> Model {
        let nast = &skola.nastaveni;
        let za_den = nast.limity_za_den;
        let odpo_od = (nast.odpoledne_od as usize).clamp(1, SLOTU);
        let lekce = &vs.lekce;
        let n_u = vs.jednotky.len();
        let ulen: Vec<u8> = vs.jednotky.iter().map(|j| j.iter().map(|&i| lekce[i].len).max().unwrap_or(1)).collect();
        let usize_: Vec<i64> =
            vs.jednotky.iter().map(|j| j.iter().map(|&i| lekce[i].zaci.len() as i64).sum::<i64>().max(1)).collect();

        // žák ➡ lekce
        let mut zak_lekce: HashMap<u32, Vec<usize>> = HashMap::new();
        for (i, l) in lekce.iter().enumerate() {
            for &z in &l.zaci {
                zak_lekce.entry(z).or_default().push(i);
            }
        }

        // profily
        let mut profily: HashMap<Vec<u32>, usize> = HashMap::new();
        let mut prof_units: Vec<Vec<u32>> = Vec::new();
        let mut prof_trida: Vec<String> = Vec::new();
        let mut prof_size: Vec<i64> = Vec::new();
        let mut zaci: Vec<(&u32, &String)> = vs.zaci_tridy.iter().collect();
        zaci.sort();
        for (z, tr) in zaci {
            let Some(ls) = zak_lekce.get(z) else { continue };
            let mut us: Vec<u32> = ls.iter().map(|&i| lekce[i].jednotka as u32).collect();
            us.sort_unstable();
            us.dedup();
            let p = *profily.entry(us.clone()).or_insert_with(|| {
                prof_units.push(us);
                prof_trida.push(tr.clone());
                prof_size.push(0);
                prof_units.len() - 1
            });
            prof_size[p] += 1;
        }

        // limity P1 + auto-uvolnění
        let mut limity: BTreeMap<String, Omezeni> = BTreeMap::new();
        for t in &skola.tridy {
            let mut lim = t.omezeni();
            let potreba = prof_units
                .iter()
                .zip(&prof_trida)
                .filter(|(_, tr)| **tr == t.id)
                .map(|(us, _)| us.iter().map(|&u| ulen[u as usize] as usize).sum::<usize>())
                .max()
                .unwrap_or(0);
            let cap0 = kapacita_tydne(lim, za_den, odpo_od);
            if potreba > cap0 {
                if nast.auto_uvolneni {
                    let puvodni = lim;
                    while kapacita_tydne(lim, za_den, odpo_od) < potreba {
                        match (lim.odpoledni, lim.rane) {
                            (Some(a), _)
                                if (a as usize) < if za_den { SLOTU - odpo_od } else { DNY * (SLOTU - odpo_od) } =>
                            {
                                lim.odpoledni = Some(a + 1)
                            }
                            (_, Some(e)) if (e as usize) < if za_den { 1 } else { DNY } => lim.rane = Some(e + 1),
                            _ => break,
                        }
                    }
                    let jed = if za_den { "/den" } else { "/týden" };
                    varovani.push(format!(
                        "AUTO-UVOLNĚNO {}: kapacita {} < potřeba {} h ➡ {} ➡ {} ({}).",
                        t.nazev,
                        cap0,
                        potreba,
                        puvodni.popis(),
                        lim.popis(),
                        jed
                    ));
                } else {
                    varovani.push(format!(
                        "NEŘEŠITELNÉ {}: kapacita {} < potřeba {} h (auto-uvolnění vypnuto).",
                        t.nazev, cap0, potreba
                    ));
                }
            }
            limity.insert(t.id.clone(), lim);
        }
        let masky: HashMap<&str, [bool; SLOTU]> =
            limity.iter().map(|(t, l)| (t.as_str(), maska_tridy(*l, odpo_od))).collect();
        let prof: Vec<ProfInfo> = prof_trida
            .iter()
            .zip(&prof_size)
            .map(|(tr, &size)| {
                let lim = limity.get(tr).copied().unwrap_or_default();
                let ef = skola.trida(tr).map_or(8, |t| t.efektivni_rocnik());
                let w = if ef <= 5 { nast.vahy.odpoledne_nizsi } else { nast.vahy.odpoledne_vyssi };
                ProfInfo { size, rane: lim.rane, odpo: lim.odpoledni, w_odpo: w as i64 }
            })
            .collect();
        let mut uprof: Vec<Vec<u32>> = vec![vec![]; n_u];
        for (p, us) in prof_units.iter().enumerate() {
            for &u in us {
                uprof[u as usize].push(p as u32);
            }
        }

        // učitelé
        let mut uc_idx: HashMap<&str, usize> = HashMap::new();
        let mut teach_max: Vec<u8> = Vec::new();
        let mut uteach: Vec<Vec<u32>> = vec![vec![]; n_u];
        let mut uteach_pref: Vec<Vec<u32>> = vec![vec![]; n_u];
        for (u, j) in vs.jednotky.iter().enumerate() {
            for &i in j {
                let id = lekce[i].ucitel.as_str();
                let t = *uc_idx.entry(id).or_insert_with(|| {
                    teach_max.push(skola.ucitel(id).map_or(8, |x| x.max_den.max(1)));
                    teach_max.len() - 1
                });
                if !uteach[u].contains(&(t as u32)) {
                    uteach[u].push(t as u32);
                }
                let preferuje = skola
                    .ucitel(id)
                    .is_some_and(|uc| lekce[i].tridy.iter().any(|tr| uc.preferovane_tridy.contains(tr)));
                if preferuje && !uteach_pref[u].contains(&(t as u32)) {
                    uteach_pref[u].push(t as u32);
                }
            }
        }

        // klíče předmětů
        let mut key_idx: HashMap<&str, usize> = HashMap::new();
        let mut key_len: Vec<u8> = Vec::new();
        let mut key_size: Vec<i64> = Vec::new();
        let mut ukeys: Vec<Vec<u32>> = vec![vec![]; n_u];
        for (u, j) in vs.jednotky.iter().enumerate() {
            for &i in j {
                let l = &lekce[i];
                let k = *key_idx.entry(l.klic_predmetu.as_str()).or_insert_with(|| {
                    key_len.push(0);
                    key_size.push(0);
                    key_len.len() - 1
                });
                key_len[k] = key_len[k].max(l.len);
                key_size[k] = key_size[k].max(l.zaci.len() as i64);
                if !ukeys[u].contains(&(k as u32)) {
                    ukeys[u].push(k as u32);
                }
            }
        }

        // bazény místností (lekce, které nesmí do kmenové)
        let mut pool_idx: HashMap<String, usize> = HashMap::new();
        let mut pool_cap: Vec<u8> = Vec::new();
        let mut upools: Vec<Vec<(u32, u8)>> = vec![vec![]; n_u];
        for (u, j) in vs.jednotky.iter().enumerate() {
            for &i in j {
                let l = &lekce[i];
                if l.kmenova_ok || l.kandidati.is_empty() {
                    continue;
                }
                let mut k = l.kandidati.clone();
                k.sort();
                let sig = k.join("|");
                let p = *pool_idx.entry(sig).or_insert_with(|| {
                    pool_cap.push(k.len().min(250) as u8);
                    pool_cap.len() - 1
                });
                upools[u].push((p as u32, l.tyden.maska()));
            }
        }

        // povolené začátky
        let mut allowed = vec![0u64; n_u];
        let mut pinovane: HashSet<&str> = HashSet::new();
        for (u, j) in vs.jednotky.iter().enumerate() {
            let len = ulen[u] as usize;
            let mut fit = 0u64;
            let mut mask = 0u64;
            for s in 0..N_SLOTU {
                let (_, k) = den_a_slot(s);
                if k + len > SLOTU {
                    continue;
                }
                fit |= 1 << s;
                let ok = j.iter().all(|&i| {
                    let l = &lekce[i];
                    let volno_ok =
                        skola.ucitel(&l.ucitel).map_or(true, |uc| !(s..s + len).any(|x| uc.volno.contains(&(x as u8))));
                    let tridy_ok =
                        l.tridy.iter().all(|t| masky.get(t.as_str()).map_or(true, |m| (k..k + len).all(|x| m[x])));
                    volno_ok && tridy_ok
                });
                if ok {
                    mask |= 1 << s;
                }
            }
            if mask == 0 {
                let l = &lekce[j[0]];
                varovani.push(format!(
                    "Lekce {} ({}) nemá žádný povolený slot (limity P1 / volno učitele) – umístěna nouzově.",
                    l.predmet,
                    l.tridy.join("+")
                ));
                mask = fit;
            }
            for &i in j {
                if let Some(&pin) = piny.get(&lekce[i].klic) {
                    pinovane.insert(lekce[i].klic.as_str());
                    if (pin as usize) < N_SLOTU && fit & (1 << pin) != 0 {
                        mask = 1 << pin;
                    } else {
                        varovani.push(format!("Pin {} nelze aplikovat (slot mimo den).", lekce[i].klic));
                    }
                }
            }
            allowed[u] = mask;
        }
        for k in piny.keys() {
            if !pinovane.contains(k.as_str()) {
                varovani.push(format!("Pin {} neodpovídá žádné lekci (data se změnila) – neaplikován.", k));
            }
        }

        // konflikty (bitset jednotek)
        let slova = n_u.div_ceil(64);
        let mut conf = vec![vec![0u64; slova]; n_u];
        let mut strukt: HashSet<(usize, usize)> = HashSet::new();
        let mut oznac = |a: usize, b: usize, conf: &mut Vec<Vec<u64>>| {
            if lekce[a].tyden.prekryv(lekce[b].tyden) {
                let (ua, ub) = (lekce[a].jednotka, lekce[b].jednotka);
                if ua == ub {
                    strukt.insert((a.min(b), a.max(b)));
                } else {
                    conf[ua][ub >> 6] |= 1 << (ub & 63);
                    conf[ub][ua >> 6] |= 1 << (ua & 63);
                }
            }
        };
        let mut podle_ucitele: HashMap<&str, Vec<usize>> = HashMap::new();
        for (i, l) in lekce.iter().enumerate() {
            podle_ucitele.entry(l.ucitel.as_str()).or_default().push(i);
        }
        for ls in podle_ucitele.values().chain(zak_lekce.values()) {
            for x in 0..ls.len() {
                for y in x + 1..ls.len() {
                    oznac(ls[x], ls[y], &mut conf);
                }
            }
        }
        for &(a, b) in &strukt {
            varovani.push(format!(
                "Vnitřní kolize jednotky: {} × {} (stejný učitel/žák ve stejném týdnu).",
                popis_lekce(&lekce[a]),
                popis_lekce(&lekce[b])
            ));
        }

        // statická cena umístění (semináře odpoledne)
        let mut uplace = vec![[0i64; N_SLOTU]; n_u];
        for (u, j) in vs.jednotky.iter().enumerate() {
            if j.iter().any(|&i| lekce[i].dopoledne) {
                for s in 0..N_SLOTU {
                    let (_, k) = den_a_slot(s);
                    let odpo = (k..k + ulen[u] as usize).filter(|&x| x >= odpo_od).count() as i64;
                    uplace[u][s] = nast.vahy.seminar_odpoledne as i64 * odpo * usize_[u];
                }
            }
        }

        Model {
            n_u,
            ulen,
            allowed,
            conf,
            uprof,
            uteach,
            uteach_pref,
            ukeys,
            upools,
            uplace,
            usize_,
            prof_units,
            prof,
            teach_max,
            key_len,
            key_size,
            pool_cap,
            za_den,
            odpo_od,
            vahy: nast.vahy,
            strukturalni: strukt.len() as i64,
            limity,
        }
    }
}

pub fn popis_lekce(l: &LekceInfo) -> String {
    let sk = if l.skupina.is_empty() { String::new() } else { format!(" {}", l.skupina) };
    format!("{}{}{} ({}, {})", l.tyden.prefix(), l.predmet, sk, l.tridy.join("+"), l.ucitel)
}

// ───────────────────────────── stav a inkrementální cena ─────────────────────────────

#[derive(Clone, Copy, Default)]
struct DenC {
    hard: i64,
    soft: i64,
    rane: u8,
    odpo: u8,
}

struct Stav<'m> {
    m: &'m Model,
    start: Vec<u8>,
    slot_units: Vec<Vec<u32>>,
    prof_occ: Vec<[u8; N_SLOTU]>,
    teach_occ: Vec<[u8; N_SLOTU]>,
    /// Obsazenost učitele hodinami jeho preferovaných tříd.
    teach_pref_occ: Vec<[u8; N_SLOTU]>,
    key_occ: Vec<[u8; N_SLOTU]>,
    pool_occ: Vec<[[u8; N_SLOTU]; 2]>,
    prof_day: Vec<[DenC; DNY]>,
    prof_week: Vec<i64>,
    teach_day: Vec<[(i64, i64); DNY]>,
    key_day: Vec<[(i64, i64); DNY]>,
    hard: i64,
    soft: i64,
}

fn beh_a_okna(occ: &[u8]) -> (usize, usize, usize, usize) {
    // (počet, okna, přesah nad MAX_RUN, počet segmentů)
    let mut n = 0;
    let mut first = usize::MAX;
    let mut last = 0;
    let mut run: usize = 0;
    let mut presah = 0;
    let mut seg = 0;
    for (k, &o) in occ.iter().enumerate() {
        if o > 0 {
            if run == 0 {
                seg += 1;
            }
            n += 1;
            first = first.min(k);
            last = k;
            run += 1;
        } else {
            presah += run.saturating_sub(MAX_RUN);
            run = 0;
        }
    }
    presah += run.saturating_sub(MAX_RUN);
    let okna = if n == 0 { 0 } else { last - first + 1 - n };
    (n, okna, presah, seg)
}

impl<'m> Stav<'m> {
    fn new(m: &'m Model) -> Stav<'m> {
        Stav {
            m,
            start: vec![NENI; m.n_u],
            slot_units: vec![Vec::new(); N_SLOTU],
            prof_occ: vec![[0; N_SLOTU]; m.prof.len()],
            teach_occ: vec![[0; N_SLOTU]; m.teach_max.len()],
            teach_pref_occ: vec![[0; N_SLOTU]; m.teach_max.len()],
            key_occ: vec![[0; N_SLOTU]; m.key_len.len()],
            pool_occ: vec![[[0; N_SLOTU]; 2]; m.pool_cap.len()],
            prof_day: vec![[DenC::default(); DNY]; m.prof.len()],
            prof_week: vec![0; m.prof.len()],
            teach_day: vec![[(0, 0); DNY]; m.teach_max.len()],
            key_day: vec![[(0, 0); DNY]; m.key_len.len()],
            hard: m.strukturalni,
            soft: 0,
        }
    }

    fn obj(&self) -> i64 {
        self.hard * H + self.soft
    }

    fn prof_den(&self, p: usize, d: usize) -> DenC {
        let m = self.m;
        let occ = &self.prof_occ[p][d * SLOTU..(d + 1) * SLOTU];
        let info = &m.prof[p];
        let (n, okna, presah, _) = beh_a_okna(occ);
        let rane = (occ[0] > 0) as u8;
        let odpo = occ[m.odpo_od..].iter().filter(|&&o| o > 0).count() as u8;
        let mut hard = presah as i64 + n.saturating_sub(MAX_DEN_TRIDA) as i64;
        if m.za_den {
            if let Some(l) = info.rane {
                hard += rane.saturating_sub(l) as i64;
            }
            if let Some(l) = info.odpo {
                hard += odpo.saturating_sub(l) as i64;
            }
        }
        let v = &m.vahy;
        let soft = (v.okno as i64 * okna as i64 + v.rana as i64 * rane as i64 + info.w_odpo * odpo as i64) * info.size;
        DenC { hard, soft, rane, odpo }
    }

    fn prof_tyden(&self, p: usize) -> i64 {
        let m = self.m;
        if m.za_den {
            return 0;
        }
        let info = &m.prof[p];
        let r: u32 = self.prof_day[p].iter().map(|d| d.rane as u32).sum();
        let a: u32 = self.prof_day[p].iter().map(|d| d.odpo as u32).sum();
        let mut h = 0;
        if let Some(l) = info.rane {
            h += r.saturating_sub(l as u32) as i64;
        }
        if let Some(l) = info.odpo {
            h += a.saturating_sub(l as u32) as i64;
        }
        h
    }

    fn ucitel_den(&self, t: usize, d: usize) -> (i64, i64) {
        let m = self.m;
        let occ = &self.teach_occ[t][d * SLOTU..(d + 1) * SLOTU];
        let (n, okna, _, _) = beh_a_okna(occ);
        let hard = n.saturating_sub(m.teach_max[t] as usize) as i64;
        // preferované třídy učitele: penalizace za jejich brzkou (7:20) a odpolední hodinu
        let pref = &self.teach_pref_occ[t][d * SLOTU..(d + 1) * SLOTU];
        let pref_rane = (pref[0] > 0) as i64;
        let pref_odpo = pref[m.odpo_od..].iter().filter(|&&o| o > 0).count() as i64;
        let soft = (m.vahy.ucitel_okno as i64 * okna as i64
            + m.vahy.ucitel_preference as i64 * (pref_rane + pref_odpo))
            * MERITKO_UCITELE;
        (hard, soft)
    }

    fn klic_den(&self, k: usize, d: usize) -> (i64, i64) {
        let occ = &self.key_occ[k][d * SLOTU..(d + 1) * SLOTU];
        let (n, _, _, seg) = beh_a_okna(occ);
        let hard = n.saturating_sub(MAX_STEJNY_PREDMET_DEN) as i64;
        let navic = n.saturating_sub(self.m.key_len[k] as usize) as i64;
        let soft = self.m.vahy.rozlozeni as i64 * self.m.key_size[k] * (navic + 2 * seg.saturating_sub(1) as i64);
        (hard, soft)
    }

    fn odeber(&mut self, u: usize, s: usize) {
        let m = self.m;
        for t in s..s + m.ulen[u] as usize {
            let su = &mut self.slot_units[t];
            let pos = su.iter().position(|&x| x as usize == u).expect("jednotka ve slotu");
            su.swap_remove(pos);
            for i in 0..self.slot_units[t].len() {
                if m.konflikt(u, self.slot_units[t][i] as usize) {
                    self.hard -= 1;
                }
            }
            for &p in &m.uprof[u] {
                self.prof_occ[p as usize][t] -= 1;
            }
            for &x in &m.uteach[u] {
                self.teach_occ[x as usize][t] -= 1;
            }
            for &x in &m.uteach_pref[u] {
                self.teach_pref_occ[x as usize][t] -= 1;
            }
            for &k in &m.ukeys[u] {
                self.key_occ[k as usize][t] -= 1;
            }
            for &(p, mask) in &m.upools[u] {
                let cap = m.pool_cap[p as usize];
                for w in 0..2 {
                    if mask & (1 << w) != 0 {
                        let o = &mut self.pool_occ[p as usize][w][t];
                        let pred = o.saturating_sub(cap) as i64;
                        *o -= 1;
                        self.hard += o.saturating_sub(cap) as i64 - pred;
                    }
                }
            }
        }
        self.soft -= m.uplace[u][s];
    }

    fn vloz(&mut self, u: usize, s: usize) {
        let m = self.m;
        for t in s..s + m.ulen[u] as usize {
            for i in 0..self.slot_units[t].len() {
                if m.konflikt(u, self.slot_units[t][i] as usize) {
                    self.hard += 1;
                }
            }
            self.slot_units[t].push(u as u32);
            for &p in &m.uprof[u] {
                self.prof_occ[p as usize][t] += 1;
            }
            for &x in &m.uteach[u] {
                self.teach_occ[x as usize][t] += 1;
            }
            for &x in &m.uteach_pref[u] {
                self.teach_pref_occ[x as usize][t] += 1;
            }
            for &k in &m.ukeys[u] {
                self.key_occ[k as usize][t] += 1;
            }
            for &(p, mask) in &m.upools[u] {
                let cap = m.pool_cap[p as usize];
                for w in 0..2 {
                    if mask & (1 << w) != 0 {
                        let o = &mut self.pool_occ[p as usize][w][t];
                        let pred = o.saturating_sub(cap) as i64;
                        *o += 1;
                        self.hard += o.saturating_sub(cap) as i64 - pred;
                    }
                }
            }
        }
        self.soft += m.uplace[u][s];
    }

    /// Přesune jednotku a vrátí změnu účelové funkce.
    fn presun(&mut self, u: usize, new: u8) -> i64 {
        let old = self.start[u];
        if old == new {
            return 0;
        }
        let m = self.m;
        let (h0, s0) = (self.hard, self.soft);
        let mut dny = [0usize; 2];
        let mut nd = 0;
        if old != NENI {
            dny[0] = old as usize / SLOTU;
            nd = 1;
        }
        if new != NENI {
            let d = new as usize / SLOTU;
            if nd == 0 || dny[0] != d {
                dny[nd] = d;
                nd += 1;
            }
        }
        let dny = &dny[..nd];
        for &p in &m.uprof[u] {
            let p = p as usize;
            self.hard -= self.prof_week[p];
            for &d in dny {
                self.hard -= self.prof_day[p][d].hard;
                self.soft -= self.prof_day[p][d].soft;
            }
        }
        for &t in &m.uteach[u] {
            for &d in dny {
                let (h, s) = self.teach_day[t as usize][d];
                self.hard -= h;
                self.soft -= s;
            }
        }
        for &k in &m.ukeys[u] {
            for &d in dny {
                let (h, s) = self.key_day[k as usize][d];
                self.hard -= h;
                self.soft -= s;
            }
        }
        if old != NENI {
            self.odeber(u, old as usize);
        }
        if new != NENI {
            self.vloz(u, new as usize);
        }
        self.start[u] = new;
        for &p in &m.uprof[u] {
            let p = p as usize;
            for &d in dny {
                let c = self.prof_den(p, d);
                self.prof_day[p][d] = c;
                self.hard += c.hard;
                self.soft += c.soft;
            }
            let w = self.prof_tyden(p);
            self.prof_week[p] = w;
            self.hard += w;
        }
        for &t in &m.uteach[u] {
            for &d in dny {
                let c = self.ucitel_den(t as usize, d);
                self.teach_day[t as usize][d] = c;
                self.hard += c.0;
                self.soft += c.1;
            }
        }
        for &k in &m.ukeys[u] {
            for &d in dny {
                let c = self.klic_den(k as usize, d);
                self.key_day[k as usize][d] = c;
                self.hard += c.0;
                self.soft += c.1;
            }
        }
        (self.hard - h0) * H + (self.soft - s0)
    }

    fn nastav(&mut self, starty: &[u8]) {
        for u in 0..self.m.n_u {
            self.presun(u, NENI);
        }
        for (u, &s) in starty.iter().enumerate() {
            self.presun(u, s);
        }
    }

    fn povoleno(&self, u: usize, s: u8) -> bool {
        s != NENI && (self.m.allowed[u] >> s) & 1 == 1
    }

    fn je_spatna(&self, u: usize) -> bool {
        let m = self.m;
        let s = self.start[u];
        if s == NENI {
            return true;
        }
        let s = s as usize;
        let d = s / SLOTU;
        for t in s..s + m.ulen[u] as usize {
            if self.slot_units[t].iter().any(|&v| v as usize != u && m.konflikt(u, v as usize)) {
                return true;
            }
            for &(p, mask) in &m.upools[u] {
                for w in 0..2 {
                    if mask & (1 << w) != 0 && self.pool_occ[p as usize][w][t] > m.pool_cap[p as usize] {
                        return true;
                    }
                }
            }
        }
        m.uprof[u].iter().any(|&p| self.prof_day[p as usize][d].hard > 0 || self.prof_week[p as usize] > 0)
            || m.uteach[u].iter().any(|&t| self.teach_day[t as usize][d].0 > 0)
            || m.ukeys[u].iter().any(|&k| self.key_day[k as usize][d].0 > 0)
    }

    fn spatne(&self) -> Vec<usize> {
        (0..self.m.n_u).filter(|&u| self.m.allowed[u].count_ones() > 1 && self.je_spatna(u)).collect()
    }
}

fn nahodny_bit(mask: u64, rng: &mut StdRng) -> u8 {
    let n = mask.count_ones();
    let mut k = rng.gen_range(0..n);
    let mut m = mask;
    loop {
        let b = m.trailing_zeros();
        if k == 0 {
            return b as u8;
        }
        k -= 1;
        m &= m - 1;
    }
}

fn bity(mask: u64) -> impl Iterator<Item = u8> {
    (0..N_SLOTU as u8).filter(move |&s| (mask >> s) & 1 == 1)
}

// ───────────────────────────── fáze řešení ─────────────────────────────

fn greedy(st: &mut Stav, rng: &mut StdRng) {
    let m = st.m;
    let mut poradi: Vec<usize> = (0..m.n_u).collect();
    poradi.shuffle(rng);
    poradi.sort_by_key(|&u| (m.allowed[u].count_ones(), Reverse(m.usize_[u] * m.ulen[u] as i64)));
    for u in poradi {
        let mut dny: Vec<usize> = (0..DNY).collect();
        dny.shuffle(rng);
        let mut best = (i64::MAX, NENI);
        for d in dny {
            for k in 0..SLOTU {
                let s = (d * SLOTU + k) as u8;
                if !st.povoleno(u, s) {
                    continue;
                }
                let delta = st.presun(u, s);
                st.presun(u, NENI);
                if delta < best.0 {
                    best = (delta, s);
                }
            }
        }
        st.presun(u, best.1);
    }
}

fn hlas(prubeh: Option<&Mutex<Prubeh>>, faze: &str, iter: u64, st: &Stav, t0: Instant) {
    if let Some(p) = prubeh {
        if let Ok(mut p) = p.lock() {
            p.faze = faze.to_string();
            p.iterace = iter;
            p.tvrde = st.hard;
            p.mekke = st.soft;
            p.cas_s = t0.elapsed().as_secs_f32();
        }
    }
}

fn zihani(
    st: &mut Stav,
    rng: &mut StdRng,
    limit_s: f64,
    stop: &AtomicBool,
    prubeh: Option<&Mutex<Prubeh>>,
    t0: Instant,
) -> (Vec<u8>, u64) {
    let m = st.m;
    let pohyblive: Vec<usize> = (0..m.n_u).filter(|&u| m.allowed[u].count_ones() > 1).collect();
    let mut best = st.start.clone();
    let mut best_obj = st.obj();
    if pohyblive.is_empty() {
        return (best, 0);
    }
    let (tmax, tmin) = (2000.0f64, 10.0f64);
    let mut temp = tmax;
    let mut spatne = st.spatne();
    let mut iter: u64 = 0;
    let mut posledni_zlepseni: u64 = 0;
    let mut cas_zlepseni = Instant::now();
    let start_zihani = Instant::now();
    let limit_zihani = (limit_s - t0.elapsed().as_secs_f64()).max(0.5);
    let mut posledni_hlaseni = Instant::now();
    loop {
        iter += 1;
        if iter % 256 == 0 {
            let frac = start_zihani.elapsed().as_secs_f64() / limit_zihani;
            if frac >= 1.0 || stop.load(Ordering::Relaxed) {
                break;
            }
            temp = tmax * (tmin / tmax).powf(frac);
            // bez tvrdých porušení a dlouho bez zlepšení ➡ konec dřív
            if best_obj < H && cas_zlepseni.elapsed().as_secs_f64() > (limit_zihani * 0.25).max(3.0) {
                break;
            }
            if posledni_hlaseni.elapsed().as_millis() > 200 {
                hlas(prubeh, "žíhání", iter, st, t0);
                posledni_hlaseni = Instant::now();
            }
        }
        if iter % 2000 == 0 {
            spatne = st.spatne();
        }
        if iter % 4000 == 0 && st.hard > 0 && iter - posledni_zlepseni > 4000 && !spatne.is_empty() {
            // shake
            for _ in 0..3 {
                let u = spatne[rng.gen_range(0..spatne.len())];
                let s = nahodny_bit(m.allowed[u], rng);
                st.presun(u, s);
            }
        }
        let u = if !spatne.is_empty() && rng.gen_bool(0.8) {
            spatne[rng.gen_range(0..spatne.len())]
        } else {
            pohyblive[rng.gen_range(0..pohyblive.len())]
        };
        let su = st.start[u];
        let prijmi = |delta: i64, rng: &mut StdRng| delta <= 0 || rng.gen::<f64>() < (-(delta as f64) / temp).exp();
        if rng.gen_bool(0.3) && !m.uprof[u].is_empty() {
            // výměna s jednotkou, která sdílí žáky
            let p = m.uprof[u][rng.gen_range(0..m.uprof[u].len())] as usize;
            let v = m.prof_units[p][rng.gen_range(0..m.prof_units[p].len())] as usize;
            let sv = st.start[v];
            if v == u || m.ulen[u] != m.ulen[v] || sv == su || !st.povoleno(u, sv) || !st.povoleno(v, su) {
                continue;
            }
            let d = st.presun(u, sv) + st.presun(v, su);
            if !prijmi(d, rng) {
                st.presun(v, sv);
                st.presun(u, su);
                continue;
            }
        } else {
            let cil = if rng.gen_bool(0.5) {
                let mut nej = (i64::MAX, NENI);
                for _ in 0..6 {
                    let c = nahodny_bit(m.allowed[u], rng);
                    if c == su {
                        continue;
                    }
                    let d = st.presun(u, c);
                    st.presun(u, su);
                    if d < nej.0 {
                        nej = (d, c);
                    }
                }
                nej.1
            } else {
                nahodny_bit(m.allowed[u], rng)
            };
            if cil == NENI || cil == su {
                continue;
            }
            let d = st.presun(u, cil);
            if !prijmi(d, rng) {
                st.presun(u, su);
                continue;
            }
        }
        let o = st.obj();
        if o < best_obj {
            best_obj = o;
            best.copy_from_slice(&st.start);
            posledni_zlepseni = iter;
            cas_zlepseni = Instant::now();
        }
    }
    (best, iter)
}

/// Stejné předměty téhož dne posune vedle sebe (souvislé dvojice), pokud to nezhorší skóre;
/// pak dočištění: každou jednotku zkusí přesunout na nejlepší povolený slot.
fn slep_bloky(st: &mut Stav) {
    let m = st.m;
    for _ in 0..3 {
        let mut zlepseno = false;
        for u in 0..m.n_u {
            let su = st.start[u];
            if su == NENI || m.allowed[u].count_ones() <= 1 {
                continue;
            }
            let d = su as usize / SLOTU;
            let mut kandidati: Vec<u8> = Vec::new();
            for t in d * SLOTU..(d + 1) * SLOTU {
                for &v in &st.slot_units[t] {
                    let v = v as usize;
                    if v != u && m.ukeys[u].iter().any(|k| m.ukeys[v].contains(k)) {
                        let sv = st.start[v] as usize;
                        let lu = m.ulen[u] as usize;
                        if sv >= d * SLOTU + lu {
                            kandidati.push((sv - lu) as u8);
                        }
                        let za = sv + m.ulen[v] as usize;
                        if za + lu <= (d + 1) * SLOTU {
                            kandidati.push(za as u8);
                        }
                    }
                }
            }
            for c in kandidati {
                if c == st.start[u] || !st.povoleno(u, c) {
                    continue;
                }
                let puvodni = st.start[u];
                if st.presun(u, c) < 0 {
                    zlepseno = true;
                    break;
                }
                st.presun(u, puvodni);
            }
        }
        // dočištění – nejlepší zlepšující tah pro každou jednotku
        for u in 0..m.n_u {
            let su = st.start[u];
            if m.allowed[u].count_ones() <= 1 {
                continue;
            }
            let mut nej = (0i64, su);
            for c in bity(m.allowed[u]) {
                if c == su {
                    continue;
                }
                let d = st.presun(u, c);
                st.presun(u, su);
                if d < nej.0 {
                    nej = (d, c);
                }
            }
            if nej.1 != su {
                st.presun(u, nej.1);
                zlepseno = true;
            }
        }
        if !zlepseno {
            break;
        }
    }
}

/// Hlavní vstup řešiče.
pub fn generuj(
    skola: &Skola,
    rok: &SkolniRok,
    piny: &BTreeMap<String, u8>,
    stop: &AtomicBool,
    prubeh: Option<&Mutex<Prubeh>>,
) -> Vysledek {
    let t0 = Instant::now();
    let vstup = postav_lekce(skola, rok);
    let mut varovani = vstup.varovani.clone();
    let model = Model::postav(skola, &vstup, piny, &mut varovani);
    let mut st = Stav::new(&model);
    let mut rng = StdRng::seed_from_u64(skola.nastaveni.seed);
    if let Some(p) = prubeh {
        if let Ok(mut p) = p.lock() {
            p.faze = "greedy start".into();
        }
    }
    greedy(&mut st, &mut rng);
    hlas(prubeh, "žíhání", 0, &st, t0);
    let limit = skola.nastaveni.casovy_limit_s.max(1) as f64;
    let (best, iteraci) = zihani(&mut st, &mut rng, limit, stop, prubeh, t0);
    st.nastav(&best);
    hlas(prubeh, "slepování bloků", iteraci, &st, t0);
    slep_bloky(&mut st);
    hlas(prubeh, "místnosti", iteraci, &st, t0);

    let n = vstup.lekce.len();
    let slot: Vec<Option<u8>> = (0..n)
        .map(|i| {
            let s = st.start[vstup.lekce[i].jednotka];
            (s != NENI).then_some(s)
        })
        .collect();
    let mut v = Vysledek {
        rok: rok.label.clone(),
        lekce: vstup.lekce,
        slot,
        mistnost: vec![None; n],
        varovani,
        problemy: vec![],
        problemove_lekce: vec![],
        skore: (st.hard, st.soft),
        iteraci,
        cas_s: 0.0,
        limity: model.limity.clone(),
        zaci_tridy: vstup.zaci_tridy,
        virtualni_zaci: vstup.virtualni,
    };
    prirad_mistnosti(skola, &mut v, None);
    zkontroluj_a_zapis(skola, &mut v);
    v.cas_s = t0.elapsed().as_secs_f32();
    hlas(prubeh, "hotovo", iteraci, &st, t0);
    v
}

/// Spustí plnou validaci a zapíše problémy do výsledku (po generování i po ručních úpravách).
pub fn zkontroluj_a_zapis(skola: &Skola, v: &mut Vysledek) {
    let k = validation::zkontroluj(skola, v);
    v.problemy = k.problemy;
    v.problemove_lekce = k.problemove_lekce.into_iter().collect();
    v.varovani.retain(|x| !x.starts_with("Kapacita:") && !x.starts_with("Místnost:"));
    v.varovani.extend(k.varovani);
}

// ───────────────────────────── místnosti ─────────────────────────────

/// Povolené místnosti lekce v pořadí preference:
/// preferovaná učitelem ➡ odborné ➡ kmenová třídy ➡ ostatní kmenové (přeliv).
pub fn povolene_mistnosti(skola: &Skola, l: &LekceInfo) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let kmenove_trid: Vec<String> = l.tridy.iter().filter_map(|t| skola.trida(t).map(|t| t.kmenova.clone())).collect();
    if let Some(pref) = skola.ucitel(&l.ucitel).and_then(|u| u.preferovana_mistnost.clone()) {
        let je_kmenova = skola.mistnost(&pref).is_some_and(|m| m.kmenova);
        if l.kandidati.contains(&pref) || (l.kmenova_ok && je_kmenova) {
            out.push(pref);
        }
    }
    for k in &l.kandidati {
        if !out.contains(k) {
            out.push(k.clone());
        }
    }
    if l.kmenova_ok || l.kandidati.is_empty() {
        for k in kmenove_trid {
            if !out.contains(&k) {
                out.push(k);
            }
        }
        for m in skola.mistnosti.iter().filter(|m| m.kmenova) {
            if !out.contains(&m.id) {
                out.push(m.id.clone());
            }
        }
    }
    out
}

type Obsazeni = HashMap<(String, usize, usize), usize>;

fn sloty_lekce(v: &Vysledek, i: usize) -> Vec<(usize, usize)> {
    let Some(s) = v.slot[i] else { return vec![] };
    let mut out = vec![];
    for &w in v.lekce[i].tyden.tydny() {
        for t in s as usize..s as usize + v.lekce[i].len as usize {
            out.push((w, t));
        }
    }
    out
}

fn volna(obs: &Obsazeni, r: &str, sl: &[(usize, usize)]) -> bool {
    sl.iter().all(|&(w, t)| !obs.contains_key(&(r.to_string(), w, t)))
}

fn obsad(obs: &mut Obsazeni, r: &str, sl: &[(usize, usize)], i: usize) {
    for &(w, t) in sl {
        obs.insert((r.to_string(), w, t), i);
    }
}

fn uvolni(obs: &mut Obsazeni, r: &str, sl: &[(usize, usize)]) {
    for &(w, t) in sl {
        obs.remove(&(r.to_string(), w, t));
    }
}

/// Greedy přiřazení místností s opravou konfliktů (jednoúrovňová výměna).
/// `jen` = přiřadit jen tyto lekce (ostatní ponechat) – po ručním přesunu.
pub fn prirad_mistnosti(skola: &Skola, v: &mut Vysledek, jen: Option<&BTreeSet<usize>>) {
    let n = v.lekce.len();
    let mut obs: Obsazeni = HashMap::new();
    let mut prirazovat: Vec<usize> = Vec::new();
    for i in 0..n {
        let znovu = jen.map_or(true, |j| j.contains(&i)) || v.mistnost[i].is_none();
        if znovu {
            v.mistnost[i] = None;
            prirazovat.push(i);
        } else if let Some(r) = v.mistnost[i].clone() {
            let sl = sloty_lekce(v, i);
            obsad(&mut obs, &r, &sl, i);
        }
    }
    let povolene: HashMap<usize, Vec<String>> =
        prirazovat.iter().map(|&i| (i, povolene_mistnosti(skola, &v.lekce[i]))).collect();
    prirazovat.sort_by_key(|&i| {
        let l = &v.lekce[i];
        (l.kmenova_ok, povolene[&i].len(), !l.skupina.is_empty(), Reverse(l.zaci.len()))
    });
    let kapacita = |r: &str| skola.mistnost(r).map_or(0, |m| m.kapacita as usize);
    for &i in &prirazovat {
        if v.slot[i].is_none() {
            continue;
        }
        let sl = sloty_lekce(v, i);
        let velikost = v.pocet_realnych_zaku(i);
        let pov = &povolene[&i];
        let vyber = pov
            .iter()
            .find(|r| volna(&obs, r, &sl) && kapacita(r) >= velikost)
            .or_else(|| pov.iter().find(|r| volna(&obs, r, &sl)))
            .cloned();
        if let Some(r) = vyber {
            obsad(&mut obs, &r, &sl, i);
            v.mistnost[i] = Some(r);
            continue;
        }
        // výměna: uvolnit místnost jiné lekci, která má jinou volnou alternativu
        'hledani: for r in pov {
            let drzitele: BTreeSet<usize> =
                sl.iter().filter_map(|&(w, t)| obs.get(&(r.clone(), w, t)).copied()).collect();
            if drzitele.len() != 1 {
                continue;
            }
            let j = *drzitele.iter().next().unwrap();
            if !prirazovat.contains(&j) {
                continue;
            }
            let slj = sloty_lekce(v, j);
            uvolni(&mut obs, r, &slj);
            if volna(&obs, r, &sl) {
                let alt = povolene_mistnosti(skola, &v.lekce[j]).into_iter().find(|x| x != r && volna(&obs, x, &slj));
                if let Some(x) = alt {
                    obsad(&mut obs, &x, &slj, j);
                    v.mistnost[j] = Some(x);
                    obsad(&mut obs, r, &sl, i);
                    v.mistnost[i] = Some(r.clone());
                    break 'hledani;
                }
            }
            obsad(&mut obs, r, &slj, j);
        }
    }
}
