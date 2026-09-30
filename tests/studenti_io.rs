//! Import a export seznamu žáků: txt, csv, xlsx, json.

use rozvrhovnik::data::*;
use rozvrhovnik::studenti_io::{self as io, Format, Moznosti, Rezim};
use rust_xlsxwriter::Workbook;
use std::path::PathBuf;

fn adresar(jmeno: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("rozvrh-io-{}-{}", jmeno, std::process::id()));
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn skola() -> Skola {
    vychozi().skola
}

#[test]
fn csv_parser_uvozovky_a_oddelovace() {
    let r = io::parsuj_csv("a;b;\"c;d\"\r\n\"x \"\"q\"\" y\";;z\n", ';');
    assert_eq!(r, vec![vec!["a", "b", "c;d"], vec!["x \"q\" y", "", "z"]]);
    // uvozovky po mezeře za oddělovačem (formát TXT „a; \"b;c\"“)
    assert_eq!(io::parsuj_csv("a; \"b;c\"\n", ';'), vec![vec!["a", "b;c"]]);
    // víceřádková buňka
    let r = io::parsuj_csv("\"a\nb\",c", ',');
    assert_eq!(r, vec![vec!["a\nb", "c"]]);
}

#[test]
fn seznam_jmen_bez_hlavicky() {
    let s = skola();
    let n =
        io::nacti_text(&s, Format::Txt, "Jana Nováková\n\n  Petr Svoboda  \r\nEva Malá\n".as_bytes(), false).unwrap();
    let jmena: Vec<&str> = n.zaznamy.iter().map(|z| z.jmeno.as_str()).collect();
    assert_eq!(jmena, ["Jana Nováková", "Petr Svoboda", "Eva Malá"]);
    assert!(n.zaznamy.iter().all(|z| z.trida.is_none()));
    // čísla řádků odpovídají souboru (prázdný řádek se počítá)
    assert_eq!(n.zaznamy[1].radek, 3);
}

#[test]
fn jmena_s_carkou_prijmeni_jmeno() {
    let s = skola();
    let n = io::nacti_text(&s, Format::Txt, "Novák, Jan\nMalá, Eva\n".as_bytes(), false).unwrap();
    assert_eq!(n.zaznamy[0].jmeno, "Novák Jan");
    assert_eq!(n.zaznamy[1].jmeno, "Malá Eva");
}

#[test]
fn bez_hlavicky_poznava_sloupce_podle_obsahu() {
    let s = skola();
    let text = "Jana Nováková; kvarta A; AJb; D\nPetr Svoboda;1.A;ch;propadá\n";
    let n = io::nacti_text(&s, Format::Txt, text.as_bytes(), false).unwrap();
    let z = &n.zaznamy[0];
    assert_eq!(
        (z.jmeno.as_str(), z.trida.as_deref(), z.skupina, z.pohlavi),
        ("Jana Nováková", Some("kvarta A"), Some(SkupinaAj::B), Some(Pohlavi::D))
    );
    let z = &n.zaznamy[1];
    assert_eq!(z.pohlavi, Some(Pohlavi::CH));
    assert_eq!(z.stav, Some(StavStudenta::Propada));
    assert_eq!(io::najdi_tridu(&s, z.trida.as_deref().unwrap()), Some("1A".to_string()));
}

#[test]
fn hlavicka_jmeno_a_prijmeni_zvlast_a_ruzne_varianty() {
    let s = skola();
    let text = "Příjmení;Jméno;Třída;Pohlaví;Poznámka\nNováková;Jana;prima;Ž;x\nSvoboda;Petr;prima;M;\n";
    let n = io::nacti_text(&s, Format::Csv, text.as_bytes(), false).unwrap();
    assert_eq!(n.zaznamy[0].jmeno, "Jana Nováková");
    assert_eq!(n.zaznamy[0].pohlavi, Some(Pohlavi::D));
    assert_eq!(n.zaznamy[1].pohlavi, Some(Pohlavi::CH));
    assert!(n.popis_sloupcu.contains("Poznámka (ignorován)"));
    // oddělovač čárka a tabulátor
    let n = io::nacti_text(&s, Format::Csv, "name,class\nAnna Nováková,tercie\n".as_bytes(), false).unwrap();
    assert_eq!(n.zaznamy[0].trida.as_deref(), Some("tercie"));
    let n = io::nacti_text(&s, Format::Csv, "Jméno\tTřída\nAnna Nováková\ttercie\n".as_bytes(), false).unwrap();
    assert_eq!(n.zaznamy[0].trida.as_deref(), Some("tercie"));
}

#[test]
fn hlavicka_bez_sloupce_se_jmenem_je_chyba() {
    let s = skola();
    let e = io::nacti_text(&s, Format::Csv, "Třída;Pohlaví\nprima;D\n".as_bytes(), false).unwrap_err();
    assert!(e.contains("chybí sloupec se jménem"), "{e}");
    assert!(io::nacti_text(&s, Format::Txt, b"\n\n", false).is_err());
}

#[test]
fn kodovani_bom_a_windows_1250() {
    let s = skola();
    let mut bom = vec![0xEF, 0xBB, 0xBF];
    bom.extend("Jméno;Třída\nŽofie Čermáková;prima\n".as_bytes());
    let n = io::nacti_text(&s, Format::Csv, &bom, false).unwrap();
    assert_eq!(n.zaznamy[0].jmeno, "Žofie Čermáková");
    assert!(n.upozorneni.is_empty());
    // Windows-1250 (český Excel „CSV“)
    let (bajty, _, _) = encoding_rs::WINDOWS_1250.encode("Jméno;Třída\nŽofie Čermáková;prima\n");
    let n = io::nacti_text(&s, Format::Csv, &bajty, false).unwrap();
    assert_eq!(n.zaznamy[0].jmeno, "Žofie Čermáková");
    assert!(n.upozorneni[0].contains("Windows-1250"), "{:?}", n.upozorneni);
    // UTF-16 s BOM (Excel „Unicode text“)
    let mut u16b = vec![0xFF, 0xFE];
    for c in "Jméno\tTřída\nŽofie Čermáková\tprima\n".encode_utf16() {
        u16b.extend(c.to_le_bytes());
    }
    let n = io::nacti_text(&s, Format::Txt, &u16b, false).unwrap();
    assert_eq!(n.zaznamy[0].jmeno, "Žofie Čermáková");
}

#[test]
fn nerozpoznane_hodnoty_davaji_upozorneni() {
    let s = skola();
    let n = io::nacti_text(&s, Format::Csv, "Jméno;Pohlaví;AJ\nJana Nováková;??;Z\n".as_bytes(), false).unwrap();
    assert_eq!(n.zaznamy[0].pohlavi, None);
    assert_eq!(n.upozorneni.len(), 2, "{:?}", n.upozorneni);
}

#[test]
fn excel_nacteni_vcetne_cisel_a_prazdnych_radku() {
    let s = skola();
    let d = adresar("xlsx-in");
    let cesta = d.join("seznam.xlsx");
    let mut wb = Workbook::new();
    let ws = wb.add_worksheet();
    ws.write_string(0, 0, "Č.").unwrap();
    ws.write_string(0, 1, "Příjmení").unwrap();
    ws.write_string(0, 2, "Jméno").unwrap();
    ws.write_string(0, 3, "Třída").unwrap();
    ws.write_string(1, 1, "Nováková").unwrap();
    ws.write_string(1, 2, "Jana").unwrap();
    ws.write_string(1, 3, "kvarta A").unwrap();
    ws.write_number(1, 0, 1.0).unwrap();
    // prázdný řádek 3
    ws.write_string(3, 1, "Svoboda").unwrap();
    ws.write_string(3, 2, "Petr").unwrap();
    ws.write_string(3, 3, "kvartaA").unwrap();
    wb.save(&cesta).unwrap();
    let n = io::nacti(&s, &cesta).unwrap();
    assert_eq!(n.format, Format::Xlsx);
    let jmena: Vec<&str> = n.zaznamy.iter().map(|z| z.jmeno.as_str()).collect();
    assert_eq!(jmena, ["Jana Nováková", "Petr Svoboda"]);
    assert_eq!(n.zaznamy[1].radek, 4);
    let plan = io::naplanuj(&s, &n.zaznamy, "");
    assert!(plan.polozky.iter().all(|p| p.trida == "kvartaA") && plan.problemy.is_empty());
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn neznama_pripona_a_chybejici_soubor() {
    let s = skola();
    assert!(io::nacti(&s, &PathBuf::from("x.pdf")).unwrap_err().contains("Nepodporovaný"));
    assert!(io::nacti(&s, &PathBuf::from("/neexistuje/x.csv")).unwrap_err().contains("Nelze číst"));
    assert!(io::nacti(&s, &PathBuf::from("/neexistuje/x.xlsx")).is_err());
}

#[test]
fn json_import_ruzne_tvary() {
    let s = skola();
    let d = adresar("json-in");
    let c = d.join("a.json");
    std::fs::write(&c, r#"["Jana Nováková", "Petr Svoboda"]"#).unwrap();
    assert_eq!(io::nacti(&s, &c).unwrap().zaznamy.len(), 2);
    std::fs::write(
        &c,
        r#"{"studenti":[{"jmeno":"Jana Nováková","trida":"prima","skupina_aj":"AJb","pohlavi":"D","stav":"propada","volby":{"vol-5":["VV2"]}},{"jmeno":""}]}"#,
    )
    .unwrap();
    let n = io::nacti(&s, &c).unwrap();
    assert_eq!(n.zaznamy.len(), 1);
    assert_eq!(n.zaznamy[0].skupina, Some(SkupinaAj::B));
    assert_eq!(n.zaznamy[0].volby["vol-5"], ["VV2"]);
    assert_eq!(n.upozorneni.len(), 1);
    std::fs::write(&c, "{ nejde").unwrap();
    assert!(io::nacti(&s, &c).unwrap_err().contains("JSON"));
    let _ = std::fs::remove_dir_all(&d);
}

type Radek = (String, String, SkupinaAj, Pohlavi, StavStudenta, Vec<(String, Vec<String>)>);

fn porovnatelne(rok: &SkolniRok) -> Vec<Radek> {
    let mut v: Vec<_> = rok
        .studenti
        .iter()
        .map(|s| {
            (
                s.jmeno.clone(),
                s.trida.clone(),
                s.skupina_aj,
                s.pohlavi,
                s.status,
                s.volby.iter().map(|(k, v)| (k.clone(), v.clone())).collect(),
            )
        })
        .collect();
    v.sort_by(|a, b| (&a.1, &a.0).cmp(&(&b.1, &b.0)));
    v
}

#[test]
fn export_a_zpetny_import_ve_vsech_formatech() {
    let p = vychozi();
    let mut puvodni = p.rok.clone();
    // zpestřit data: propadá, odešel, zvláštní znaky ve jméně
    puvodni.studenti.truncate(120);
    puvodni.studenti[0].status = StavStudenta::Propada;
    puvodni.studenti[1].status = StavStudenta::Odesel;
    puvodni.studenti[2].jmeno = "Jan \"Honza\"; Dvořák".into();
    let d = adresar("roundtrip");
    let vsichni: Vec<&Student> = puvodni.studenti.iter().collect();
    for f in [Format::Csv, Format::Json, Format::Xlsx, Format::Txt] {
        let cesta = d.join(format!("zaci.{}", f.pripona()));
        io::exportuj(&p.skola, &puvodni, &vsichni, f, &cesta, Moznosti::default()).unwrap();
        assert!(std::fs::metadata(&cesta).unwrap().len() > 0, "{:?}", f);
        let n = io::nacti(&p.skola, &cesta).unwrap_or_else(|e| panic!("{:?}: {e}", f));
        assert!(n.upozorneni.is_empty(), "{:?}: {:?}", f, n.upozorneni);
        assert_eq!(n.zaznamy.len(), 120, "{:?}", f);
        // import do roku bez žáků, menu zachována → musí vzniknout totéž
        let mut cil = SkolniRok { studenti: vec![], ..puvodni.clone() };
        let plan = io::naplanuj(&p.skola, &n.zaznamy, "");
        assert!(plan.problemy.is_empty(), "{:?}: {:?}", f, plan.problemy);
        let v = io::aplikuj(&p.skola, &mut cil, &plan, Rezim::PridatAktualizovat);
        assert_eq!(v.pridano, 120, "{:?}", f);
        assert!(v.varovani.is_empty(), "{:?}: {:?}", f, v.varovani);
        assert_eq!(porovnatelne(&cil), porovnatelne(&puvodni), "{:?}", f);
    }
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn txt_jen_jmena_a_obsah_csv() {
    let p = vychozi();
    let zaci: Vec<&Student> = p.rok.studenti.iter().take(3).collect();
    let txt = io::txt_text(&p.skola, &zaci, Moznosti { txt_jen_jmena: true });
    assert_eq!(txt.lines().count(), 3);
    assert!(txt.lines().all(|l| !l.contains(';')));
    let csv = io::csv_text(&p.skola, &zaci);
    assert!(csv.starts_with("\u{FEFF}Jméno;Třída;AJ;Pohlaví;Stav;Volby\r\n"));
    // excel otevře BOM + středníky; export nikdy nevyrobí neukončený řádek
    assert!(csv.ends_with("\r\n"));
}

#[test]
fn rezimy_importu() {
    let p = vychozi();
    let mut rok = p.rok.clone();
    let pred_prima = rok.studenti.iter().filter(|s| s.trida == "prima").count();
    let pred_ostatni = rok.studenti.len() - pred_prima;
    let zaznamy = io::nacti_text(
        &p.skola,
        Format::Txt,
        "Jana Nováková\nPetr Svoboda\nEva Malá\nTomáš Černý\nJana Nováková\n".as_bytes(),
        false,
    )
    .unwrap()
    .zaznamy;

    // bez třídy a bez výchozí třídy → problémy
    let plan = io::naplanuj(&p.skola, &zaznamy, "");
    assert!(plan.polozky.is_empty() && plan.problemy.len() == 5);

    let plan = io::naplanuj(&p.skola, &zaznamy, "prima");
    // přidat: duplicita ve stejném souboru se přeskočí
    let v = io::aplikuj(&p.skola, &mut rok, &plan, Rezim::PridatAktualizovat);
    assert_eq!((v.pridano, v.aktualizovano, v.preskoceno), (4, 0, 1), "{:?}", v);
    assert_eq!(rok.studenti.iter().filter(|s| s.trida == "prima").count(), pred_prima + 4);
    // doplnění do menší skupiny: rozdíl skupin ≤ 1 (na začátku 14:14)
    let a = rok.studenti.iter().filter(|s| s.trida == "prima" && s.skupina_aj == SkupinaAj::A).count();
    let b = rok.studenti.iter().filter(|s| s.trida == "prima" && s.skupina_aj == SkupinaAj::B).count();
    assert!(a.abs_diff(b) <= 1, "{a}:{b}");
    // podruhé = aktualizace, nic nepřibude
    let v = io::aplikuj(&p.skola, &mut rok, &plan, Rezim::PridatAktualizovat);
    assert_eq!((v.pridano, v.aktualizovano), (0, 4));
    // pohlaví z odhadu: Jana = D, Petr = CH
    let jana = rok.studenti.iter().find(|s| s.jmeno == "Jana Nováková").unwrap();
    assert_eq!(jana.pohlavi, Pohlavi::D);

    // nahradit třídy: prima má pak přesně 4 žáky, ostatní beze změny; 15:15 pro celou novou třídu
    let v = io::aplikuj(&p.skola, &mut rok, &plan, Rezim::NahraditTridy);
    assert_eq!(v.odstraneno, pred_prima + 4);
    assert_eq!(rok.studenti.iter().filter(|s| s.trida == "prima").count(), 4);
    assert_eq!(rok.studenti.len(), pred_ostatni + 4);
    let a = rok.studenti.iter().filter(|s| s.trida == "prima" && s.skupina_aj == SkupinaAj::A).count();
    assert_eq!(a, 2);

    // nahradit vše
    let v = io::aplikuj(&p.skola, &mut rok, &plan, Rezim::NahraditVse);
    assert_eq!(rok.studenti.len(), 4);
    assert_eq!(v.odstraneno, pred_ostatni + 4);
    // ID jsou unikátní a dál rostou
    let mut ids: Vec<u32> = rok.studenti.iter().map(|s| s.id).collect();
    ids.sort_unstable();
    ids.dedup();
    assert_eq!(ids.len(), 4);
}

#[test]
fn aktualizace_a_overeni_voleb() {
    let p = vychozi();
    let mut rok = p.rok.clone();
    let s = rok.studenti.iter().find(|s| s.trida == "kvintaA").unwrap().clone();
    let puvodni_vol5 = s.volby["vol-5"].clone();
    let jina = if puvodni_vol5[0] == "VV2" { "DV2" } else { "VV2" };
    let text =
        format!("Jméno;Třída;AJ;Stav;Volby\n{};kvinta A;AJb;propadá;vol-5={}|neexistuje=X|sem-7=SMA\n", s.jmeno, jina);
    let n = io::nacti_text(&p.skola, Format::Csv, text.as_bytes(), false).unwrap();
    let plan = io::naplanuj(&p.skola, &n.zaznamy, "");
    let v = io::aplikuj(&p.skola, &mut rok, &plan, Rezim::PridatAktualizovat);
    assert_eq!((v.pridano, v.aktualizovano), (0, 1));
    let po = rok.student(s.id).unwrap();
    assert_eq!(po.skupina_aj, SkupinaAj::B);
    assert_eq!(po.status, StavStudenta::Propada);
    assert_eq!(po.volby["vol-5"], [jina]);
    assert!(!po.volby.contains_key("neexistuje"));
    assert!(!po.volby.contains_key("sem-7") || po.volby["sem-7"] == s.volby.get("sem-7").cloned().unwrap_or_default());
    // neexistující menu i menu, které se netýká třídy (sem-7 není pro kvintu), se hlásí
    assert!(v.varovani.iter().any(|x| x.contains("neznámé menu")), "{:?}", v.varovani);
    assert!(v.varovani.iter().any(|x| x.contains("netýká jeho třídy")), "{:?}", v.varovani);
}

#[test]
fn trida_se_pozna_ruznymi_zapisy() {
    let s = skola();
    for (zapis, id) in [
        ("prima", "prima"),
        ("Prima", "prima"),
        ("kvarta A", "kvartaA"),
        ("KVARTAA", "kvartaA"),
        ("1.A", "1A"),
        ("první A", "1A"),
        ("4. A", "4A"),
        ("oktáva B", "oktavaB"),
    ] {
        assert_eq!(io::najdi_tridu(&s, zapis), Some(id.to_string()), "{zapis}");
    }
    assert_eq!(io::najdi_tridu(&s, "9.Z"), None);
    assert_eq!(io::najdi_tridu(&s, ""), None);
}

// ───────────── aliasy tříd, rodné číslo, DBF, pořadí jména ─────────────

#[test]
fn aliasy_trid_z_exportu_skoly() {
    let s = skola();
    for (zapis, id) in [("7.A", "septimaA"), ("7.B", "septimaB"), ("8.b", "oktavaB"), ("3.A", "3A"), ("5.A", "kvintaA")]
    {
        assert_eq!(io::najdi_tridu(&s, zapis), Some(id.to_string()), "{zapis}");
    }
    // nejednoznačný alias → nic (a srozumitelné hlášení)
    let mut s2 = s.clone();
    s2.tridy.iter_mut().find(|t| t.id == "kvartaA").unwrap().alias = "4.A".into();
    assert_eq!(io::najdi_tridu(&s2, "4.A"), None);
    assert_eq!(io::nejednoznacna_trida(&s2, "4.A").len(), 2);
    let z = io::nacti_text(&s2, Format::Csv, "Jméno;Třída\nJana Nováková;4.A\n".as_bytes(), false).unwrap();
    let plan = io::naplanuj(&s2, &z.zaznamy, "");
    assert!(plan.problemy[0].contains("nejednoznačná"), "{:?}", plan.problemy);
}

#[test]
fn pohlavi_z_rodneho_cisla() {
    assert_eq!(io::pohlavi_z_rc("085512/1234"), Some(Pohlavi::D));
    assert_eq!(io::pohlavi_z_rc("0803051234"), Some(Pohlavi::CH));
    assert_eq!(io::pohlavi_z_rc("087512/123"), Some(Pohlavi::D)); // +70 (od 2004)
    assert_eq!(io::pohlavi_z_rc("082512/1234"), Some(Pohlavi::CH)); // +20
    assert_eq!(io::pohlavi_z_rc("abc"), None);
    let s = skola();
    // sloupec Pohlaví má přednost; rodné číslo se neukládá
    let text = "Jméno;Třída;Rodné číslo;Pohlaví\nKim Lee;prima;085512/1234;\nAlex Novák;prima;085512/1234;CH\n";
    let n = io::nacti_text(&s, Format::Csv, text.as_bytes(), false).unwrap();
    assert_eq!(n.zaznamy[0].pohlavi, Some(Pohlavi::D));
    assert_eq!(n.zaznamy[1].pohlavi, Some(Pohlavi::CH));
}

#[test]
fn hlavicka_z_nastroje_na_seminare() {
    // sloupce „Jméno žáka“ a „Třída žáka“ (Příjmení Jméno)
    let s = skola();
    let text = "Seminář;Jméno žáka;Třída žáka\nSBI1;Brokl Marek;7.B\n;Dolanský Jan;7.B\n";
    let n = io::nacti_text(&s, Format::Csv, text.as_bytes(), false).unwrap();
    assert_eq!(n.zaznamy.len(), 2);
    assert_eq!(n.zaznamy[0].jmeno, "Brokl Marek");
    assert_eq!(io::najdi_tridu(&s, n.zaznamy[1].trida.as_deref().unwrap()), Some("septimaB".into()));
}

#[test]
fn dbf_cp852_i_bez_znacky_kodovani() {
    let s = skola();
    for (soubor, upozorneni) in [("zaci_cp852.dbf", false), ("zaci_1250_bez_znacky.dbf", true)] {
        let cesta = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/data").join(soubor);
        let n = io::nacti(&s, &cesta).unwrap_or_else(|e| panic!("{soubor}: {e}"));
        assert_eq!(n.format, Format::Dbf);
        let jmena: Vec<&str> = n.zaznamy.iter().map(|z| z.jmeno.as_str()).collect();
        // smazaný záznam chybí; Jméno + Příjmení ze dvou sloupců
        assert_eq!(jmena, ["Žofie Řezníčková", "Ondřej Šťastný", "Anežka Dvořáková"], "{soubor}");
        let pohlavi: Vec<Option<Pohlavi>> = n.zaznamy.iter().map(|z| z.pohlavi).collect();
        assert_eq!(pohlavi, [Some(Pohlavi::D), Some(Pohlavi::CH), Some(Pohlavi::D)], "{soubor}");
        let plan = io::naplanuj(&s, &n.zaznamy, "");
        let tridy: Vec<&str> = plan.polozky.iter().map(|p| p.trida.as_str()).collect();
        assert_eq!(tridy, ["septimaA", "septimaB", "3A"], "{soubor}");
        assert_eq!(n.upozorneni.iter().any(|u| u.contains("odhadnuto")), upozorneni, "{soubor}: {:?}", n.upozorneni);
    }
    // poškozený soubor
    assert!(io::dbf_na_tabulku(b"nesmysl").is_err());
}

#[test]
fn aktualizace_zaka_nezavisle_na_poradi_jmena() {
    let p = vychozi();
    let mut rok = p.rok.clone();
    rok.studenti.push(Student {
        id: 99_999,
        jmeno: "Marek Brokl".into(),
        trida: "septimaB".into(),
        skupina_aj: SkupinaAj::A,
        pohlavi: Pohlavi::CH,
        volby: Default::default(),
        status: StavStudenta::Aktivni,
    });
    let n = io::nacti_text(&p.skola, Format::Csv, "Jméno;Třída;AJ\nBROKL Marek;7.B;AJb\n".as_bytes(), false).unwrap();
    let plan = io::naplanuj(&p.skola, &n.zaznamy, "");
    let v = io::aplikuj(&p.skola, &mut rok, &plan, Rezim::PridatAktualizovat);
    assert_eq!((v.pridano, v.aktualizovano), (0, 1));
    assert_eq!(rok.student(99_999).unwrap().skupina_aj, SkupinaAj::B);
    assert_eq!(io::klic_jmena("Hladík Jakub Antonín"), io::klic_jmena("Jakub Antonín Hladík"));
}
