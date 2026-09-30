//! Bloky seminářů: které semináře mohou běžet současně („proti sobě“).
//!
//! Dva semináře spolu kolidují (nesmí být ve stejném bloku), když
//! - mají společného žáka,
//! - učí je stejný učitel (i jen v jedné z hodin),
//! - potřebují tutéž jedinou odbornou učebnu (např. dva semináře jen v PIF).
//!
//! Návrh bloků = obarvení grafu kolizí nejmenším počtem barev (DSatur + přesné prohledávání
//! s časovým limitem), pak vyrovnání počtu žáků v blocích. Volby jednoho bloku pak řešič
//! plánuje paralelně (jedna jednotka).

use crate::data::*;
use crate::studenti_io::norm;
use std::collections::{BTreeMap, BTreeSet};
use std::time::{Duration, Instant};

#[derive(Clone, Debug, PartialEq)]
pub enum Duvod {
    /// Počet společných žáků a jejich jména.
    Zaci(Vec<String>),
    Ucitel(String),
    Mistnost(String),
}

impl Duvod {
    pub fn popis(&self) -> String {
        match self {
            Duvod::Zaci(z) => {
                let ukazka: Vec<&str> = z.iter().take(4).map(String::as_str).collect();
                let dalsi = if z.len() > 4 { format!(" a další {}", z.len() - 4) } else { String::new() };
                format!("{} společných žáků ({}{})", z.len(), ukazka.join(", "), dalsi)
            }
            Duvod::Ucitel(u) => format!("stejný učitel {u}"),
            Duvod::Mistnost(m) => format!("obě jen v učebně {m}"),
        }
    }
}

/// Graf kolizí voleb jednoho menu.
#[derive(Clone, Debug, Default)]
pub struct Analyza {
    pub menu_id: String,
    /// id volby
    pub volby: Vec<String>,
    pub nazvy: Vec<String>,
    /// počet žáků volby
    pub pocty: Vec<usize>,
    /// id žáků volby
    pub zaci: Vec<BTreeSet<u32>>,
    /// důvody kolize pro dvojici (i < j)
    pub kolize: BTreeMap<(usize, usize), Vec<Duvod>>,
    /// Nejvíc voleb jednoho žáka v tomto menu (dolní mez počtu bloků).
    pub max_voleb_zaka: usize,
    /// Žáci tříd menu, kteří v menu mají aspoň jednu volbu.
    pub zaku_celkem: usize,
}

impl Analyza {
    pub fn n(&self) -> usize {
        self.volby.len()
    }
    pub fn koliduje(&self, i: usize, j: usize) -> bool {
        i != j && self.kolize.contains_key(&(i.min(j), i.max(j)))
    }
    pub fn duvody(&self, i: usize, j: usize) -> &[Duvod] {
        self.kolize.get(&(i.min(j), i.max(j))).map_or(&[], |v| v.as_slice())
    }
    pub fn spolecnych_zaku(&self, i: usize, j: usize) -> usize {
        self.duvody(i, j)
            .iter()
            .find_map(|d| match d {
                Duvod::Zaci(z) => Some(z.len()),
                _ => None,
            })
            .unwrap_or(0)
    }
    pub fn index(&self, volba: &str) -> Option<usize> {
        self.volby.iter().position(|v| v == volba)
    }
}

/// Sestaví graf kolizí pro menu.
pub fn analyzuj(skola: &Skola, rok: &SkolniRok, menu: &Menu) -> Analyza {
    let n = menu.volby.len();
    let mut a = Analyza {
        menu_id: menu.id.clone(),
        volby: menu.volby.iter().map(|v| v.id.clone()).collect(),
        nazvy: menu.volby.iter().map(|v| v.nazev.clone()).collect(),
        pocty: vec![0; n],
        zaci: vec![BTreeSet::new(); n],
        ..Default::default()
    };
    let jmena: BTreeMap<u32, String> = rok.studenti.iter().map(|s| (s.id, s.jmeno.clone())).collect();
    for s in rok.studenti.iter().filter(|s| s.v_rozvrhu() && menu.tridy.contains(&s.trida)) {
        let Some(vol) = s.volby.get(&menu.id) else { continue };
        let mut pocet = 0;
        for v in vol {
            if let Some(i) = a.index(v) {
                a.zaci[i].insert(s.id);
                pocet += 1;
            }
        }
        if pocet > 0 {
            a.zaku_celkem += 1;
        }
        a.max_voleb_zaka = a.max_voleb_zaka.max(pocet);
    }
    for i in 0..n {
        a.pocty[i] = a.zaci[i].len();
    }
    let hodin = crate::solver::rozpis(menu.hodin_tydne, menu.dvojhodina).len().max(1);
    let ucitele: Vec<BTreeSet<String>> = menu
        .volby
        .iter()
        .map(|v| (0..hodin).map(|h| v.ucitel_hodiny(h).to_string()).filter(|u| !u.is_empty()).collect())
        .collect();
    // jediná povinná odborná učebna předmětu (bez možnosti kmenové)
    let jedina_ucebna: Vec<Option<String>> = menu
        .volby
        .iter()
        .map(|v| {
            skola
                .predmet(&v.predmet)
                .filter(|p| !p.kmenova_ok && p.specialni_mistnosti.len() == 1)
                .map(|p| p.specialni_mistnosti[0].clone())
        })
        .collect();
    for i in 0..n {
        for j in i + 1..n {
            let mut d = Vec::new();
            let spolecni: Vec<String> =
                a.zaci[i].intersection(&a.zaci[j]).map(|z| jmena.get(z).cloned().unwrap_or_default()).collect();
            if !spolecni.is_empty() {
                d.push(Duvod::Zaci(spolecni));
            }
            if let Some(u) = ucitele[i].intersection(&ucitele[j]).next() {
                d.push(Duvod::Ucitel(u.clone()));
            }
            if let (Some(x), Some(y)) = (&jedina_ucebna[i], &jedina_ucebna[j]) {
                if x == y {
                    d.push(Duvod::Mistnost(x.clone()));
                }
            }
            if !d.is_empty() {
                a.kolize.insert((i, j), d);
            }
        }
    }
    a
}

// ───────────────────────────── kontrola přiřazení ─────────────────────────────

#[derive(Clone, Debug, PartialEq)]
pub struct KolizeVBloku {
    pub blok: u8,
    pub a: usize,
    pub b: usize,
    pub duvody: Vec<Duvod>,
}

/// Kolize uvnitř bloků (bloky: číslo bloku pro každou volbu v pořadí `Analyza::volby`).
pub fn kontrola(a: &Analyza, bloky: &[Option<u8>]) -> Vec<KolizeVBloku> {
    let mut out = Vec::new();
    for ((i, j), d) in &a.kolize {
        if let (Some(x), Some(y)) = (bloky[*i], bloky[*j]) {
            if x == y {
                out.push(KolizeVBloku { blok: x, a: *i, b: *j, duvody: d.clone() });
            }
        }
    }
    out.sort_by_key(|k| (k.blok, k.a, k.b));
    out
}

/// Pro volbu `i`: do kterých bloků (1..=max+1) ji lze dát a s čím by kolidovala.
pub fn kam_muze(a: &Analyza, bloky: &[Option<u8>], i: usize) -> Vec<(u8, Vec<(usize, Vec<Duvod>)>)> {
    let max = bloky.iter().flatten().copied().max().unwrap_or(0);
    (1..=max + 1)
        .map(|b| {
            let kolize: Vec<(usize, Vec<Duvod>)> = (0..a.n())
                .filter(|&j| j != i && bloky[j] == Some(b) && a.koliduje(i, j))
                .map(|j| (j, a.duvody(i, j).to_vec()))
                .collect();
            (b, kolize)
        })
        .collect()
}

/// Souhrn bloku: žáci v bloku a žáci menu, kteří v bloku nemají žádný seminář (mají volno).
pub fn obsazeni_bloku(a: &Analyza, bloky: &[Option<u8>], blok: u8) -> (usize, usize) {
    let v_bloku: BTreeSet<u32> =
        (0..a.n()).filter(|&i| bloky[i] == Some(blok)).flat_map(|i| a.zaci[i].iter().copied()).collect();
    let vsichni: BTreeSet<u32> = a.zaci.iter().flatten().copied().collect();
    (v_bloku.len(), vsichni.len() - v_bloku.len())
}

// ───────────────────────────── návrh bloků ─────────────────────────────

/// Největší klika grafu kolizí = semináře, které se navzájem vylučují (každý musí být v jiném
/// bloku) – dolní mez počtu bloků. Přesně pro malé grafy, v časovém limitu jinak nejlepší nalezená.
pub fn nejvetsi_klika(a: &Analyza, limit: Duration) -> Vec<usize> {
    let n = a.n();
    let konec = Instant::now() + limit;
    let mut nejlepsi: Vec<usize> = if n > 0 { vec![0] } else { vec![] };
    fn rozsir(a: &Analyza, klika: &mut Vec<usize>, kandidati: Vec<usize>, nejlepsi: &mut Vec<usize>, konec: Instant) {
        if klika.len() > nejlepsi.len() {
            *nejlepsi = klika.clone();
        }
        if klika.len() + kandidati.len() <= nejlepsi.len() || Instant::now() > konec {
            return;
        }
        for (k, &v) in kandidati.iter().enumerate() {
            if klika.len() + (kandidati.len() - k) <= nejlepsi.len() {
                return;
            }
            let dalsi: Vec<usize> = kandidati[k + 1..].iter().copied().filter(|&w| a.koliduje(v, w)).collect();
            klika.push(v);
            rozsir(a, klika, dalsi, nejlepsi, konec);
            klika.pop();
        }
    }
    let mut poradi: Vec<usize> = (0..n).collect();
    poradi.sort_by_key(|&i| std::cmp::Reverse((0..n).filter(|&j| a.koliduje(i, j)).count()));
    rozsir(a, &mut Vec::new(), poradi, &mut nejlepsi, konec);
    nejlepsi.sort_unstable();
    nejlepsi
}

/// Heuristika DSatur: vrací barvy 0..k.
fn dsatur(a: &Analyza, pevne: &[Option<usize>]) -> Vec<usize> {
    let n = a.n();
    let mut barva: Vec<Option<usize>> = pevne.to_vec();
    let stupen: Vec<usize> = (0..n).map(|i| (0..n).filter(|&j| a.koliduje(i, j)).count()).collect();
    while let Some(v) = (0..n).filter(|&i| barva[i].is_none()).max_by_key(|&i| {
        let sat: BTreeSet<usize> = (0..n).filter(|&j| a.koliduje(i, j)).filter_map(|j| barva[j]).collect();
        (sat.len(), stupen[i], a.pocty[i])
    }) {
        let zakazane: BTreeSet<usize> = (0..n).filter(|&j| a.koliduje(v, j)).filter_map(|j| barva[j]).collect();
        barva[v] = Some((0..).find(|c| !zakazane.contains(c)).unwrap());
    }
    barva.into_iter().map(|b| b.unwrap_or(0)).collect()
}

/// Přesný test obarvitelnosti k barvami (s pevnými barvami), s časovým limitem.
fn obarvi_k(a: &Analyza, k: usize, pevne: &[Option<usize>], konec: Instant) -> Option<Vec<usize>> {
    let n = a.n();
    let mut barva: Vec<Option<usize>> = pevne.to_vec();
    for i in 0..n {
        if let Some(c) = barva[i] {
            if c >= k || (0..n).any(|j| j != i && barva[j] == Some(c) && a.koliduje(i, j)) {
                return None;
            }
        }
    }
    let ma_pevne = pevne.iter().any(|p| p.is_some());
    fn krok(a: &Analyza, k: usize, barva: &mut Vec<Option<usize>>, symetrie: bool, konec: Instant) -> bool {
        if Instant::now() > konec {
            return false;
        }
        let n = a.n();
        // nejvíc omezený neobarvený vrchol
        let mut vyber: Option<(usize, usize, usize)> = None; // (sat, stupeň, v)
        for i in (0..n).filter(|&i| barva[i].is_none()) {
            let sat: BTreeSet<usize> = (0..n).filter(|&j| a.koliduje(i, j)).filter_map(|j| barva[j]).collect();
            let st = (0..n).filter(|&j| barva[j].is_none() && a.koliduje(i, j)).count();
            if vyber.is_none_or(|(s, d, _)| (sat.len(), st) > (s, d)) {
                vyber = Some((sat.len(), st, i));
            }
        }
        let Some((_, _, v)) = vyber else { return true };
        let pouzite = barva.iter().flatten().copied().max().map_or(0, |m| m + 1);
        let horni = if symetrie { (pouzite + 1).min(k) } else { k };
        for c in 0..horni {
            if (0..n).any(|j| barva[j] == Some(c) && a.koliduje(v, j)) {
                continue;
            }
            barva[v] = Some(c);
            if krok(a, k, barva, symetrie, konec) {
                return true;
            }
            barva[v] = None;
        }
        false
    }
    krok(a, k, &mut barva, !ma_pevne, konec).then(|| barva.into_iter().map(|b| b.unwrap()).collect())
}

/// Vyrovná počty žáků v blocích (minimalizuje součet čtverců) přesuny volby do jiného platného bloku.
fn vyrovnej(a: &Analyza, barvy: &mut [usize], k: usize, pevne: &[Option<usize>]) {
    let n = a.n();
    let soucet = |barvy: &[usize]| -> i64 {
        (0..k)
            .map(|c| {
                let s: i64 = (0..n).filter(|&i| barvy[i] == c).map(|i| a.pocty[i] as i64).sum();
                s * s
            })
            .sum()
    };
    let mut zlepseni = true;
    let mut kolo = 0;
    while zlepseni && kolo < 100 {
        zlepseni = false;
        kolo += 1;
        for v in (0..n).filter(|&v| pevne[v].is_none()) {
            let puvodni = barvy[v];
            let mut nejlepsi = (soucet(barvy), puvodni);
            for c in (0..k).filter(|&c| c != puvodni) {
                if (0..n).any(|j| j != v && barvy[j] == c && a.koliduje(v, j)) {
                    continue;
                }
                barvy[v] = c;
                let s = soucet(barvy);
                if s < nejlepsi.0 {
                    nejlepsi = (s, c);
                }
            }
            barvy[v] = nejlepsi.1;
            if nejlepsi.1 != puvodni {
                zlepseni = true;
            }
        }
    }
}

#[derive(Clone, Debug)]
pub struct Navrh {
    /// Blok (1..) pro každou volbu v pořadí `Analyza::volby`.
    pub bloky: Vec<u8>,
    pub pocet_bloku: usize,
    /// Dolní mez (největší klika / nejvíc voleb žáka) – méně bloků nejde.
    pub dolni_mez: usize,
    /// true = počet bloků je dokázaně nejmenší možný.
    pub optimalni: bool,
    /// Semináře, které se navzájem vylučují (proto nejde méně bloků).
    pub klika: Vec<String>,
}

/// Navrhne bloky: nejmenší počet bloků (přesně, pokud to stihne v limitu), pak vyrovnání velikostí.
/// `pevne` = bloky, které se nemají měnit (1..), `None` = navrhnout.
pub fn navrhni(a: &Analyza, pevne: &[Option<u8>], limit: Duration) -> Navrh {
    let n = a.n();
    if n == 0 {
        return Navrh { bloky: vec![], pocet_bloku: 0, dolni_mez: 0, optimalni: true, klika: vec![] };
    }
    let start = Instant::now();
    let pevne0: Vec<Option<usize>> = pevne.iter().map(|b| b.map(|x| x.max(1) as usize - 1)).collect();
    let klika = nejvetsi_klika(a, limit / 4);
    let max_pevny = pevne0.iter().flatten().max().map_or(0, |m| m + 1);
    let dolni = klika.len().max(a.max_voleb_zaka).max(1);
    // horní mez z DSatur (s pevnými barvami)
    let mut nejlepsi = dsatur(a, &pevne0);
    let mut k_nej = nejlepsi.iter().max().map_or(0, |m| m + 1);
    let mut optimalni = k_nej <= dolni.max(max_pevny);
    let mut k = dolni.max(max_pevny);
    while k < k_nej {
        let zbyva = limit.saturating_sub(start.elapsed());
        if zbyva.is_zero() {
            break;
        }
        match obarvi_k(a, k, &pevne0, Instant::now() + zbyva) {
            Some(b) => {
                nejlepsi = b;
                k_nej = k;
                optimalni = true;
                break;
            }
            None if Instant::now() < start + limit => k += 1, // k barev nestačí (dokázáno)
            None => break,
        }
    }
    if !optimalni && k >= k_nej {
        optimalni = true; // všechna menší k vyloučena
    }
    vyrovnej(a, &mut nejlepsi, k_nej, &pevne0);
    // očíslovat bloky podle pořadí prvního výskytu (stabilní pro uživatele), pevné ponechat
    let bloky: Vec<u8> = if pevne0.iter().any(|p| p.is_some()) {
        nejlepsi.iter().map(|&c| (c + 1) as u8).collect()
    } else {
        let mut mapa: BTreeMap<usize, u8> = BTreeMap::new();
        let mut poradi: Vec<usize> = (0..n).collect();
        poradi.sort_by_key(|&i| a.volby[i].clone());
        for i in poradi {
            let dalsi = mapa.len() as u8 + 1;
            mapa.entry(nejlepsi[i]).or_insert(dalsi);
        }
        nejlepsi.iter().map(|c| mapa[c]).collect()
    };
    Navrh {
        bloky,
        pocet_bloku: k_nej,
        dolni_mez: dolni,
        optimalni,
        klika: klika.iter().map(|&i| a.volby[i].clone()).collect(),
    }
}

// ───────────────────────────── kdo brání menšímu počtu bloků ─────────────────────────────

/// Kolize způsobené jen několika žáky (bez společného učitele / učebny): dvojice seminářů a jména žáků.
pub fn slabe_kolize(a: &Analyza, max_zaku: usize) -> Vec<(usize, usize, Vec<String>)> {
    let mut v: Vec<(usize, usize, Vec<String>)> = a
        .kolize
        .iter()
        .filter_map(|((i, j), d)| match d.as_slice() {
            [Duvod::Zaci(z)] if z.len() <= max_zaku => Some((*i, *j, z.clone())),
            _ => None,
        })
        .collect();
    v.sort_by_key(|(i, j, z)| (z.len(), *i, *j));
    v
}

/// Kdo brání menšímu počtu bloků: postupně vybírá žáka, jehož volby způsobují nejvíc kolizí,
/// které nikdo jiný (nebo jen jeden další žák) nesdílí, a přepočítá počet bloků bez jeho voleb.
/// Vrací pořadí (jméno žáka, jeho volby, počet bloků po změně jeho volby a všech předchozích).
pub fn kdo_brani(a: &Analyza, jmena: &BTreeMap<u32, String>, limit: Duration) -> Vec<(String, Vec<String>, usize)> {
    let start = Instant::now();
    let mut b = a.clone();
    let mut aktualne = navrhni(&b, &vec![None; b.n()], limit / 8).pocet_bloku;
    let mut vysledek = Vec::new();
    let odebrat_zaka = |b: &mut Analyza, z: u32| {
        for i in 0..b.n() {
            b.zaci[i].remove(&z);
        }
        let jmeno = jmena.get(&z).cloned().unwrap_or_default();
        let klice: Vec<(usize, usize)> = b.kolize.keys().copied().collect();
        for k in klice {
            let d = b.kolize.get_mut(&k).unwrap();
            for x in d.iter_mut() {
                if let Duvod::Zaci(v) = x {
                    if let Some(p) = v.iter().position(|n| *n == jmeno) {
                        v.remove(p);
                    }
                }
            }
            d.retain(|x| !matches!(x, Duvod::Zaci(v) if v.is_empty()));
            if d.is_empty() {
                b.kolize.remove(&k);
            }
        }
    };
    while aktualne > a.max_voleb_zaka.max(1) && start.elapsed() < limit && vysledek.len() < 30 {
        // příspěvek žáka: kolize (jen kvůli žákům), které sdílí nejvýš 2 žáci včetně něj
        let mut prispevek: BTreeMap<u32, usize> = BTreeMap::new();
        for ((i, j), d) in &b.kolize {
            if d.len() != 1 || !matches!(d[0], Duvod::Zaci(_)) {
                continue;
            }
            let spolecni: Vec<u32> = b.zaci[*i].intersection(&b.zaci[*j]).copied().collect();
            if spolecni.len() <= 2 {
                for z in spolecni {
                    *prispevek.entry(z).or_default() += 1;
                }
            }
        }
        let Some((&z, _)) = prispevek.iter().max_by_key(|(z, p)| (**p, std::cmp::Reverse(**z))) else { break };
        let volby: Vec<String> = (0..b.n()).filter(|&i| b.zaci[i].contains(&z)).map(|i| b.volby[i].clone()).collect();
        odebrat_zaka(&mut b, z);
        aktualne = navrhni(&b, &vec![None; b.n()], limit / 8).pocet_bloku.max(a.max_voleb_zaka);
        vysledek.push((jmena.get(&z).cloned().unwrap_or_default(), volby, aktualne));
    }
    // zkrátit na nejkratší předponu, která dosáhne nejlepšího výsledku
    if let Some(nejlepsi) = vysledek.iter().map(|x| x.2).min() {
        if let Some(p) = vysledek.iter().position(|x| x.2 == nejlepsi) {
            vysledek.truncate(p + 1);
        }
    }
    vysledek
}

/// Hodiny týdně pro třídy menu: (třída, hodiny plánu + ostatních voleb, hodiny bloků tohoto menu).
pub fn tydenni_zatizeni(skola: &Skola, rok: &SkolniRok, menu: &Menu, pocet_bloku: usize) -> Vec<(String, u32, u32)> {
    menu.tridy
        .iter()
        .map(|t| {
            let plan: u32 = skola.plan.iter().filter(|h| &h.trida == t).map(|h| h.hodin as u32).sum();
            let ostatni: u32 = rok
                .menu
                .iter()
                .filter(|m| m.id != menu.id && m.tridy.contains(t))
                .map(|m| m.hodin_tydne as u32 * if m.pocet_voleb == 1 { 1 } else { m.pocet_voleb as u32 })
                .sum();
            (t.clone(), plan + ostatni, pocet_bloku as u32 * menu.hodin_tydne as u32)
        })
        .collect()
}

/// Zapíše bloky do voleb menu.
pub fn nastav_bloky(menu: &mut Menu, a: &Analyza, bloky: &[Option<u8>]) {
    for (i, id) in a.volby.iter().enumerate() {
        if let Some(v) = menu.volby.iter_mut().find(|v| &v.id == id) {
            v.blok = bloky[i];
        }
    }
}

pub fn bloky_menu(menu: &Menu, a: &Analyza) -> Vec<Option<u8>> {
    a.volby.iter().map(|id| menu.volby.iter().find(|v| &v.id == id).and_then(|v| v.blok)).collect()
}

// ───────────────────────────── export ─────────────────────────────

/// CSV ve formátu nástroje na semináře + sloupec Blok (lze znovu naimportovat jako skupiny).
pub fn export_csv(skola: &Skola, rok: &SkolniRok, menu: &Menu) -> String {
    let mut s = String::from("\u{FEFF}Blok;Seminář;Název;Učitel;Třídy;Počet žáků;Jméno žáka;Třída žáka\r\n");
    let pole = |x: &str| {
        if x.contains([';', '"', '\n']) {
            format!("\"{}\"", x.replace('"', "\"\""))
        } else {
            x.to_string()
        }
    };
    let mut volby: Vec<&Volba> = menu.volby.iter().collect();
    volby.sort_by_key(|v| (v.blok.unwrap_or(u8::MAX), norm(&v.id)));
    let poradi_tridy = |t: &str| skola.tridy.iter().position(|x| x.id == t).unwrap_or(usize::MAX);
    for v in volby {
        let mut zaci: Vec<&Student> = rok
            .studenti
            .iter()
            .filter(|s| {
                s.v_rozvrhu()
                    && menu.tridy.contains(&s.trida)
                    && s.volby.get(&menu.id).is_some_and(|x| x.contains(&v.id))
            })
            .collect();
        zaci.sort_by(|a, b| (poradi_tridy(&a.trida), &a.jmeno).cmp(&(poradi_tridy(&b.trida), &b.jmeno)));
        let tridy: BTreeSet<&str> = zaci.iter().map(|z| z.trida.as_str()).collect();
        let mut tridy: Vec<&str> = tridy.into_iter().collect();
        tridy.sort_by_key(|t| poradi_tridy(t));
        let tridy_text = tridy.iter().map(|t| skola.nazev_tridy(t)).collect::<Vec<_>>().join(", ");
        let blok = v.blok.map_or(String::new(), |b| b.to_string());
        let hlavicka = [blok, v.id.clone(), v.nazev.clone(), v.ucitel.clone(), tridy_text, zaci.len().to_string()];
        if zaci.is_empty() {
            let r: Vec<String> = hlavicka.iter().map(|x| pole(x)).collect();
            s.push_str(&format!("{};;\r\n", r.join(";")));
        }
        for (k, z) in zaci.iter().enumerate() {
            let prvni: Vec<String> =
                if k == 0 { hlavicka.iter().map(|x| pole(x)).collect() } else { vec![String::new(); 6] };
            s.push_str(&format!("{};{};{}\r\n", prvni.join(";"), pole(&z.jmeno), pole(&skola.nazev_tridy(&z.trida))));
        }
    }
    s
}
