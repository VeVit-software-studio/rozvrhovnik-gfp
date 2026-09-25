//! Plná validace rozvrhu – nezávislá na řešiči, počítá se přesně pro každého žáka
//! a každý týden (lichý/sudý). Volá se po generování i po každé ruční úpravě.

use crate::data::*;
use crate::solver::{popis_lekce, povolene_mistnosti, Vysledek};
use std::collections::{BTreeMap, BTreeSet, HashMap};

#[derive(Clone, Debug, Default)]
pub struct StatTridy {
    pub trida: String,
    pub zaku: usize,
    pub hodin_min: usize,
    pub hodin_max: usize,
    pub rane_dny: usize,
    pub odpoledni: usize,
    pub nejdelsi_blok: usize,
    pub okna_prumer: f32,
}

#[derive(Clone, Debug, Default)]
pub struct Kontrola {
    /// Tvrdé problémy (kolize, 7 h, limity, maxima…).
    pub problemy: Vec<String>,
    /// Měkká upozornění (kapacity místností).
    pub varovani: Vec<String>,
    pub problemove_lekce: BTreeSet<usize>,
    pub statistiky: Vec<StatTridy>,
}

fn kdy(t: usize) -> String {
    let (d, k) = den_a_slot(t);
    format!("{} {}. hod ({})", NAZVY_DNU[d], k, CASY[k])
}

fn tyden_nazev(w: usize) -> &'static str {
    if w == 0 {
        "L"
    } else {
        "S"
    }
}

pub fn zkontroluj(skola: &Skola, v: &Vysledek) -> Kontrola {
    let mut k = Kontrola::default();
    let n = v.lekce.len();
    let odpo_od = (skola.nastaveni.odpoledne_od as usize).clamp(1, SLOTU);
    let za_den = skola.nastaveni.limity_za_den;

    // 1. neumístěné lekce
    for i in 0..n {
        if v.slot[i].is_none() {
            k.problemy.push(format!("Neumístěná lekce: {}", popis_lekce(&v.lekce[i])));
            k.problemove_lekce.insert(i);
        }
    }

    // 2. obsazení (týden, slot)
    let mut uc: HashMap<(&str, usize, usize), Vec<usize>> = HashMap::new();
    let mut mi: HashMap<(&str, usize, usize), Vec<usize>> = HashMap::new();
    let mut zk: HashMap<(u32, usize, usize), Vec<usize>> = HashMap::new();
    for i in 0..n {
        let Some(s) = v.slot[i] else { continue };
        let l = &v.lekce[i];
        for &w in l.tyden.tydny() {
            for t in s as usize..s as usize + l.len as usize {
                uc.entry((l.ucitel.as_str(), w, t)).or_default().push(i);
                if let Some(r) = &v.mistnost[i] {
                    mi.entry((r.as_str(), w, t)).or_default().push(i);
                }
                for &z in &l.zaci {
                    zk.entry((z, w, t)).or_default().push(i);
                }
            }
        }
    }
    let mut hlaseno: BTreeSet<(u8, usize, usize)> = BTreeSet::new();
    let mut kolize = |druh: u8, co: &str, ls: &[usize], t: usize, k: &mut Kontrola, pocet: Option<usize>| {
        for x in 0..ls.len() {
            for y in x + 1..ls.len() {
                let (a, b) = (ls[x].min(ls[y]), ls[x].max(ls[y]));
                if a == b || !hlaseno.insert((druh, a, b)) {
                    continue;
                }
                let zaku = pocet.map(|p| format!(" ({} žáků)", p)).unwrap_or_default();
                k.problemy.push(format!(
                    "Kolize {}: {} × {} – {}{}",
                    co,
                    popis_lekce(&v.lekce[a]),
                    popis_lekce(&v.lekce[b]),
                    kdy(t),
                    zaku
                ));
                k.problemove_lekce.insert(a);
                k.problemove_lekce.insert(b);
            }
        }
    };
    let mut klice: Vec<_> = uc.iter().filter(|(_, ls)| ls.len() > 1).collect();
    klice.sort_by_key(|(k, _)| (k.2, k.0, k.1));
    for ((u, _, t), ls) in klice {
        kolize(0, &format!("učitele {}", u), ls, *t, &mut k, None);
    }
    let mut klice: Vec<_> = mi.iter().filter(|(_, ls)| ls.len() > 1).collect();
    klice.sort_by_key(|(k, _)| (k.2, k.0, k.1));
    for ((r, _, t), ls) in klice {
        kolize(1, &format!("místnosti {}", r), ls, *t, &mut k, None);
    }
    // žáci: agregace po dvojicích lekcí
    let mut par_zaku: BTreeMap<(usize, usize), (usize, BTreeSet<u32>)> = BTreeMap::new();
    for ((z, _, t), ls) in &zk {
        for x in 0..ls.len() {
            for y in x + 1..ls.len() {
                let (a, b) = (ls[x].min(ls[y]), ls[x].max(ls[y]));
                let e = par_zaku.entry((a, b)).or_insert((*t, BTreeSet::new()));
                e.0 = e.0.min(*t);
                e.1.insert(*z);
            }
        }
    }
    for ((a, b), (t, zaci)) in par_zaku {
        kolize(2, "žáků", &[a, b], t, &mut k, Some(zaci.len()));
    }

    // 3. per žák a týden: pravidlo 7 h, max 8 h/den, limity P1
    let mut poruseni: BTreeMap<(String, String, usize), BTreeSet<u32>> = BTreeMap::new();
    let mut zak_lekce: HashMap<u32, Vec<usize>> = HashMap::new();
    for i in 0..n {
        for &z in &v.lekce[i].zaci {
            zak_lekce.entry(z).or_default().push(i);
        }
    }
    let mut stat_zaka: BTreeMap<String, Vec<(usize, usize, usize, usize, usize)>> = BTreeMap::new();
    for (z, tr) in &v.zaci_tridy {
        let lim = v.limity.get(tr).copied().unwrap_or_else(|| skola.trida(tr).map(|t| t.omezeni()).unwrap_or_default());
        let ls = zak_lekce.get(z).cloned().unwrap_or_default();
        let mut occ = [[false; N_SLOTU]; 2];
        for &i in &ls {
            if let Some(s) = v.slot[i] {
                for &w in v.lekce[i].tyden.tydny() {
                    for t in s as usize..s as usize + v.lekce[i].len as usize {
                        occ[w][t] = true;
                    }
                }
            }
        }
        let mut max_blok = 0;
        for (w, occ_w) in occ.iter().enumerate() {
            let mut rane_t = 0;
            let mut odpo_t = 0;
            for d in 0..DNY {
                let den = &occ_w[d * SLOTU..(d + 1) * SLOTU];
                let pocet = den.iter().filter(|&&x| x).count();
                let mut run = 0;
                let mut nejdelsi = 0;
                for &x in den {
                    run = if x { run + 1 } else { 0 };
                    nejdelsi = nejdelsi.max(run);
                }
                max_blok = max_blok.max(nejdelsi);
                let rane = den[0] as usize;
                let odpo = den[odpo_od..].iter().filter(|&&x| x).count();
                rane_t += rane;
                odpo_t += odpo;
                let mut zapis = |druh: String| {
                    poruseni.entry((tr.clone(), druh, d)).or_default().insert(*z);
                };
                if nejdelsi > MAX_RUN {
                    zapis(format!("pravidlo 7 h ({} h v kuse, {})", nejdelsi, tyden_nazev(w)));
                }
                if pocet > MAX_DEN_TRIDA {
                    zapis(format!("více než {} h za den ({} h, {})", MAX_DEN_TRIDA, pocet, tyden_nazev(w)));
                }
                if za_den {
                    if lim.rane.is_some_and(|l| rane > l as usize) {
                        zapis("limit raných hodin".into());
                    }
                    if lim.odpoledni.is_some_and(|l| odpo > l as usize) {
                        zapis(format!("limit odpoledních hodin ({} > {})", odpo, lim.odpoledni.unwrap()));
                    }
                }
            }
            if !za_den {
                if lim.rane.is_some_and(|l| rane_t > l as usize) {
                    poruseni
                        .entry((tr.clone(), format!("týdenní limit raných ({})", tyden_nazev(w)), 99))
                        .or_default()
                        .insert(*z);
                }
                if lim.odpoledni.is_some_and(|l| odpo_t > l as usize) {
                    poruseni
                        .entry((tr.clone(), format!("týdenní limit odpoledních ({})", tyden_nazev(w)), 99))
                        .or_default()
                        .insert(*z);
                }
            }
        }
        // statistiky (sjednocení obou týdnů)
        let mut hodin = 0;
        let mut rane_dny = 0;
        let mut odpo = 0;
        let mut okna = 0;
        for d in 0..DNY {
            let den: Vec<bool> = (0..SLOTU).map(|x| occ[0][d * SLOTU + x] || occ[1][d * SLOTU + x]).collect();
            let c = den.iter().filter(|&&x| x).count();
            hodin += c;
            rane_dny += den[0] as usize;
            odpo += den[odpo_od..].iter().filter(|&&x| x).count();
            if let (Some(f), Some(l)) = (den.iter().position(|&x| x), den.iter().rposition(|&x| x)) {
                okna += l - f + 1 - c;
            }
        }
        stat_zaka.entry(tr.clone()).or_default().push((hodin, rane_dny, odpo, max_blok, okna));
    }
    for ((tr, druh, d), zaci) in &poruseni {
        let den = if *d < DNY { NAZVY_DNU[*d] } else { "týden" };
        let virt = zaci.iter().all(|z| v.virtualni_zaci.contains(z));
        let kdo = if virt { "třída (zástupní žáci)".to_string() } else { format!("{} žáků", zaci.len()) };
        k.problemy.push(format!("{}: {} – {}, {}", skola.nazev_tridy(tr), druh, den, kdo));
        for z in zaci {
            for &i in zak_lekce.get(z).into_iter().flatten() {
                if let Some(s) = v.slot[i] {
                    if *d >= DNY || s as usize / SLOTU == *d {
                        k.problemove_lekce.insert(i);
                    }
                }
            }
        }
    }

    // 4. učitelé: max hodin za den, volno
    let mut uc_occ: BTreeMap<&str, [bool; N_SLOTU]> = BTreeMap::new();
    for i in 0..n {
        let Some(s) = v.slot[i] else { continue };
        let l = &v.lekce[i];
        let o = uc_occ.entry(l.ucitel.as_str()).or_insert([false; N_SLOTU]);
        for t in s as usize..s as usize + l.len as usize {
            o[t] = true;
            if skola.ucitel(&l.ucitel).is_some_and(|u| u.volno.contains(&(t as u8))) {
                k.problemy.push(format!("Učitel {} má volno: {} – {}", l.ucitel, popis_lekce(l), kdy(t)));
                k.problemove_lekce.insert(i);
            }
        }
    }
    for (u, o) in &uc_occ {
        let max = skola.ucitel(u).map_or(8, |x| x.max_den as usize);
        for d in 0..DNY {
            let c = o[d * SLOTU..(d + 1) * SLOTU].iter().filter(|&&x| x).count();
            if c > max {
                k.problemy.push(format!("Učitel {} má {} h v {} (max {}).", u, c, NAZVY_DNU[d], max));
                for i in
                    (0..n).filter(|&i| v.lekce[i].ucitel == *u && v.slot[i].is_some_and(|s| s as usize / SLOTU == d))
                {
                    k.problemove_lekce.insert(i);
                }
            }
        }
    }

    // 5. max 2 h stejného předmětu a skupiny za den
    let mut kl: BTreeMap<&str, [bool; N_SLOTU]> = BTreeMap::new();
    for i in 0..n {
        let Some(s) = v.slot[i] else { continue };
        let l = &v.lekce[i];
        let o = kl.entry(l.klic_predmetu.as_str()).or_insert([false; N_SLOTU]);
        for t in s as usize..s as usize + l.len as usize {
            o[t] = true;
        }
    }
    for (klic, o) in &kl {
        for d in 0..DNY {
            let c = o[d * SLOTU..(d + 1) * SLOTU].iter().filter(|&&x| x).count();
            if c > MAX_STEJNY_PREDMET_DEN {
                let ukazka = (0..n).find(|&i| v.lekce[i].klic_predmetu == *klic).unwrap();
                k.problemy.push(format!(
                    "Více než {} h stejného předmětu za den: {} – {} ({} h).",
                    MAX_STEJNY_PREDMET_DEN,
                    popis_lekce(&v.lekce[ukazka]),
                    NAZVY_DNU[d],
                    c
                ));
                for i in (0..n).filter(|&i| {
                    v.lekce[i].klic_predmetu == *klic && v.slot[i].is_some_and(|s| s as usize / SLOTU == d)
                }) {
                    k.problemove_lekce.insert(i);
                }
            }
        }
    }

    // 6. místnosti
    for i in 0..n {
        if v.slot[i].is_none() {
            continue;
        }
        let l = &v.lekce[i];
        match &v.mistnost[i] {
            None => {
                k.problemy.push(format!(
                    "Bez vhodné místnosti: {} – {}",
                    popis_lekce(l),
                    kdy(v.slot[i].unwrap() as usize)
                ));
                k.problemove_lekce.insert(i);
            }
            Some(r) => {
                if !povolene_mistnosti(skola, l).contains(r) {
                    k.problemy.push(format!("Nevhodná místnost {} pro {}", r, popis_lekce(l)));
                    k.problemove_lekce.insert(i);
                }
                let kap = skola.mistnost(r).map_or(0, |m| m.kapacita as usize);
                let zaku = v.pocet_realnych_zaku(i);
                if zaku > kap {
                    k.varovani.push(format!(
                        "Kapacita: {} ({} žáků) v {} (kapacita {}).",
                        popis_lekce(l),
                        zaku,
                        r,
                        kap
                    ));
                }
            }
        }
    }

    // 7. statistiky tříd
    for t in &skola.tridy {
        let Some(zaci) = stat_zaka.get(&t.id) else { continue };
        let virt = v.zaci_tridy.iter().filter(|(z, tr)| **tr == t.id && v.virtualni_zaci.contains(z)).count();
        k.statistiky.push(StatTridy {
            trida: t.id.clone(),
            zaku: zaci.len() - virt,
            hodin_min: zaci.iter().map(|x| x.0).min().unwrap_or(0),
            hodin_max: zaci.iter().map(|x| x.0).max().unwrap_or(0),
            rane_dny: zaci.iter().map(|x| x.1).max().unwrap_or(0),
            odpoledni: zaci.iter().map(|x| x.2).max().unwrap_or(0),
            nejdelsi_blok: zaci.iter().map(|x| x.3).max().unwrap_or(0),
            okna_prumer: zaci.iter().map(|x| x.4 as f32).sum::<f32>() / zaci.len().max(1) as f32,
        });
    }
    k
}
