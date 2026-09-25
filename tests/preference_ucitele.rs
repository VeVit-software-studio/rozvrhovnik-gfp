//! Preferované třídy učitele: solver jim přednostně dává lepší (ne brzké/pozdní) hodiny.

use rozvrhovnik::data::*;
use rozvrhovnik::solver::{self, Vysledek};
use std::collections::BTreeMap;
use std::sync::atomic::AtomicBool;

/// 2 třídy (A, B) po 15 h, všechno učí jeden učitel (30 h/týden). Dobrých slotů
/// (8:10–12:45) je jen 25, takže aspoň 5 hodin musí padnout na 7:20 nebo odpoledne.
fn mala_skola(preferovane: &[&str], vaha: u32) -> (Skola, SkolniRok) {
    let trida = |id: &str| Trida {
        id: id.into(),
        nazev: id.into(),
        rocnik: 8,
        typ: TypStudia::Osmilete,
        kmenova: format!("R{id}"),
        naslednik: None,
        omezeni_prepsat: None,
    };
    let mistnost = |id: &str| Mistnost { id: id.into(), nazev: id.into(), kapacita: 30, kmenova: true };
    let predmet = |id: &str| Predmet { id: id.into(), nazev: id.into(), specialni_mistnosti: vec![], kmenova_ok: true };
    let mut plan = Vec::new();
    for t in ["A", "B"] {
        for p in ["M1", "M2", "M3"] {
            plan.push(PlanovaHodina {
                trida: t.into(),
                predmet: p.into(),
                hodin: 5,
                struktura: Struktura::CelouTridou("T".into()),
                dvoj: false,
            });
        }
    }
    let mut nastaveni = Nastaveni { casovy_limit_s: 2, seed: 7, ..Default::default() };
    nastaveni.vahy.ucitel_preference = vaha;
    let skola = Skola {
        tridy: vec![trida("A"), trida("B")],
        mistnosti: vec![mistnost("RA"), mistnost("RB")],
        ucitele: vec![Ucitel {
            id: "T".into(),
            jmeno: "Učitel".into(),
            volno: vec![],
            max_den: 8,
            preferovana_mistnost: None,
            preferovane_tridy: preferovane.iter().map(|s| s.to_string()).collect(),
        }],
        predmety: vec![predmet("M1"), predmet("M2"), predmet("M3")],
        plan,
        nastaveni,
    };
    let mut rok = SkolniRok { label: "test".into(), studenti: vec![], menu: vec![], dalsi_id: 1 };
    for t in ["A", "B"] {
        for i in 0..3 {
            let id = rok.nove_id();
            rok.studenti.push(Student {
                id,
                jmeno: format!("{t}{i}"),
                trida: t.into(),
                skupina_aj: if i % 2 == 0 { SkupinaAj::A } else { SkupinaAj::B },
                pohlavi: if i % 2 == 0 { Pohlavi::D } else { Pohlavi::CH },
                volby: BTreeMap::new(),
                status: StavStudenta::Aktivni,
            });
        }
    }
    (skola, rok)
}

/// Počet brzkých (7:20) a odpoledních hodin třídy.
fn spatne_hodiny(v: &Vysledek, trida: &str, odpo_od: usize) -> usize {
    (0..v.lekce.len())
        .filter(|&i| v.lekce[i].tridy.iter().any(|t| t == trida))
        .map(|i| {
            let s = v.slot[i].expect("umístěno") as usize;
            (s..s + v.lekce[i].len as usize).filter(|t| t % SLOTU == 0 || t % SLOTU >= odpo_od).count()
        })
        .sum()
}

fn generuj(skola: &Skola, rok: &SkolniRok) -> Vysledek {
    solver::generuj(skola, rok, &BTreeMap::new(), &AtomicBool::new(false), None)
}

#[test]
fn preferovana_trida_dostava_lepsi_hodiny() {
    let (skola, rok) = mala_skola(&["A"], Vahy::default().ucitel_preference);
    let odpo_od = skola.nastaveni.odpoledne_od as usize;
    let v = generuj(&skola, &rok);
    assert!(v.problemy.is_empty(), "{:?}", v.problemy);
    let (a, b) = (spatne_hodiny(&v, "A", odpo_od), spatne_hodiny(&v, "B", odpo_od));
    assert!(a + b >= 5, "aspoň 5 hodin musí být brzy/odpoledne ({a} + {b})");

    let (skola0, rok0) = mala_skola(&["A"], 0);
    let v0 = generuj(&skola0, &rok0);
    assert!(v0.problemy.is_empty(), "{:?}", v0.problemy);
    let a0 = spatne_hodiny(&v0, "A", odpo_od);
    let b0 = spatne_hodiny(&v0, "B", odpo_od);
    eprintln!("brzké/odpolední hodiny – s preferencí A: A={a}, B={b}; bez (váha 0): A={a0}, B={b0}");

    assert!(a < b, "preferovaná A má mít méně brzkých/pozdních hodin než B: A={a}, B={b}");
    assert!(a <= a0, "preference nesmí A zhoršit oproti váze 0: {a} > {a0}");
}

#[test]
fn stary_json_bez_novych_poli_se_nacte() {
    let json = r#"{"id":"X","jmeno":"Y","volno":[],"max_den":7,"preferovana_mistnost":null}"#;
    let u: Ucitel = serde_json::from_str(json).unwrap();
    assert!(u.preferovane_tridy.is_empty());
    let v: Vahy = serde_json::from_str(
        r#"{"okno":5,"rana":3,"odpoledne_nizsi":1,"odpoledne_vyssi":2,"seminar_odpoledne":2,"rozlozeni":2,"ucitel_okno":1}"#,
    )
    .unwrap();
    assert_eq!(v.ucitel_preference, 0);
}
