//! Odvozená data pro profily učitelů a předmětů: kdo co učí, ve kterých třídách a kolik hodin.
//! Vše se počítá z učebního plánu a katalogů voleb – nic se neduplikuje.

use crate::data::*;
use crate::rok;

/// Jedna položka výuky: učitel × předmět × třída/skupina.
#[derive(Clone, Debug, PartialEq)]
pub struct Vyuka {
    pub ucitel: String,
    pub predmet: String,
    /// Třídy (u plánu jedna, u volitelných všechny třídy menu).
    pub tridy: Vec<String>,
    /// Skupina: "", "AJa", "D", "L", "AJa L" … nebo název volby.
    pub skupina: String,
    /// Hodin týdně (L/S hodina = 0,5).
    pub hodin: f32,
    /// Název menu, jde-li o volitelný předmět.
    pub menu: Option<String>,
    /// Počet zapsaných žáků (jen u voleb).
    pub zaku: Option<usize>,
}

/// Všechna výuka školy z plánu a z menu voleb.
pub fn vyuka(skola: &Skola, rok: &SkolniRok) -> Vec<Vyuka> {
    let mut out = Vec::new();
    for h in &skola.plan {
        let mut pridej = |ucitel: &str, predmet: &str, skupina: &str, hodin: f32| {
            out.push(Vyuka {
                ucitel: ucitel.to_string(),
                predmet: predmet.to_string(),
                tridy: vec![h.trida.clone()],
                skupina: skupina.to_string(),
                hodin,
                menu: None,
                zaku: None,
            })
        };
        let hod = h.hodin as f32;
        match &h.struktura {
            Struktura::CelouTridou(u) => pridej(u, &h.predmet, "", hod),
            Struktura::Rozdeleno { a, b } => {
                pridej(a, &h.predmet, "AJa", hod);
                pridej(b, &h.predmet, "AJb", hod);
            }
            Struktura::Pohlavi { div, chl } => {
                pridej(div, &h.predmet, "D", hod);
                pridej(chl, &h.predmet, "CH", hod);
            }
            Struktura::Stridave { l, s } => {
                pridej(&l.ucitel, &l.predmet, "L", hod / 2.0);
                pridej(&s.ucitel, &s.predmet, "S", hod / 2.0);
            }
            Struktura::StridaveSkupiny { a, b } => {
                for (g, pary) in [("AJa", a), ("AJb", b)] {
                    pridej(&pary[0].ucitel, &pary[0].predmet, &format!("{g} L"), hod / 2.0);
                    pridej(&pary[1].ucitel, &pary[1].predmet, &format!("{g} S"), hod / 2.0);
                }
            }
        }
    }
    for m in &rok.menu {
        let obs = rok::obsazenost(rok, m);
        for v in &m.volby {
            out.push(Vyuka {
                ucitel: v.ucitel.clone(),
                predmet: v.predmet.clone(),
                tridy: m.tridy.clone(),
                skupina: v.nazev.clone(),
                hodin: m.hodin_tydne as f32,
                menu: Some(m.nazev.clone()),
                zaku: obs.get(&v.id).copied(),
            });
        }
    }
    out
}

/// Třídy v pořadí číselníku, které se vyskytují v položkách výuky.
pub fn tridy_vyuky<'a>(tridy: &[Trida], polozky: impl IntoIterator<Item = &'a Vyuka>) -> Vec<String> {
    let mut vsechny: Vec<String> = polozky.into_iter().flat_map(|v| v.tridy.iter().cloned()).collect();
    vsechny.sort_by_key(|t| tridy.iter().position(|x| &x.id == t).unwrap_or(usize::MAX));
    vsechny.dedup();
    vsechny
}
