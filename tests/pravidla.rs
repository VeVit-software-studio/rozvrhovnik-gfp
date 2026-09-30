//! Pravidla dne žáka: neslučitelné předměty (TV × PL), krátké dny, dlouhá okna, jídelna.

use rozvrhovnik::data::*;
use rozvrhovnik::jidelna;
use rozvrhovnik::profily;
use rozvrhovnik::solver::{self, Vysledek};
use rozvrhovnik::validation;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::atomic::AtomicBool;

fn generuj(skola: &Skola, rok: &SkolniRok, sekund: u32) -> Vysledek {
    let mut skola = skola.clone();
    skola.nastaveni.casovy_limit_s = sekund;
    solver::generuj(&skola, rok, &BTreeMap::new(), &AtomicBool::new(false), None)
}

/// Dny, kdy má třída lekci předmětu.
fn dny_predmetu(v: &Vysledek, trida: &str, predmet: &str) -> BTreeSet<usize> {
    (0..v.lekce.len())
        .filter(|&i| v.lekce[i].predmet == predmet && v.lekce[i].tridy.iter().any(|t| t == trida))
        .filter_map(|i| v.slot[i].map(|s| s as usize / SLOTU))
        .collect()
}

fn den(hodiny: &[usize]) -> [bool; SLOTU] {
    let mut d = [false; SLOTU];
    for &h in hodiny {
        d[h] = true;
    }
    d
}

#[test]
fn vlny_obedu_a_polední_pauza() {
    // vyučování 8:10–11:40 ➡ oběd v 11:40
    assert_eq!(jidelna::vlna(&den(&[1, 2, 3, 4])), Some(0));
    // do 13:40 ➡ oběd po 7. hodině (13:40)
    assert_eq!(jidelna::vlna(&den(&[1, 2, 3, 4, 5, 6])), Some(2));
    assert_eq!(jidelna::cas_vlny(2), "13:40");
    // volná 6. hodina (12:00) a odpoledne vyučování ➡ oběd 11:40, 12:00 je pauza, ne okno
    let d = den(&[1, 2, 3, 4, 6, 7]);
    assert_eq!(jidelna::vlna(&d), Some(0));
    assert_eq!(jidelna::obedova_pauza(&d), Some(5));
    // bez volné hodiny až do 14:30 ➡ jen přestávka v 11:40
    assert_eq!(jidelna::vlna(&den(&[2, 3, 4, 5, 6, 7, 8])), Some(0));
    assert_eq!(jidelna::obedova_pauza(&den(&[2, 3, 4, 5, 6, 7, 8])), None);
    // ve škole není
    assert_eq!(jidelna::vlna(&[false; SLOTU]), None);
    // okno dopoledne není polední pauza
    assert_eq!(jidelna::obedova_pauza(&den(&[1, 3, 4, 5])), None);
}

#[test]
fn tv_a_plavani_nejsou_ve_stejny_den_a_validace_to_hlida() {
    let p = vychozi();
    assert!(p.skola.plan.iter().any(|h| h.predmet == "PL"));
    let mut v = generuj(&p.skola, &p.rok, 12);
    assert!(v.problemy.is_empty(), "{:?}", &v.problemy[..v.problemy.len().min(5)]);
    let tridy_s_pl: BTreeSet<&str> =
        p.skola.plan.iter().filter(|h| h.predmet == "PL").map(|h| h.trida.as_str()).collect();
    for t in &tridy_s_pl {
        let tv = dny_predmetu(&v, t, "TV");
        let pl = dny_predmetu(&v, t, "PL");
        assert!(!pl.is_empty());
        assert!(tv.is_disjoint(&pl), "{t}: TV {tv:?}, PL {pl:?}");
    }
    // ruční přesun PL na den s TV ➡ tvrdý problém
    let t = tridy_s_pl.iter().next().unwrap();
    let tv_den = *dny_predmetu(&v, t, "TV").iter().next().unwrap();
    for i in 0..v.lekce.len() {
        if v.lekce[i].predmet == "PL" && v.lekce[i].tridy.iter().any(|x| x == t) {
            v.slot[i] = Some((tv_den * SLOTU + 11) as u8);
        }
    }
    let k = validation::zkontroluj(&p.skola, &v);
    assert!(k.problemy.iter().any(|x| x.contains("TV a PL ve stejný den")), "{:?}", k.problemy);
    // bez pravidla se nehlásí
    let mut skola = p.skola.clone();
    skola.nastaveni.neslucitelne.clear();
    let k = validation::zkontroluj(&skola, &v);
    assert!(!k.problemy.iter().any(|x| x.contains("ve stejný den")));
}

#[test]
fn jidelna_rozlozi_obedy_do_vln() {
    let p = vychozi();
    let preplneni = |skola: &Skola, v: &Vysledek, kap: u32| -> u32 {
        let mut s = skola.clone();
        s.nastaveni.jidelna_kapacita = kap;
        let j = jidelna::spocitej(&s, v);
        (0..DNY).flat_map(|d| j.stravniku[d].iter().map(|&x| x.saturating_sub(kap)).collect::<Vec<_>>()).sum()
    };
    let mut bez = p.skola.clone();
    bez.nastaveni.jidelna_kapacita = 0;
    let v0 = generuj(&bez, &p.rok, 10);
    let kap = 110;
    let mut s = p.skola.clone();
    s.nastaveni.jidelna_kapacita = kap;
    let v1 = generuj(&s, &p.rok, 10);
    assert!(v1.problemy.is_empty(), "{:?}", &v1.problemy[..v1.problemy.len().min(5)]);
    let (a, b) = (preplneni(&s, &v0, kap), preplneni(&s, &v1, kap));
    assert!(b < a, "s hlídáním jídelny má být méně strávníků nad kapacitu: bez {a}, s {b}");
    // report: součet vln = žáci ve škole v daný den
    let j = jidelna::spocitej(&s, &v1);
    let zaku = v1.zaci_tridy.len() as u32;
    for d in 0..DNY {
        assert_eq!(j.zaku[d].iter().sum::<u32>(), zaku, "den {d}");
    }
    assert_eq!(j.tridy.len(), p.skola.tridy.len());
}

#[test]
fn kratke_dny_a_dlouha_okna_se_hlasi() {
    let p = vychozi();
    let mut v = generuj(&p.skola, &p.rok, 5);
    // přesun všech hodin jednoho dne primy kromě jedné na pátek 11. hodinu nelze – místo toho
    // posuneme jednu hodinu primy na konec dne ➡ dlouhé okno
    let i = (0..v.lekce.len())
        .find(|&i| v.lekce[i].tridy == ["prima"] && v.lekce[i].len == 1 && v.lekce[i].par.is_none())
        .unwrap();
    let d = v.slot[i].unwrap() as usize / SLOTU;
    let posledni = (0..v.lekce.len())
        .filter(|&j| j != i && v.lekce[j].tridy.iter().any(|t| t == "prima"))
        .filter_map(|j| v.slot[j].map(|s| s as usize))
        .filter(|&s| s / SLOTU == d)
        .map(|s| s % SLOTU)
        .max()
        .unwrap();
    assert!(posledni + 4 < SLOTU);
    v.slot[i] = Some((d * SLOTU + posledni + 4) as u8);
    let k = validation::zkontroluj(&p.skola, &v);
    assert!(k.varovani.iter().any(|x| x.starts_with("Dlouhé okno (3+ h): prima")), "{:?}", k.varovani);
}

#[test]
fn profil_ucitele_deli_hodiny_volby_podle_ucitelu() {
    let mut p = vychozi();
    let m = p.rok.menu.iter_mut().find(|m| m.id == "3j-5").unwrap();
    assert!(!m.dvojhodina && m.hodin_tydne == 3);
    let nj = m.volby.iter_mut().find(|v| v.id == "NJ2").unwrap();
    nj.ucitel = "UHR".into();
    nj.ucitele_hodin = vec!["UHR".into(), "UHR".into(), "KOŠ".into()];
    let vy = profily::vyuka(&p.skola, &p.rok);
    let nj: Vec<(&str, f32)> = vy
        .iter()
        .filter(|x| x.predmet == "NJ2" && x.menu.as_deref() == Some("3. jazyk – 5. ročník (8leté)"))
        .map(|x| (x.ucitel.as_str(), x.hodin))
        .collect();
    assert_eq!(nj, [("UHR", 2.0), ("KOŠ", 1.0)]);
}
