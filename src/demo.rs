//! Demo data pro vyzkoušení importů a bloků seminářů: smyšlení žáci s běžnými českými jmény,
//! semináře zvolené náhodně (jak to studenti dělají – bez ohledu na bloky) a soubory ve formátu
//! školy (seznam žáků, výstup nástroje na semináře). Jména jsou náhodné kombinace – nejde o skutečné osoby.

use crate::data::*;
use crate::rok;
use crate::studenti_io;
use rand::{rngs::StdRng, Rng, SeedableRng};
use std::collections::BTreeSet;
use std::path::Path;

const JMENA_D: [&str; 40] = [
    "Anna",
    "Eliška",
    "Tereza",
    "Adéla",
    "Karolína",
    "Natálie",
    "Kristýna",
    "Barbora",
    "Lucie",
    "Veronika",
    "Klára",
    "Michaela",
    "Nikola",
    "Kateřina",
    "Denisa",
    "Markéta",
    "Simona",
    "Aneta",
    "Julie",
    "Sofie",
    "Viktorie",
    "Ema",
    "Magdaléna",
    "Anežka",
    "Linda",
    "Zuzana",
    "Petra",
    "Hana",
    "Monika",
    "Beáta",
    "Kamila",
    "Šárka",
    "Vanesa",
    "Nela",
    "Rozálie",
    "Laura",
    "Emma",
    "Dominika",
    "Pavla",
    "Jana",
];
const JMENA_CH: [&str; 40] = [
    "Jan",
    "Jakub",
    "Tomáš",
    "Adam",
    "Matěj",
    "Vojtěch",
    "Ondřej",
    "David",
    "Lukáš",
    "Filip",
    "Martin",
    "Daniel",
    "Marek",
    "Michal",
    "Petr",
    "Dominik",
    "Šimon",
    "Štěpán",
    "Matyáš",
    "Tadeáš",
    "Jiří",
    "Josef",
    "Václav",
    "Radim",
    "Lumír",
    "Vít",
    "Patrik",
    "Kryštof",
    "Antonín",
    "Robert",
    "Richard",
    "Samuel",
    "Oliver",
    "František",
    "Pavel",
    "Aleš",
    "Zdeněk",
    "Karel",
    "Viktor",
    "Denis",
];
/// (mužský, ženský tvar)
const PRIJMENI: [(&str, &str); 50] = [
    ("Novák", "Nováková"),
    ("Svoboda", "Svobodová"),
    ("Dvořák", "Dvořáková"),
    ("Černý", "Černá"),
    ("Procházka", "Procházková"),
    ("Kučera", "Kučerová"),
    ("Veselý", "Veselá"),
    ("Horák", "Horáková"),
    ("Němec", "Němcová"),
    ("Pospíšil", "Pospíšilová"),
    ("Pokorný", "Pokorná"),
    ("Hájek", "Hájková"),
    ("Král", "Králová"),
    ("Jelínek", "Jelínková"),
    ("Růžička", "Růžičková"),
    ("Beneš", "Benešová"),
    ("Fiala", "Fialová"),
    ("Sedláček", "Sedláčková"),
    ("Doležal", "Doležalová"),
    ("Zeman", "Zemanová"),
    ("Kolář", "Kolářová"),
    ("Navrátil", "Navrátilová"),
    ("Čermák", "Čermáková"),
    ("Vaněk", "Vaňková"),
    ("Urban", "Urbanová"),
    ("Blažek", "Blažková"),
    ("Kříž", "Křížová"),
    ("Kovář", "Kovářová"),
    ("Kratochvíl", "Kratochvílová"),
    ("Bartoš", "Bartošová"),
    ("Vlček", "Vlčková"),
    ("Polák", "Poláková"),
    ("Musil", "Musilová"),
    ("Kopecký", "Kopecká"),
    ("Šimek", "Šimková"),
    ("Konečný", "Konečná"),
    ("Malý", "Malá"),
    ("Holub", "Holubová"),
    ("Štěpánek", "Štěpánková"),
    ("Kadlec", "Kadlecová"),
    ("Staněk", "Staňková"),
    ("Dostál", "Dostálová"),
    ("Soukup", "Soukupová"),
    ("Šťastný", "Šťastná"),
    ("Mareš", "Marešová"),
    ("Moravec", "Moravcová"),
    ("Sýkora", "Sýkorová"),
    ("Tichý", "Tichá"),
    ("Valenta", "Valentová"),
    ("Machala", "Machalová"),
];

/// Výchozí projekt se smyšlenými jmény žáků a náhodně zvolenými semináři.
pub fn projekt(seed: u64) -> Projekt {
    let mut p = vychozi();
    let mut rng = StdRng::seed_from_u64(seed);
    let mut pouzita: BTreeSet<String> = BTreeSet::new();
    for s in p.rok.studenti.iter_mut() {
        s.jmeno = loop {
            let (m, z) = PRIJMENI[rng.gen_range(0..PRIJMENI.len())];
            let j = match s.pohlavi {
                Pohlavi::D => format!("{} {}", JMENA_D[rng.gen_range(0..JMENA_D.len())], z),
                Pohlavi::CH => format!("{} {}", JMENA_CH[rng.gen_range(0..JMENA_CH.len())], m),
            };
            if pouzita.insert(format!("{}|{}", s.trida, j)) {
                break j;
            }
        };
    }
    // AJ skupiny se ve škole dělí ručně (rozřazovací testy) – rozhodíme je znovu, pohlaví zůstává
    for t in p.skola.tridy.clone() {
        rok::rozdel_tridu(&mut p.rok, &t.id);
    }
    // semináře: většina žáků volí „rozumně“ (jeden seminář z každé skupiny nabídky), ~15 % náhodně –
    // jak to ve škole bývá; bloky pak ukáží, kdo brání menšímu počtu bloků
    for m in p.rok.menu.clone().iter().filter(|m| m.pocet_voleb > 1) {
        let obliba: Vec<u32> = m.volby.iter().map(|_| rng.gen_range(1..=4)).collect();
        let k = (m.pocet_voleb as usize).min(m.volby.len());
        for s in p.rok.studenti.iter_mut().filter(|s| m.tridy.contains(&s.trida)) {
            let mut volby: Vec<String> = Vec::new();
            if rng.gen_bool(0.85) {
                for b in 0..k {
                    let skupina: Vec<usize> = (0..m.volby.len()).filter(|i| i % k == b).collect();
                    let i = skupina[rng.gen_range(0..skupina.len())];
                    volby.push(m.volby[i].id.clone());
                }
            }
            while volby.len() < k {
                let celkem: u32 = obliba.iter().sum();
                let mut x = rng.gen_range(0..celkem);
                let i = obliba
                    .iter()
                    .position(|&w| {
                        if x < w {
                            true
                        } else {
                            x -= w;
                            false
                        }
                    })
                    .unwrap();
                if !volby.contains(&m.volby[i].id) {
                    volby.push(m.volby[i].id.clone());
                }
            }
            s.volby.insert(m.id.clone(), volby);
        }
    }
    p
}

/// „Příjmení Jméno“ jako ve výstupech školy.
fn prijmeni_jmeno(jmeno: &str) -> String {
    match jmeno.split_once(' ') {
        Some((j, p)) => format!("{p} {j}"),
        None => jmeno.to_string(),
    }
}

/// Název třídy ve formátu školy: alias (7.A), jinak název.
fn trida_skoly(skola: &Skola, id: &str) -> String {
    skola.trida(id).map_or(id.to_string(), |t| t.aliasy().next().map_or(t.nazev.clone(), str::to_string))
}

/// Výstup ve formátu nástroje na semináře:
/// Ročník;Seminář;Mezitřídní;Třídy;Počet žáků;Jméno žáka;Třída žáka
pub fn seminare_csv(skola: &Skola, rok: &SkolniRok, menu: &Menu, rocnik: &str) -> String {
    let mut s = String::from("\u{FEFF}Ročník;Seminář;Mezitřídní;Třídy;Počet žáků;Jméno žáka;Třída žáka\r\n");
    for v in &menu.volby {
        let mut zaci: Vec<&Student> = rok
            .studenti
            .iter()
            .filter(|s| menu.tridy.contains(&s.trida) && s.volby.get(&menu.id).is_some_and(|x| x.contains(&v.id)))
            .collect();
        zaci.sort_by_key(|z| (trida_skoly(skola, &z.trida), prijmeni_jmeno(&z.jmeno)));
        let tridy: BTreeSet<String> = zaci.iter().map(|z| trida_skoly(skola, &z.trida)).collect();
        let mezitridni = if tridy.len() > 1 { "ano" } else { "" };
        let tridy = tridy.into_iter().collect::<Vec<_>>().join(", ");
        for (k, z) in zaci.iter().enumerate() {
            let hlava = if k == 0 {
                format!("{rocnik};{};{mezitridni};\"{tridy}\";{}", v.id, zaci.len())
            } else {
                ";;;;".to_string()
            };
            s.push_str(&format!("{hlava};{};{}\r\n", prijmeni_jmeno(&z.jmeno), trida_skoly(skola, &z.trida)));
        }
    }
    s
}

/// Zapíše demo soubory do adresáře. Vrací seznam zapsaných souborů.
pub fn zapis(adresar: &Path, seed: u64) -> Result<Vec<String>, String> {
    std::fs::create_dir_all(adresar).map_err(|e| format!("Nelze vytvořit {}: {e}", adresar.display()))?;
    let p = projekt(seed);
    let mut zapsano = Vec::new();
    // seznam žáků (jako export z evidence): Příjmení;Jméno;Třída;Pohlaví – bez voleb
    let mut zaci = String::from("\u{FEFF}Příjmení;Jméno;Třída;Pohlaví\r\n");
    let poradi = |t: &str| p.skola.tridy.iter().position(|x| x.id == t).unwrap_or(usize::MAX);
    let mut vsichni: Vec<&Student> = p.rok.studenti.iter().collect();
    vsichni.sort_by_key(|s| (poradi(&s.trida), prijmeni_jmeno(&s.jmeno)));
    for s in &vsichni {
        let (j, pr) = s.jmeno.split_once(' ').unwrap_or((&s.jmeno, ""));
        zaci.push_str(&format!("{pr};{j};{};{}\r\n", trida_skoly(&p.skola, &s.trida), s.pohlavi.zkratka()));
    }
    let mut zapis = |nazev: &str, obsah: String| -> Result<(), String> {
        let c = adresar.join(nazev);
        std::fs::write(&c, obsah).map_err(|e| format!("Nelze zapsat {}: {e}", c.display()))?;
        zapsano.push(c.display().to_string());
        Ok(())
    };
    zapis("zaci.csv", zaci)?;
    for (menu, rocnik) in [("sem-7", "Septima + třeťák"), ("sem-8", "Oktáva + čtvrťák")] {
        if let Some(m) = p.rok.menu.iter().find(|m| m.id == menu) {
            zapis(&format!("seminare_{}.csv", menu), seminare_csv(&p.skola, &p.rok, m, rocnik))?;
        }
    }
    // celý projekt (lze rovnou otevřít: rozvrhovnik demo_projekt.json)
    let c = adresar.join("demo_projekt.json");
    p.uloz(&c)?;
    zapsano.push(c.display().to_string());
    // kompletní seznam i s volbami (formát exportu aplikace) – Excel
    let c = adresar.join("zaci_s_volbami.xlsx");
    studenti_io::exportuj(&p.skola, &p.rok, &vsichni, studenti_io::Format::Xlsx, &c, Default::default())?;
    zapsano.push(c.display().to_string());
    Ok(zapsano)
}
