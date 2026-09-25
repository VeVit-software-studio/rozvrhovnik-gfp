//! Testy logiky: rozdělení 15:15, přechod roku, import voleb, validace, piny a řešič.

use rozvrhovnik::data::*;
use rozvrhovnik::rok::{self, Zmena};
use rozvrhovnik::solver::{self, Vysledek};
use rozvrhovnik::validation;
use std::collections::BTreeMap;
use std::sync::atomic::AtomicBool;

fn generuj(p: &Projekt, sekund: u32) -> Vysledek {
    let mut skola = p.skola.clone();
    skola.nastaveni.casovy_limit_s = sekund;
    solver::generuj(&skola, &p.rok, &p.piny, &AtomicBool::new(false), None)
}

#[test]
fn rozdeleni_1515_je_vyrovnane() {
    let mut zaci = Vec::new();
    for i in 0..17 {
        zaci.push((format!("Dívka {i:02}"), Pohlavi::D));
    }
    for i in 0..13 {
        zaci.push((format!("Chlapec {i:02}"), Pohlavi::CH));
    }
    let sk = rok::rozdel_1515(&zaci);
    let a = sk.iter().filter(|&&g| g == SkupinaAj::A).count();
    assert_eq!(a, 15);
    let d_a = zaci.iter().zip(&sk).filter(|(z, g)| z.1 == Pohlavi::D && **g == SkupinaAj::A).count();
    assert!((8..=9).contains(&d_a), "dívky v AJa: {d_a}");
    // lichý počet: rozdíl nejvýš 1
    let sk = rok::rozdel_1515(&zaci[..29]);
    let a = sk.iter().filter(|&&g| g == SkupinaAj::A).count();
    assert!(a == 14 || a == 15);
}

#[test]
fn odhad_pohlavi_a_parsovani() {
    assert_eq!(rok::odhad_pohlavi("Jana Nováková"), Pohlavi::D);
    assert_eq!(rok::odhad_pohlavi("Petr Svoboda"), Pohlavi::CH);
    assert_eq!(rok::odhad_pohlavi("Tereza Malá"), Pohlavi::D);
    assert_eq!(rok::odhad_pohlavi("Jiří Černý"), Pohlavi::CH);
    assert_eq!(rok::odhad_pohlavi("Nikola Veselý"), Pohlavi::CH);
    let z = rok::parsuj_jmena("Jana Nováková\n\n  Petr Svoboda  \nAlex Kovář; D\nKim Lee;CH\n");
    assert_eq!(z.len(), 4);
    assert_eq!(z[1].0, "Petr Svoboda");
    assert_eq!(z[2], ("Alex Kovář".to_string(), Pohlavi::D));
    assert_eq!(z[3].1, Pohlavi::CH);
}

#[test]
fn seed_je_konzistentni() {
    let p = vychozi();
    assert_eq!(p.skola.tridy.len(), 17);
    assert_eq!(p.skola.ucitele.len(), 42);
    for ph in &p.skola.plan {
        assert!(p.skola.trida(&ph.trida).is_some());
        for u in ph.struktura.ucitele() {
            assert!(p.skola.ucitel(u).is_some(), "neznámý učitel {u}");
        }
    }
    for m in &p.rok.menu {
        for v in &m.volby {
            assert!(p.skola.predmet(&v.predmet).is_some(), "menu {} volba {}", m.id, v.id);
            assert!(p.skola.ucitel(&v.ucitel).is_some());
        }
    }
    assert_eq!(p.skola.vstupni_tridy(), vec!["prima".to_string(), "1A".to_string()]);
    // všichni zástupní žáci mají kompletní volby
    for m in &p.rok.menu {
        for s in p.rok.studenti.iter().filter(|s| m.tridy.contains(&s.trida)) {
            assert_eq!(s.volby.get(&m.id).map_or(0, |v| v.len()), m.pocet_voleb as usize, "{} {}", s.jmeno, m.id);
        }
    }
    // úvazky učitelů jsou rozumné
    let z = solver::zateze(&p.skola, &p.rok);
    assert!(z.iter().all(|(_, h)| *h <= 26.0), "{:?}", &z[..3]);
}

#[test]
fn prechod_do_noveho_roku() {
    let p = vychozi();
    let stary = &p.rok;
    let v_prime = stary.studenti.iter().find(|s| s.trida == "prima").unwrap().clone();
    let propada = stary.studenti.iter().find(|s| s.trida == "sexta A" || s.trida == "sextaA").unwrap().clone();
    let odchazi = stary.studenti.iter().find(|s| s.trida == "tercie").unwrap().clone();
    let maturant = stary.studenti.iter().find(|s| s.trida == "oktavaA").unwrap().clone();
    let mut zmeny = BTreeMap::new();
    zmeny.insert(propada.id, Zmena::Propada);
    zmeny.insert(odchazi.id, Zmena::Odchazi);
    let mut novi = BTreeMap::new();
    novi.insert("prima".to_string(), rok::novi_zaci_z_textu("Jana Nováková\nPetr Svoboda\nEva Malá\nTomáš Černý"));
    let n = rok::novy_rok(&p.skola, stary, &zmeny, &novi, "2027/28");

    let najdi = |id: u32| n.studenti.iter().find(|s| s.id == id);
    assert_eq!(najdi(v_prime.id).unwrap().trida, "sekunda");
    assert_eq!(najdi(propada.id).unwrap().trida, "sextaA");
    assert!(najdi(odchazi.id).is_none());
    assert!(najdi(maturant.id).is_none());
    let prima: Vec<&Student> = n.studenti.iter().filter(|s| s.trida == "prima").collect();
    assert_eq!(prima.len(), 4);
    assert_eq!(prima.iter().filter(|s| s.skupina_aj == SkupinaAj::A).count(), 2);
    // kvarta B nemá předchůdce → prázdná (dobíhající)
    assert!(n.studenti.iter().all(|s| s.trida != "kvartaB"));
    // pokračující 3. jazyk se přenesl (tercie → kvarta A)
    let z_tercie = stary.studenti.iter().find(|s| s.trida == "tercie" && s.id != odchazi.id).unwrap();
    let jazyk = &z_tercie.volby["3j-3"][0];
    assert_eq!(&najdi(z_tercie.id).unwrap().volby["3j-4"][0], jazyk);
    // semináře septimy se do oktávy nepřenášejí
    let sept = stary.studenti.iter().find(|s| s.trida == "septimaA").unwrap();
    assert!(!najdi(sept.id).unwrap().volby.contains_key("sem-8"));
    // ID nových žáků jsou unikátní
    let mut ids: Vec<u32> = n.studenti.iter().map(|s| s.id).collect();
    ids.sort_unstable();
    ids.dedup();
    assert_eq!(ids.len(), n.studenti.len());
}

#[test]
fn rozdeleni_skupiny_a_doplneni_voleb() {
    let mut p = vychozi();
    let menu = p.rok.menu.iter().find(|m| m.id == "sem-7").unwrap().clone();
    let pred = rok::obsazenost(&p.rok, &menu)["SMA"];
    let nova = rok::rozdel_skupinu(&mut p.rok, "sem-7", "SMA", "TES").unwrap();
    let menu = p.rok.menu.iter().find(|m| m.id == "sem-7").unwrap().clone();
    let obs = rok::obsazenost(&p.rok, &menu);
    assert_eq!(obs["SMA"] + obs[&nova], pred);
    assert!(obs[&nova] >= pred / 2);
    // smazat volby a doplnit znovu
    for s in p.rok.studenti.iter_mut() {
        s.volby.remove("vol-5");
    }
    let n = rok::doplnit_volby(&mut p.rok, "vol-5", 3);
    assert!(n > 0);
    let menu = p.rok.menu.iter().find(|m| m.id == "vol-5").unwrap().clone();
    let obs = rok::obsazenost(&p.rok, &menu);
    assert!(obs["VV2"] <= 28, "kapacita VV respektována: {}", obs["VV2"]);
}

#[test]
fn validace_najde_kolize_a_pravidlo_7_hodin() {
    let p = vychozi();
    let mut v = generuj(&p, 3);
    // prima: všechny hodiny prvního dne do jednoho souvislého bloku 1..=8 → porušení 7 h
    let mut idx: Vec<usize> = (0..v.lekce.len())
        .filter(|&i| {
            v.lekce[i].tridy == ["prima"]
                && v.lekce[i].skupina.is_empty()
                && v.lekce[i].len == 1
                && v.lekce[i].tyden == solver::Tyden::Oba
        })
        .collect();
    idx.truncate(8);
    for (k, &i) in idx.iter().enumerate() {
        v.slot[i] = Some(1 + k as u8);
    }
    let k = validation::zkontroluj(&p.skola, &v);
    assert!(k.problemy.iter().any(|x| x.contains("pravidlo 7 h")), "{:#?}", &k.problemy[..k.problemy.len().min(5)]);
    // dvě hodiny téhož učitele ve stejném slotu → kolize učitele
    let a = idx[0];
    let b = (0..v.lekce.len())
        .find(|&i| i != a && v.lekce[i].ucitel == v.lekce[a].ucitel && v.slot[i] != v.slot[a])
        .unwrap();
    v.slot[b] = v.slot[a];
    let k = validation::zkontroluj(&p.skola, &v);
    assert!(k.problemy.iter().any(|x| x.starts_with("Kolize učitele")));
    assert!(k.problemove_lekce.contains(&a) && k.problemove_lekce.contains(&b));
}

#[test]
fn reseni_seedu_bez_tvrdych_problemu() {
    let p = vychozi();
    let v = generuj(&p, 30);
    assert!(v.problemy.is_empty(), "problémy: {:#?}", &v.problemy[..v.problemy.len().min(10)]);
    assert!(v.slot.iter().all(|s| s.is_some()));
    assert!(v.mistnost.iter().all(|m| m.is_some()));
    // auto-uvolnění prima–tercie je nahlášeno
    assert!(v.varovani.iter().any(|x| x.starts_with("AUTO-UVOLNĚNO prima")));
    // L/S páry i dívky/chlapci sdílí slot
    for (i, l) in v.lekce.iter().enumerate() {
        if let Some(p) = l.par {
            assert_eq!(v.slot[i], v.slot[p]);
        }
    }
    // volby „vyber 1“ jsou synchronizované: všechny volby menu 3j-5 ve stejných slotech
    let mut sloty: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    for (i, l) in v.lekce.iter().enumerate() {
        if l.menu.as_deref() == Some("3j-5") {
            sloty.entry(l.skupina.clone()).or_default().push(v.slot[i].unwrap());
        }
    }
    let mut hodnoty: Vec<Vec<u8>> = sloty
        .into_values()
        .map(|mut s| {
            s.sort();
            s
        })
        .collect();
    hodnoty.dedup();
    assert_eq!(hodnoty.len(), 1, "volby 3. jazyka nejsou paralelně");
    // prima–tercie nemají 7:20
    let k = validation::zkontroluj(&p.skola, &v);
    for s in k.statistiky.iter().filter(|s| ["prima", "sekunda", "tercie"].contains(&s.trida.as_str())) {
        assert_eq!(s.rane_dny, 0);
        assert!(s.nejdelsi_blok <= MAX_RUN);
    }
}

#[test]
fn piny_drzi_hodinu_na_miste() {
    let mut p = vychozi();
    let v = generuj(&p, 2);
    let i = v.lekce.iter().position(|l| l.predmet == "MAT" && l.tridy == ["kvintaA"]).unwrap();
    let cil: u8 = 3 * SLOTU as u8 + 2; // čtvrtek 2. hodina
    p.piny.insert(v.lekce[i].klic.clone(), cil);
    p.piny.insert("P|neexistuje|X||0".into(), 5);
    let v2 = generuj(&p, 2);
    let j = v2.lekce.iter().position(|l| l.klic == v.lekce[i].klic).unwrap();
    assert_eq!(v2.slot[j], Some(cil));
    assert!(v2.varovani.iter().any(|x| x.contains("neodpovídá žádné lekci")));
}

#[test]
fn projekt_se_ulozi_a_nacte() {
    let mut p = vychozi();
    p.rozvrh = Some(generuj(&p, 1));
    let dir = std::env::temp_dir().join(format!("rozvrh-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let cesta = dir.join("skola_data.json");
    p.uloz(&cesta).unwrap();
    let q = Projekt::nacti(&cesta).unwrap();
    assert_eq!(q.rok.studenti.len(), p.rok.studenti.len());
    assert_eq!(q.rozvrh.as_ref().unwrap().lekce.len(), p.rozvrh.as_ref().unwrap().lekce.len());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn profily_vyuky_odpovidaji_uvazkum() {
    let p = vychozi();
    let v = rozvrhovnik::profily::vyuka(&p.skola, &p.rok);
    for (u, h) in solver::zateze(&p.skola, &p.rok) {
        let soucet: f32 = v.iter().filter(|x| x.ucitel == u).map(|x| x.hodin).sum();
        assert!((soucet - h).abs() < 0.01, "{u}: profil {soucet} ≠ úvazek {h}");
    }
    // L/S hodiny se v profilu počítají pod skutečnými předměty, ne pod popiskem „DV/VV“
    assert!(v.iter().any(|x| x.predmet == "DV" && x.tridy == ["prima"] && x.skupina == "L" && x.hodin == 1.0));
    let tridy = rozvrhovnik::profily::tridy_vyuky(&p.skola.tridy, v.iter().filter(|x| x.predmet == "INF"));
    assert_eq!(tridy.first().map(String::as_str), Some("prima"));
}
