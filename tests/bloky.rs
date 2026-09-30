//! Bloky seminářů: graf kolizí, návrh minimálního počtu bloků, kontrola, řešič, export.

use rozvrhovnik::bloky::{self, Duvod};
use rozvrhovnik::data::*;
use rozvrhovnik::skupiny_io;
use rozvrhovnik::solver;
use rozvrhovnik::studenti_io::{self as io, Format};
use std::collections::BTreeMap;
use std::sync::atomic::AtomicBool;
use std::time::Duration;

/// Menu s 5 semináři: kolize A–B, B–C, C–A (trojúhelník) a D–E. Nejmenší počet bloků = 3.
fn male_menu() -> Projekt {
    let mut p = vychozi();
    let volby = ["A", "B", "C", "D", "E"];
    let ucitele = ["SOJ", "HAM", "UHR", "PAŘ", "LIP"];
    p.rok.menu = vec![Menu {
        id: "sem".into(),
        nazev: "Semináře".into(),
        tridy: vec!["septimaA".into()],
        volby: volby
            .iter()
            .zip(ucitele)
            .map(|(v, u)| Volba {
                id: v.to_string(),
                nazev: v.to_string(),
                predmet: "SMA".into(),
                ucitel: u.into(),
                ..Default::default()
            })
            .collect(),
        hodin_tydne: 2,
        dvojhodina: true,
        pocet_voleb: 2,
    }];
    let kombinace: [&[&str]; 5] = [&["A", "B"], &["B", "C"], &["C", "A"], &["D", "E"], &["A", "D"]];
    let zaci: Vec<u32> = p.rok.studenti.iter().filter(|s| s.trida == "septimaA").map(|s| s.id).collect();
    for s in p.rok.studenti.iter_mut() {
        s.volby.clear();
    }
    for (k, id) in zaci.iter().enumerate() {
        let s = p.rok.studenti.iter_mut().find(|s| s.id == *id).unwrap();
        s.volby.insert("sem".into(), kombinace[k % 5].iter().map(|x| x.to_string()).collect());
    }
    p
}

#[test]
fn graf_kolizi_a_duvody() {
    let p = male_menu();
    let a = bloky::analyzuj(&p.skola, &p.rok, &p.rok.menu[0]);
    let (ia, ib, id, ie) = (a.index("A").unwrap(), a.index("B").unwrap(), a.index("D").unwrap(), a.index("E").unwrap());
    assert!(a.koliduje(ia, ib));
    assert!(a.koliduje(id, ie));
    assert!(!a.koliduje(ib, ie));
    assert!(a.spolecnych_zaku(ia, ib) > 0);
    assert!(matches!(&a.duvody(ia, ib)[0], Duvod::Zaci(_)));
    assert_eq!(a.max_voleb_zaka, 2);
}

#[test]
fn stejny_ucitel_a_jedina_ucebna_koliduji() {
    let mut p = male_menu();
    p.rok.menu[0].dvojhodina = false; // 2 samostatné hodiny týdně → učitel pro každou zvlášť
    p.rok.menu[0].volby[3].ucitel = "LIP".into(); // D a E – stejný učitel (LIP)
    p.rok.menu[0].volby[1].ucitele_hodin = vec!["".into(), "LIP".into()]; // B: 2. hodina LIP
    p.rok.menu[0].volby[0].predmet = "PRG".into(); // A a C jen v PIF
    p.rok.menu[0].volby[2].predmet = "SEK".into();
    let a = bloky::analyzuj(&p.skola, &p.rok, &p.rok.menu[0]);
    let d = a.duvody(1, 4); // B × E: jen učitel (žáky společné nemají)
    assert_eq!(d, [Duvod::Ucitel("LIP".into())]);
    let d = a.duvody(3, 4); // D × E: žáci i učitel
    assert!(d.iter().any(|x| matches!(x, Duvod::Ucitel(u) if u == "LIP")), "{:?}", d);
    let d = a.duvody(0, 2);
    assert!(d.iter().any(|x| matches!(x, Duvod::Mistnost(m) if m == "PIF")), "{:?}", d);
}

#[test]
fn navrh_najde_nejmensi_pocet_bloku() {
    let p = male_menu();
    let a = bloky::analyzuj(&p.skola, &p.rok, &p.rok.menu[0]);
    let n = bloky::navrhni(&a, &vec![None; a.n()], Duration::from_secs(2));
    assert_eq!(n.pocet_bloku, 3);
    assert!(n.optimalni);
    assert_eq!(n.dolni_mez, 3);
    let b: Vec<Option<u8>> = n.bloky.iter().map(|&x| Some(x)).collect();
    assert!(bloky::kontrola(&a, &b).is_empty());
    assert!(n.bloky.iter().all(|&x| (1..=3).contains(&x)));
    // pevný blok se zachová
    let mut pevne = vec![None; a.n()];
    pevne[a.index("E").unwrap()] = Some(3);
    let n2 = bloky::navrhni(&a, &pevne, Duration::from_secs(2));
    assert_eq!(n2.bloky[a.index("E").unwrap()], 3);
    let b2: Vec<Option<u8>> = n2.bloky.iter().map(|&x| Some(x)).collect();
    assert!(bloky::kontrola(&a, &b2).is_empty());
}

#[test]
fn kontrola_a_kam_muze() {
    let p = male_menu();
    let a = bloky::analyzuj(&p.skola, &p.rok, &p.rok.menu[0]);
    let (ia, ib) = (a.index("A").unwrap(), a.index("B").unwrap());
    let mut b = vec![None; a.n()];
    b[ia] = Some(1);
    b[ib] = Some(1);
    let k = bloky::kontrola(&a, &b);
    assert_eq!(k.len(), 1);
    assert_eq!((k[0].blok, k[0].a.min(k[0].b), k[0].a.max(k[0].b)), (1, ia.min(ib), ia.max(ib)));
    let kam = bloky::kam_muze(&a, &b, a.index("C").unwrap());
    assert_eq!(kam.len(), 2); // blok 1 (kolize s A i B) a nový blok 2
    assert_eq!(kam[0].1.len(), 2);
    assert!(kam[1].1.is_empty());
}

#[test]
fn seminare_seedu_maji_bloky_podle_voleb() {
    let p = vychozi();
    for (menu, voleb) in [("sem-7", 3), ("sem-8", 6)] {
        let m = p.rok.menu.iter().find(|m| m.id == menu).unwrap();
        let a = bloky::analyzuj(&p.skola, &p.rok, m);
        let n = bloky::navrhni(&a, &vec![None; a.n()], Duration::from_secs(3));
        assert!(n.pocet_bloku >= voleb, "{menu}: {}", n.pocet_bloku);
        let b: Vec<Option<u8>> = n.bloky.iter().map(|&x| Some(x)).collect();
        assert!(bloky::kontrola(&a, &b).is_empty(), "{menu}");
        // vyrovnání: žádný blok není prázdný
        for blok in 1..=n.pocet_bloku as u8 {
            assert!(bloky::obsazeni_bloku(&a, &b, blok).0 > 0, "{menu} blok {blok}");
        }
    }
}

#[test]
fn resic_planuje_blok_paralelne() {
    let mut p = vychozi();
    let mi = p.rok.menu.iter().position(|m| m.id == "sem-7").unwrap();
    let a = bloky::analyzuj(&p.skola, &p.rok, &p.rok.menu[mi]);
    let n = bloky::navrhni(&a, &vec![None; a.n()], Duration::from_secs(2));
    let b: Vec<Option<u8>> = n.bloky.iter().map(|&x| Some(x)).collect();
    bloky::nastav_bloky(&mut p.rok.menu[mi], &a, &b);
    p.skola.nastaveni.casovy_limit_s = 15;
    let v = solver::generuj(&p.skola, &p.rok, &p.piny, &AtomicBool::new(false), None);
    assert!(v.problemy.is_empty(), "{:?}", &v.problemy[..v.problemy.len().min(5)]);
    // všechny semináře jednoho bloku ve stejném čase
    let mut cas: BTreeMap<u8, Vec<Option<u8>>> = BTreeMap::new();
    for (i, l) in v.lekce.iter().enumerate() {
        if l.menu.as_deref() == Some("sem-7") {
            let blok = p.rok.menu[mi].volby.iter().find(|x| x.id == l.skupina).unwrap().blok.unwrap();
            cas.entry(blok).or_default().push(v.slot[i]);
        }
    }
    assert_eq!(cas.len(), n.pocet_bloku);
    for (blok, sloty) in cas {
        let mut s = sloty.clone();
        s.dedup();
        assert_eq!(s.len(), 1, "blok {blok}: {:?}", sloty);
    }
}

#[test]
fn export_bloku_lze_znovu_naimportovat() {
    let mut p = male_menu();
    let a = bloky::analyzuj(&p.skola, &p.rok, &p.rok.menu[0]);
    let n = bloky::navrhni(&a, &vec![None; a.n()], Duration::from_secs(1));
    let b: Vec<Option<u8>> = n.bloky.iter().map(|&x| Some(x)).collect();
    bloky::nastav_bloky(&mut p.rok.menu[0], &a, &b);
    let csv = bloky::export_csv(&p.skola, &p.rok, &p.rok.menu[0]);
    assert!(csv.starts_with("\u{FEFF}Blok;Seminář;"));
    // import do kopie bez bloků a voleb → stejné bloky i volby
    let mut q = p.clone();
    for v in &mut q.rok.menu[0].volby {
        v.blok = None;
    }
    for s in &mut q.rok.studenti {
        s.volby.clear();
    }
    let t = io::text_na_tabulku(Format::Csv, csv.as_bytes(), false);
    let nacteno = skupiny_io::zpracuj(t.radky, 0).unwrap();
    assert!(nacteno.upozorneni.is_empty(), "{:?}", nacteno.upozorneni);
    let v = skupiny_io::aplikuj(&mut q.skola, &mut q.rok, "sem", &nacteno, Default::default());
    assert!(v.problemy.is_empty(), "{:?}", v.problemy);
    let bloky_p: Vec<Option<u8>> = p.rok.menu[0].volby.iter().map(|x| x.blok).collect();
    let bloky_q: Vec<Option<u8>> = q.rok.menu[0].volby.iter().map(|x| x.blok).collect();
    assert_eq!(bloky_p, bloky_q);
    let serazene = |x: Option<&Vec<String>>| {
        let mut v = x.cloned().unwrap_or_default();
        v.sort();
        v
    };
    for (s, t) in p.rok.studenti.iter().zip(&q.rok.studenti) {
        assert_eq!(serazene(s.volby.get("sem")), serazene(t.volby.get("sem")), "{}", s.jmeno);
    }
}

#[test]
fn ucitele_po_hodinach_u_volby() {
    let mut p = vychozi();
    let mi = p.rok.menu.iter().position(|m| m.id == "3j-5").unwrap();
    let nj = p.rok.menu[mi].volby.iter_mut().find(|v| v.id == "NJ2").unwrap();
    nj.ucitel = "UHR".into();
    nj.ucitele_hodin = vec!["UHR".into(), "UHR".into(), "KOŠ".into()];
    let vstup = solver::postav_lekce(&p.skola, &p.rok);
    let mut ucitele: Vec<(String, String)> = vstup
        .lekce
        .iter()
        .filter(|l| l.menu.as_deref() == Some("3j-5") && l.skupina == "NJ2")
        .map(|l| (l.klic.clone(), l.ucitel.clone()))
        .collect();
    ucitele.sort();
    let jen: Vec<&str> = ucitele.iter().map(|(_, u)| u.as_str()).collect();
    assert_eq!(jen, ["UHR", "UHR", "KOŠ"]);
    // úvazky: KOŠ má o 1 h víc, UHR o 1 h méně než kdyby učila všechny 3
    let z: BTreeMap<String, f32> = solver::zateze(&p.skola, &p.rok).into_iter().collect();
    p.rok.menu[mi].volby.iter_mut().find(|v| v.id == "NJ2").unwrap().ucitele_hodin.clear();
    let z0: BTreeMap<String, f32> = solver::zateze(&p.skola, &p.rok).into_iter().collect();
    assert_eq!(z["KOŠ"] - z0["KOŠ"], 1.0);
    assert_eq!(z0["UHR"] - z["UHR"], 1.0);
}

#[test]
fn kdo_brani_mensimu_poctu_bloku() {
    let p = rozvrhovnik::demo::projekt(7);
    let m = p.rok.menu.iter().find(|m| m.id == "sem-7").unwrap();
    let a = bloky::analyzuj(&p.skola, &p.rok, m);
    let pred = bloky::navrhni(&a, &vec![None; a.n()], Duration::from_secs(2)).pocet_bloku;
    let jmena: BTreeMap<u32, String> = p.rok.studenti.iter().map(|s| (s.id, s.jmeno.clone())).collect();
    let poradi = bloky::kdo_brani(&a, &jmena, Duration::from_secs(4));
    assert!(!poradi.is_empty());
    // počty bloků po změnách neklesají pod počet voleb žáka a poslední je nejlepší
    let posledni = poradi.last().unwrap().2;
    assert!(posledni < pred, "{pred} → {posledni}");
    assert!(posledni >= a.max_voleb_zaka);
    assert!(poradi.iter().all(|x| !x.0.is_empty() && !x.1.is_empty()));
}
