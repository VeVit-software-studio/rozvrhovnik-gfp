//! Exporty rozvrhu: HTML k tisku (třídy / učitelé / osobní rozvrhy žáků), CSV, XML
//! a textový report pro příkazovou řádku.

use crate::data::*;
use crate::solver::{Tyden, Vysledek};
use crate::validation::Kontrola;
use std::fmt::Write;

/// Z jakého pohledu se rozvrh zobrazuje.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Pohled {
    Trida(String),
    Ucitel(String),
    Mistnost(String),
    Zak(u32),
}

impl Pohled {
    pub fn obsahuje(&self, v: &Vysledek, i: usize) -> bool {
        let l = &v.lekce[i];
        match self {
            Pohled::Trida(t) => l.tridy.contains(t),
            Pohled::Ucitel(u) => &l.ucitel == u,
            Pohled::Mistnost(m) => v.mistnost[i].as_deref() == Some(m.as_str()),
            Pohled::Zak(z) => l.zaci.contains(z),
        }
    }
}

/// Lekce, které v daném pohledu probíhají v buňce (den, slot) – včetně pokračování dvouhodinovek.
pub fn lekce_v_bunce(v: &Vysledek, pohled: &Pohled, den: usize, slot: usize) -> Vec<usize> {
    let t = den * SLOTU + slot;
    let mut out: Vec<usize> = (0..v.lekce.len())
        .filter(|&i| {
            v.slot[i].is_some_and(|s| (s as usize..s as usize + v.lekce[i].len as usize).contains(&t))
                && pohled.obsahuje(v, i)
        })
        .collect();
    out.sort_by(|&a, &b| {
        let (la, lb) = (&v.lekce[a], &v.lekce[b]);
        (la.tyden as u8, &la.skupina, &la.predmet).cmp(&(lb.tyden as u8, &lb.skupina, &lb.predmet))
    });
    out
}

pub fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

const CSS: &str = r#"
@page { size: A4 landscape; margin: 8mm; }
body { font-family: Arial, Helvetica, sans-serif; font-size: 10px; color: #111; }
.strana { page-break-after: always; break-after: page; }
.strana:last-child { page-break-after: auto; }
h2 { font-size: 15px; margin: 0 0 6px 0; }
table { border-collapse: collapse; width: 100%; table-layout: fixed; }
th, td { border: 1px solid #666; padding: 2px; vertical-align: top; }
td { height: 62px; }
th { background: #e8e8e8; font-weight: bold; }
th.den { width: 28px; }
.l { margin-bottom: 3px; line-height: 1.2; }
.p { font-weight: bold; font-size: 11px; }
.m { color: #444; }
.lt { color: #0a5; font-weight: bold; }
.pozn { margin-top: 6px; color: #555; }
"#;

fn bunka_html(skola: &Skola, v: &Vysledek, pohled: &Pohled, d: usize, k: usize) -> String {
    let mut s = String::new();
    for i in lekce_v_bunce(v, pohled, d, k) {
        let l = &v.lekce[i];
        let tyden = match l.tyden {
            Tyden::Oba => String::new(),
            t => format!("<span class=\"lt\">{}</span>", t.prefix()),
        };
        let skupina = if l.skupina.is_empty() { String::new() } else { format!(" {}", esc(&l.skupina)) };
        let mistnost = v.mistnost[i].clone().unwrap_or_else(|| "?".into());
        let druhy = match pohled {
            Pohled::Trida(_) | Pohled::Zak(_) => format!("{} · {}", esc(&l.ucitel), esc(&mistnost)),
            Pohled::Ucitel(_) => format!("{} · {}", esc(&tridy_kratce(skola, &l.tridy)), esc(&mistnost)),
            Pohled::Mistnost(_) => format!("{} · {}", esc(&tridy_kratce(skola, &l.tridy)), esc(&l.ucitel)),
        };
        let _ = write!(
            s,
            "<div class=\"l\">{}<span class=\"p\">{}</span>{}<br><span class=\"m\">{}</span></div>",
            tyden,
            esc(&l.predmet),
            skupina,
            druhy
        );
    }
    s
}

pub fn tridy_kratce(skola: &Skola, tridy: &[String]) -> String {
    tridy.iter().map(|t| skola.nazev_tridy(t)).collect::<Vec<_>>().join("+")
}

fn tabulka_html(skola: &Skola, v: &Vysledek, pohled: &Pohled) -> String {
    let mut s = String::from("<table><tr><th class=\"den\"></th>");
    for k in 0..SLOTU {
        let _ = write!(s, "<th>{}.<br>{}–{}</th>", k, CASY[k], CASY_KONEC[k]);
    }
    s.push_str("</tr>");
    for d in 0..DNY {
        let _ = write!(s, "<tr><th class=\"den\">{}</th>", NAZVY_DNU[d]);
        for k in 0..SLOTU {
            let _ = write!(s, "<td>{}</td>", bunka_html(skola, v, pohled, d, k));
        }
        s.push_str("</tr>");
    }
    s.push_str("</table>");
    s
}

/// Obecný tisk: jedna stránka A4 na šířku pro každý pohled.
pub fn html_stranky(skola: &Skola, v: &Vysledek, titulek: &str, stranky: &[(String, Pohled)]) -> String {
    let mut s = format!(
        "<!doctype html><html lang=\"cs\"><head><meta charset=\"utf-8\"><title>{}</title><style>{}</style></head><body>",
        esc(titulek),
        CSS
    );
    for (nadpis, pohled) in stranky {
        let _ = write!(
            s,
            "<div class=\"strana\"><h2>{}</h2>{}<div class=\"pozn\">L· = lichý týden, S· = sudý týden · Gymnázium Františka Palackého Neratovice · {}</div></div>",
            esc(nadpis),
            tabulka_html(skola, v, pohled),
            esc(&v.rok)
        );
    }
    s.push_str("</body></html>");
    s
}

pub fn html_tridy(skola: &Skola, v: &Vysledek) -> String {
    let stranky: Vec<(String, Pohled)> = skola
        .tridy
        .iter()
        .map(|t| (format!("Třída {} – rozvrh {}", t.nazev, v.rok), Pohled::Trida(t.id.clone())))
        .collect();
    html_stranky(skola, v, &format!("Rozvrh tříd {}", v.rok), &stranky)
}

pub fn html_ucitele(skola: &Skola, v: &Vysledek) -> String {
    let stranky: Vec<(String, Pohled)> = skola
        .ucitele
        .iter()
        .filter(|u| v.lekce.iter().any(|l| l.ucitel == u.id))
        .map(|u| (format!("{} – {} – rozvrh {}", u.id, u.jmeno, v.rok), Pohled::Ucitel(u.id.clone())))
        .collect();
    html_stranky(skola, v, &format!("Rozvrh učitelů {}", v.rok), &stranky)
}

pub fn html_zaci(skola: &Skola, rok: &SkolniRok, v: &Vysledek) -> String {
    let mut zaci: Vec<&Student> = rok.studenti.iter().filter(|s| s.v_rozvrhu()).collect();
    let poradi_tridy = |t: &str| skola.tridy.iter().position(|x| x.id == t).unwrap_or(usize::MAX);
    zaci.sort_by(|a, b| (poradi_tridy(&a.trida), &a.jmeno).cmp(&(poradi_tridy(&b.trida), &b.jmeno)));
    let stranky: Vec<(String, Pohled)> = zaci
        .iter()
        .map(|s| {
            (
                format!("{} – {} ({}) – rozvrh {}", s.jmeno, skola.nazev_tridy(&s.trida), s.skupina_aj.nazev(), v.rok),
                Pohled::Zak(s.id),
            )
        })
        .collect();
    html_stranky(skola, v, &format!("Osobní rozvrhy žáků {}", v.rok), &stranky)
}

fn csv_pole(s: &str) -> String {
    if s.contains(';') || s.contains('"') || s.contains('\n') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

/// CSV (UTF-8 s BOM, středníky – rovnou do Excelu). Jeden řádek = třída × hodina.
pub fn csv(skola: &Skola, v: &Vysledek) -> String {
    let mut s = String::from("\u{FEFF}trida;den;slot;cas;predmet;ucitel;mistnost;skupina;pocet_zaku;tyden\r\n");
    for t in &skola.tridy {
        for d in 0..DNY {
            for k in 0..SLOTU {
                for i in lekce_v_bunce(v, &Pohled::Trida(t.id.clone()), d, k) {
                    let l = &v.lekce[i];
                    let tyden = match l.tyden {
                        Tyden::Oba => "",
                        Tyden::L => "L",
                        Tyden::S => "S",
                    };
                    let pole = [
                        t.id.clone(),
                        NAZVY_DNU[d].to_string(),
                        k.to_string(),
                        CASY[k].to_string(),
                        l.predmet.clone(),
                        l.ucitel.clone(),
                        v.mistnost[i].clone().unwrap_or_default(),
                        l.skupina.clone(),
                        v.pocet_realnych_zaku(i).to_string(),
                        tyden.to_string(),
                    ];
                    let radek: Vec<String> = pole.iter().map(|x| csv_pole(x)).collect();
                    s.push_str(&radek.join(";"));
                    s.push_str("\r\n");
                }
            }
        }
    }
    s
}

pub fn xml(skola: &Skola, v: &Vysledek) -> String {
    let mut s = format!("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<rozvrh rok=\"{}\">\n", esc(&v.rok));
    for t in &skola.tridy {
        let _ = writeln!(s, "  <trida id=\"{}\" nazev=\"{}\">", esc(&t.id), esc(&t.nazev));
        for d in 0..DNY {
            let _ = writeln!(s, "    <den index=\"{}\" nazev=\"{}\">", d, NAZVY_DNU[d]);
            for k in 0..SLOTU {
                for i in lekce_v_bunce(v, &Pohled::Trida(t.id.clone()), d, k) {
                    let l = &v.lekce[i];
                    let tyden = match l.tyden {
                        Tyden::Oba => "",
                        Tyden::L => "L",
                        Tyden::S => "S",
                    };
                    let _ = writeln!(
                        s,
                        "      <hodina slot=\"{}\" cas=\"{}\" predmet=\"{}\" ucitel=\"{}\" mistnost=\"{}\" skupina=\"{}\" tyden=\"{}\" zaku=\"{}\"/>",
                        k,
                        CASY[k],
                        esc(&l.predmet),
                        esc(&l.ucitel),
                        esc(&v.mistnost[i].clone().unwrap_or_default()),
                        esc(&l.skupina),
                        tyden,
                        v.pocet_realnych_zaku(i)
                    );
                }
            }
            s.push_str("    </den>\n");
        }
        s.push_str("  </trida>\n");
    }
    s.push_str("</rozvrh>\n");
    s
}

/// Textový report (příkazová řádka).
pub fn text_reportu(skola: &Skola, v: &Vysledek, k: &Kontrola, zateze: &[(String, f32)]) -> String {
    let mut s = String::new();
    let _ = writeln!(s, "=== Rozvrh {} ===", v.rok);
    let _ = writeln!(
        s,
        "Lekcí: {} · skóre tvrdé/měkké: {}/{} · iterací: {} · čas: {:.1} s",
        v.lekce.len(),
        v.skore.0,
        v.skore.1,
        v.iteraci,
        v.cas_s
    );
    let _ = writeln!(s, "\n--- Varování ({}) ---", v.varovani.len());
    for x in &v.varovani {
        let _ = writeln!(s, "  ⚠ {}", x);
    }
    let _ = writeln!(s, "\n--- Problémy ({}) ---", k.problemy.len());
    for x in &k.problemy {
        let _ = writeln!(s, "  ✖ {}", x);
    }
    let _ = writeln!(s, "\n--- Statistiky tříd ---");
    let _ = writeln!(
        s,
        "  {:<12} {:>5} {:>9} {:>6} {:>6} {:>6} {:>6}",
        "třída", "žáků", "hodin", "7:20", "odpo", "blok", "okna"
    );
    for st in &k.statistiky {
        let _ = writeln!(
            s,
            "  {:<12} {:>5} {:>4}–{:<4} {:>6} {:>6} {:>6} {:>6.1}",
            skola.nazev_tridy(&st.trida),
            st.zaku,
            st.hodin_min,
            st.hodin_max,
            st.rane_dny,
            st.odpoledni,
            st.nejdelsi_blok,
            st.okna_prumer
        );
    }
    let _ = writeln!(s, "\n--- Úvazky učitelů (h/týden) ---");
    for (u, h) in zateze {
        let sem = if *h >= 31.0 {
            " ‼"
        } else if *h >= 26.0 {
            " !"
        } else {
            ""
        };
        let _ = writeln!(s, "  {:<6} {:>5.1}{}", u, h, sem);
    }
    s
}
