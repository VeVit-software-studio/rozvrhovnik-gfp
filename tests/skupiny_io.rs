//! Import skupin (semináře, jazykové skupiny) ve formátu školního nástroje na semináře.

use rozvrhovnik::data::*;
use rozvrhovnik::skupiny_io::{self as sk, Moznosti};
use rozvrhovnik::studenti_io::{self as io, Format};

/// Stejná struktura jako výstup nástroje ze screenshotu (jména smyšlená).
const SOUBOR: &str = "\
Ročník;Seminář;Mezitřídní;Třídy;Počet žáků;Jméno žáka;Třída žáka
Septima + třeťák;SBI1;;7.B;3;Brokl Marek;7.B
;;;;;Dolanský Jan;7.B
;;;;;Haklová Denisa;7.B
Septima + třeťák;SBI2;ano;3.A, 7.A;3;Boháčová Magdaléna;3.A
;;;;;Grohman David;7.A

Septima + třeťák;SFY1;ano;7.A, 7.B;2;Brokl Marek;7.B
;;;;;Grohman David;7.A
";

fn nacti(text: &str) -> sk::NactenoSkupiny {
    let t = io::text_na_tabulku(Format::Csv, text.as_bytes(), false);
    sk::zpracuj(t.radky, 0).unwrap()
}

#[test]
fn nacteni_skupin_ve_formatu_nastroje() {
    let n = nacti(SOUBOR);
    let kody: Vec<&str> = n.skupiny.iter().map(|g| g.kod.as_str()).collect();
    assert_eq!(kody, ["SBI1", "SBI2", "SFY1"]);
    assert_eq!(n.skupiny[0].zaci.len(), 3);
    assert_eq!(n.skupiny[0].zaci[1].jmeno, "Dolanský Jan");
    assert_eq!(n.skupiny[0].zaci[1].radek, 3);
    assert_eq!(n.skupiny[1].tridy, "3.A, 7.A");
    assert_eq!(n.skupiny[1].pocet_uvedeny, Some(3));
    // SBI2 uvádí 3 žáky, v seznamu jsou 2
    assert!(n.upozorneni.iter().any(|u| u.contains("SBI2") && u.contains("uvedeno 3")), "{:?}", n.upozorneni);
    assert_eq!(n.upozorneni.len(), 1, "{:?}", n.upozorneni);
}

#[test]
fn stejny_kod_na_vice_mistech_se_slouci_a_blok_se_nacte() {
    let text = "Seminář;Blok;Jméno žáka;Třída žáka\nSBI1;A;Brokl Marek;7.B\nSFY1;2;Grohman David;7.A\nSBI1;;Dolanský Jan;7.B\n";
    let n = nacti(text);
    assert_eq!(n.skupiny.len(), 2);
    assert_eq!(n.skupiny[0].zaci.len(), 2);
    assert_eq!(n.skupiny[0].blok, Some(1));
    assert_eq!(n.skupiny[1].blok, Some(2));
    assert_eq!(sk::parsuj_blok("Blok 3"), Some(3));
    assert_eq!(sk::parsuj_blok("C"), Some(3));
    assert_eq!(sk::parsuj_blok("x1y"), None);
}

#[test]
fn chybejici_sloupce_jsou_chyba() {
    let t = io::text_na_tabulku(Format::Csv, "Jméno;Třída\nA B;7.A\n".as_bytes(), false);
    assert!(sk::zpracuj(t.radky, 0).unwrap_err().contains("Seminář"));
}

fn rok_se_zaky() -> Projekt {
    let mut p = vychozi();
    // skuteční žáci septim a třetí A podle souboru (ostatní zástupní zůstanou)
    for (jmeno, trida) in [
        ("Marek Brokl", "septimaB"),
        ("Jan Dolanský", "septimaB"),
        ("Denisa Haklová", "septimaB"),
        ("Magdaléna Boháčová", "3A"),
        ("David Grohman", "septimaA"),
    ] {
        let id = p.rok.nove_id();
        p.rok.studenti.push(Student {
            id,
            jmeno: jmeno.into(),
            trida: trida.into(),
            skupina_aj: SkupinaAj::A,
            pohlavi: Pohlavi::CH,
            volby: Default::default(),
            status: StavStudenta::Aktivni,
        });
    }
    p
}

#[test]
fn prenos_do_menu_seminaru() {
    let mut p = rok_se_zaky();
    let n = nacti(SOUBOR);
    assert_eq!(sk::navrhni_menu(&p.skola, &p.rok, &n).as_deref(), Some("sem-7"));
    let predmetu = p.skola.predmety.len();
    let v = sk::aplikuj(&mut p.skola, &mut p.rok, "sem-7", &n, Moznosti::default());
    assert!(v.problemy.is_empty(), "{:?}", v.problemy);
    assert_eq!(v.volby_nove, ["SBI1", "SBI2", "SFY1"]);
    // SBI neexistuje → nový předmět; SFY existuje → použije se (a učitel sesterské volby SFY)
    assert_eq!(v.predmety_nove, ["SBI"]);
    assert_eq!(p.skola.predmety.len(), predmetu + 1);
    let menu = p.rok.menu.iter().find(|m| m.id == "sem-7").unwrap();
    let sfy1 = menu.volby.iter().find(|x| x.id == "SFY1").unwrap();
    assert_eq!(sfy1.predmet, "SFY");
    assert_eq!(sfy1.ucitel, menu.volby.iter().find(|x| x.id == "SFY").unwrap().ucitel);
    // SBI1 nemá učitele → upozornění
    assert!(v.varovani.iter().any(|x| x.contains("SBI1") && x.contains("učitele")), "{:?}", v.varovani);
    // volby žáků: Brokl má SBI1 + SFY1 (pořadí jména v souboru „Příjmení Jméno“ nevadí)
    let brokl = p.rok.studenti.iter().find(|s| s.jmeno == "Marek Brokl").unwrap();
    assert_eq!(brokl.volby["sem-7"], ["SBI1", "SFY1"]);
    assert_eq!(v.zaci_prirazeni, 5);
    // ostatní žáci septim a 3A v souboru nejsou → jejich volby semináře se vymazaly
    assert!(v.zaci_bez_skupiny > 0);
    let zastupny = p.rok.studenti.iter().find(|s| s.trida == "septimaA" && s.jmeno.starts_with("Žák")).unwrap();
    assert!(!zastupny.volby.contains_key("sem-7"));
    // původní volby menu zůstaly (odebrat_volby_mimo_soubor = false)
    assert!(menu.volby.iter().any(|x| x.id == "SMA"));
}

#[test]
fn chybejici_zaci_se_nahlasi_nebo_pridaji() {
    let mut p = vychozi();
    let n = nacti(SOUBOR);
    let mut q = p.clone();
    let v = sk::aplikuj(&mut q.skola, &mut q.rok, "sem-7", &n, Moznosti::default());
    assert_eq!(v.zaci_prirazeni, 0);
    assert_eq!(v.problemy.len(), 7, "{:?}", v.problemy);
    assert!(v.problemy[0].contains("není ve třídě"));
    let pred = p.rok.studenti.len();
    let v = sk::aplikuj(
        &mut p.skola,
        &mut p.rok,
        "sem-7",
        &n,
        Moznosti { pridat_chybejici_zaky: true, odebrat_volby_mimo_soubor: true },
    );
    assert!(v.problemy.is_empty(), "{:?}", v.problemy);
    assert_eq!(v.zaci_pridani, 5);
    assert_eq!(p.rok.studenti.len(), pred + 5);
    let menu = p.rok.menu.iter().find(|m| m.id == "sem-7").unwrap();
    assert_eq!(menu.volby.len(), 3, "odebrány volby mimo soubor");
    assert_eq!(v.volby_odebrane.len(), 14);
}

#[test]
fn nova_trida_se_prida_do_menu_a_nezname_tridy_hlasi() {
    let mut p = rok_se_zaky();
    let text = "Seminář;Jméno žáka;Třída žáka\nAJ-a;Marek Brokl;7.B\nAJ-a;Někdo Neznámý;9.Z\n";
    let n = nacti(text);
    let v = sk::aplikuj(&mut p.skola, &mut p.rok, "3j-3", &n, Moznosti::default());
    assert_eq!(v.tridy_pridane, ["septima B"]);
    assert!(v.problemy.iter().any(|x| x.contains("neznámá třída „9.Z“")), "{:?}", v.problemy);
    assert_eq!(sk::zaklad_kodu("AJ-a"), "AJ-a");
    assert_eq!(sk::zaklad_kodu("SBI12"), "SBI");
}
