//! Jídelna: kdy žáci přijdou na oběd a kolik jich přijde najednou.
//!
//! Žák jde na oběd po první hodině od 11:40 dál, po které má volno (konec vyučování nebo
//! volná hodina). Kdo má vyučování bez přestávky až do 14:30, stihne oběd jen o velké
//! přestávce v 11:40. Když všichni končí dopoledne v 11:40, jídelna se ucpe – řešič proto
//! (podle kapacity v Nastavení) posouvá část tříd na pozdější vlny.

use crate::data::*;
use crate::solver::Vysledek;
use std::collections::{BTreeMap, HashMap};

/// Počet vln obědů.
pub const VLNY: usize = 4;
/// Hodina (index slotu), po jejímž konci žák přijde do jídelny: 5., 6., 7. a 8. hodina.
pub const PO_HODINE: [usize; VLNY] = [4, 5, 6, 7];

/// Čas příchodu vlny (konec hodiny), např. „11:40“.
pub fn cas_vlny(w: usize) -> &'static str {
    CASY_KONEC[PO_HODINE[w]]
}

/// Vlna, ve které přijde na oběd žák s daným dnem (obsazené hodiny), nebo `None`, když ten den
/// ve škole není.
pub fn vlna(den: &[bool]) -> Option<usize> {
    if !den.iter().any(|&x| x) {
        return None;
    }
    // po hodině s přijde, když hodina s + 1 je volná (volná hodina nebo konec vyučování)
    Some(PO_HODINE.iter().position(|&s| !den[s + 1]).unwrap_or(0))
}

/// Polední pauza: volná hodina, ve které žák obědvá, když po ní ještě má vyučování.
/// Taková hodina se nepočítá jako okno.
pub fn obedova_pauza(den: &[bool]) -> Option<usize> {
    let w = vlna(den)?;
    let s = PO_HODINE[w] + 1;
    (!den[s] && den[s + 1..].iter().any(|&x| x) && den[..s].iter().any(|&x| x)).then_some(s)
}

/// Odhad zatížení jídelny v hotovém rozvrhu.
#[derive(Clone, Debug, Default)]
pub struct Jidelna {
    /// Počet žáků ve škole, kteří přijdou v dané vlně (bez ohledu na podíl strávníků).
    pub zaku: [[u32; VLNY]; DNY],
    /// Odhad strávníků = žáci × podíl strávníků.
    pub stravniku: [[u32; VLNY]; DNY],
    pub kapacita: u32,
    /// Pro každou třídu a den vlna, ve které přijde většina třídy.
    pub tridy: Vec<(String, [Option<usize>; DNY])>,
    /// Žáci bez polední pauzy (vyučování 11:40–14:30 v kuse) – počet za den.
    pub bez_pauzy: [u32; DNY],
}

impl Jidelna {
    pub fn pres_kapacitu(&self, d: usize, w: usize) -> bool {
        self.kapacita > 0 && self.stravniku[d][w] > self.kapacita
    }
}

/// Spočítá, kolik žáků kdy přijde na oběd (sjednocení lichého a sudého týdne).
pub fn spocitej(skola: &Skola, v: &Vysledek) -> Jidelna {
    let n = &skola.nastaveni;
    let mut j = Jidelna { kapacita: n.jidelna_kapacita, ..Default::default() };
    let mut occ: HashMap<u32, [bool; N_SLOTU]> = HashMap::new();
    for (i, l) in v.lekce.iter().enumerate() {
        let Some(s) = v.slot[i] else { continue };
        for &z in &l.zaci {
            let o = occ.entry(z).or_insert([false; N_SLOTU]);
            for t in s as usize..(s as usize + l.len as usize).min(N_SLOTU) {
                o[t] = true;
            }
        }
    }
    let mut po_tridach: BTreeMap<&str, [[u32; VLNY]; DNY]> = BTreeMap::new();
    for (z, tr) in &v.zaci_tridy {
        let Some(o) = occ.get(z) else { continue };
        for d in 0..DNY {
            let den = &o[d * SLOTU..(d + 1) * SLOTU];
            let Some(w) = vlna(den) else { continue };
            j.zaku[d][w] += 1;
            po_tridach.entry(tr.as_str()).or_insert([[0; VLNY]; DNY])[d][w] += 1;
            if PO_HODINE.iter().all(|&s| den[s + 1]) {
                j.bez_pauzy[d] += 1;
            }
        }
    }
    for d in 0..DNY {
        for w in 0..VLNY {
            j.stravniku[d][w] = (j.zaku[d][w] * n.jidelna_podil as u32 + 50) / 100;
        }
    }
    for t in &skola.tridy {
        let Some(p) = po_tridach.get(t.id.as_str()) else { continue };
        let mut dny = [None; DNY];
        for d in 0..DNY {
            dny[d] = (0..VLNY).filter(|&w| p[d][w] > 0).max_by_key(|&w| (p[d][w], std::cmp::Reverse(w)));
        }
        j.tridy.push((t.id.clone(), dny));
    }
    j
}
