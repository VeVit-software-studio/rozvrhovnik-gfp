//! GUI (eframe/egui): obrazovky Rozvrh | Studenti | Volitelné | Učební plán | Číselníky |
//! Nastavení | Report a průvodce „✚ Vytvořit nový rok“.
//!
//! Vzor „fází“: akce z vykreslení (klik na hodinu, přesun, pin…) se sbírají do `Vec<Akce>`
//! během výpůjčky `&self.p.rozvrh` a aplikují se až po jejím uvolnění.

use eframe::egui::{self, Color32, RichText, Ui};
use rozvrhovnik::data::*;
use rozvrhovnik::export::{self, Pohled};
use rozvrhovnik::rok::{self, NovyZak, Zmena};
use rozvrhovnik::solver::{self, Prubeh, Vysledek};
use rozvrhovnik::validation::{self, Kontrola};
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

const CERVENA: Color32 = Color32::from_rgb(230, 80, 70);
const ORANZOVA: Color32 = Color32::from_rgb(235, 150, 40);
const ZELENA: Color32 = Color32::from_rgb(70, 180, 90);
const MODRA: Color32 = Color32::from_rgb(50, 130, 255);

#[derive(Clone, Copy, PartialEq, Eq)]
enum Obrazovka {
    Rozvrh,
    Studenti,
    Volitelne,
    Plan,
    Ciselniky,
    Nastaveni,
    Report,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Druh {
    Trida,
    Ucitel,
    Mistnost,
    Zak,
}

#[derive(Clone, Copy)]
enum Exp {
    Tridy,
    Ucitele,
    Zaci,
    Csv,
    Xml,
}

enum Akce {
    Vyber(Option<usize>),
    Presun(usize, u8),
    Pin(usize),
    Odepni(usize),
    Undo,
    Export(Exp),
    Generuj,
}

struct Beh {
    handle: JoinHandle<Vysledek>,
    stop: Arc<AtomicBool>,
    prubeh: Arc<Mutex<Prubeh>>,
}

type Rozdelit = Option<(String, String, String)>;

struct Pruvodce {
    krok: usize,
    label: String,
    texty: BTreeMap<String, String>,
    novi: BTreeMap<String, Vec<NovyZak>>,
    zmeny: BTreeMap<u32, Zmena>,
    filtr: String,
    novy: Option<SkolniRok>,
    menu_sel: usize,
    rozdelit: Rozdelit,
}

pub struct RozvrhApp {
    p: Projekt,
    cesta: PathBuf,
    obr: Obrazovka,
    beh: Option<Beh>,
    zprava: String,
    // rozvrh
    druh: Druh,
    sel_trida: String,
    sel_ucitel: String,
    sel_mistnost: String,
    sel_zak: Option<u32>,
    vybrana: Option<usize>,
    undo: Vec<(Vec<Option<u8>>, Vec<Option<String>>)>,
    // studenti
    filtr: String,
    novy_jmeno: String,
    novy_trida: String,
    // volitelné
    menu_sel: usize,
    rozdelit: Rozdelit,
    // plán
    plan_trida: String,
    // číselníky
    cis: u8,
    novy_id: String,
    volno_ucitel: Option<String>,
    // soubory
    otevrit_okno: bool,
    cesta_text: String,
    pruvodce: Option<Pruvodce>,
    // cache pro report
    zateze: Option<Vec<(String, f32)>>,
    kontrola: Option<Kontrola>,
}

impl RozvrhApp {
    pub fn new(cc: &eframe::CreationContext<'_>, p: Projekt, cesta: PathBuf) -> Self {
        let mut style = (*cc.egui_ctx.style()).clone();
        style.spacing.item_spacing = egui::vec2(6.0, 4.0);
        cc.egui_ctx.set_style(style);
        let sel_trida = p.skola.tridy.first().map(|t| t.id.clone()).unwrap_or_default();
        let sel_ucitel = p.skola.ucitele.first().map(|t| t.id.clone()).unwrap_or_default();
        let sel_mistnost = p.skola.mistnosti.first().map(|t| t.id.clone()).unwrap_or_default();
        let zprava = if cesta.exists() {
            format!("Načteno: {}", cesta.display())
        } else {
            "Výchozí data (seed z rozvrhu 2026/27). Uloží se do skola_data.json.".into()
        };
        RozvrhApp {
            cesta_text: cesta.display().to_string(),
            p,
            cesta,
            obr: Obrazovka::Rozvrh,
            beh: None,
            zprava,
            druh: Druh::Trida,
            sel_trida: sel_trida.clone(),
            sel_ucitel,
            sel_mistnost,
            sel_zak: None,
            vybrana: None,
            undo: vec![],
            filtr: String::new(),
            novy_jmeno: String::new(),
            novy_trida: sel_trida.clone(),
            menu_sel: 0,
            rozdelit: None,
            plan_trida: sel_trida,
            cis: 0,
            novy_id: String::new(),
            volno_ucitel: None,
            otevrit_okno: false,
            pruvodce: None,
            zateze: None,
            kontrola: None,
        }
    }

    // ───────────── soubory ─────────────

    fn uloz(&mut self) {
        self.zprava = match self.p.uloz(&self.cesta) {
            Ok(()) => format!("💾 Uloženo do {}", self.cesta.display()),
            Err(e) => e,
        };
    }

    fn otevri(&mut self, c: PathBuf) {
        match Projekt::nacti(&c) {
            Ok(p) => {
                self.p = p;
                self.cesta = c;
                self.vybrana = None;
                self.undo.clear();
                self.zateze = None;
                self.kontrola = None;
                self.menu_sel = 0;
                if let Some(t) = self.p.skola.tridy.first() {
                    self.sel_trida = t.id.clone();
                    self.plan_trida = t.id.clone();
                }
                self.zprava = format!("📁 Otevřeno: {}", self.cesta.display());
            }
            Err(e) => self.zprava = e,
        }
    }

    fn export(&mut self, e: Exp) {
        let Some(v) = &self.p.rozvrh else { return };
        let s = &self.p.skola;
        let (nazev, obsah) = match e {
            Exp::Tridy => ("rozvrh_tridy.html", export::html_tridy(s, v)),
            Exp::Ucitele => ("rozvrh_ucitele.html", export::html_ucitele(s, v)),
            Exp::Zaci => ("rozvrh_zaci.html", export::html_zaci(s, &self.p.rok, v)),
            Exp::Csv => ("rozvrh.csv", export::csv(s, v)),
            Exp::Xml => ("rozvrh.xml", export::xml(s, v)),
        };
        if let Some(c) = cesta_pro_ulozeni(nazev) {
            self.zprava = match std::fs::write(&c, obsah) {
                Ok(()) => format!("⬇ Uloženo: {}", c.display()),
                Err(err) => format!("Nelze zapsat {}: {err}", c.display()),
            };
        }
    }

    // ───────────── řešič ─────────────

    fn spust(&mut self) {
        if self.beh.is_some() {
            return;
        }
        let skola = self.p.skola.clone();
        let rok = self.p.rok.clone();
        let piny = self.p.piny.clone();
        let stop = Arc::new(AtomicBool::new(false));
        let prubeh = Arc::new(Mutex::new(Prubeh { faze: "start".into(), ..Default::default() }));
        let (s2, p2) = (stop.clone(), prubeh.clone());
        let handle = std::thread::spawn(move || solver::generuj(&skola, &rok, &piny, &s2, Some(&p2)));
        self.beh = Some(Beh { handle, stop, prubeh });
        self.zprava = "Generuji rozvrh…".into();
    }

    fn kontrola_behu(&mut self, ctx: &egui::Context) {
        let hotovo = self.beh.as_ref().is_some_and(|b| b.handle.is_finished());
        if !hotovo {
            if self.beh.is_some() {
                ctx.request_repaint_after(Duration::from_millis(150));
            }
            return;
        }
        let b = self.beh.take().unwrap();
        match b.handle.join() {
            Ok(v) => {
                self.zprava = format!(
                    "✔ Rozvrh vygenerován za {:.1} s: {} problémů, {} varování.",
                    v.cas_s,
                    v.problemy.len(),
                    v.varovani.len()
                );
                if self.p.skola.tridy.iter().all(|t| t.id != self.sel_trida) {
                    self.sel_trida = self.p.skola.tridy.first().map(|t| t.id.clone()).unwrap_or_default();
                }
                self.p.rozvrh = Some(v);
                self.vybrana = None;
                self.undo.clear();
                self.kontrola = None;
                self.zateze = None;
                self.obr = Obrazovka::Report;
            }
            Err(_) => self.zprava = "✖ Řešič skončil chybou.".into(),
        }
    }

    // ───────────── úpravy rozvrhu ─────────────

    fn presun(&mut self, i: usize, start: u8) {
        let skola = &self.p.skola;
        let Some(v) = self.p.rozvrh.as_mut() else { return };
        self.undo.push((v.slot.clone(), v.mistnost.clone()));
        let clenove = v.clenove_jednotky(v.lekce[i].jednotka);
        for &c in &clenove {
            v.slot[c] = Some(start);
        }
        let set: BTreeSet<usize> = clenove.iter().copied().collect();
        solver::prirad_mistnosti(skola, v, Some(&set));
        solver::zkontroluj_a_zapis(skola, v);
        for &c in &clenove {
            if let Some(x) = self.p.piny.get_mut(&v.lekce[c].klic) {
                *x = start;
            }
        }
        let (d, k) = den_a_slot(start as usize);
        self.zprava = format!(
            "Přesunuto: {} ➡ {} {}. hod · problémů: {}",
            solver::popis_lekce(&v.lekce[i]),
            NAZVY_DNU[d],
            k,
            v.problemy.len()
        );
        self.kontrola = None;
    }

    fn zpet(&mut self) {
        let skola = &self.p.skola;
        if let (Some(v), Some((s, m))) = (self.p.rozvrh.as_mut(), self.undo.pop()) {
            v.slot = s;
            v.mistnost = m;
            solver::zkontroluj_a_zapis(skola, v);
            self.zprava = format!("↩ Vráceno · problémů: {}", v.problemy.len());
            self.kontrola = None;
        }
    }

    fn aplikuj(&mut self, akce: Vec<Akce>) {
        for a in akce {
            match a {
                Akce::Vyber(x) => self.vybrana = x,
                Akce::Presun(i, s) => self.presun(i, s),
                Akce::Pin(i) => {
                    if let Some(v) = &self.p.rozvrh {
                        if let Some(s) = v.slot[i] {
                            self.p.piny.insert(v.lekce[i].klic.clone(), s);
                            self.zprava = format!("📌 Připnuto: {}", solver::popis_lekce(&v.lekce[i]));
                        }
                    }
                }
                Akce::Odepni(i) => {
                    if let Some(v) = &self.p.rozvrh {
                        for c in v.clenove_jednotky(v.lekce[i].jednotka) {
                            self.p.piny.remove(&v.lekce[c].klic);
                        }
                        self.zprava = format!("Odepnuto: {}", solver::popis_lekce(&v.lekce[i]));
                    }
                }
                Akce::Undo => self.zpet(),
                Akce::Export(e) => self.export(e),
                Akce::Generuj => self.spust(),
            }
        }
    }

    fn pohled(&self) -> Pohled {
        match self.druh {
            Druh::Trida => Pohled::Trida(self.sel_trida.clone()),
            Druh::Ucitel => Pohled::Ucitel(self.sel_ucitel.clone()),
            Druh::Mistnost => Pohled::Mistnost(self.sel_mistnost.clone()),
            Druh::Zak => Pohled::Zak(self.sel_zak.unwrap_or(u32::MAX - 100_000)),
        }
    }

    // ───────────── horní lišta ─────────────

    fn horni_lista(&mut self, ui: &mut Ui) {
        ui.horizontal_wrapped(|ui| {
            let taby = [
                (Obrazovka::Rozvrh, "📅 Rozvrh"),
                (Obrazovka::Studenti, "👥 Studenti"),
                (Obrazovka::Volitelne, "☑ Volitelné"),
                (Obrazovka::Plan, "📚 Učební plán"),
                (Obrazovka::Ciselniky, "📇 Číselníky"),
                (Obrazovka::Nastaveni, "⚙ Nastavení"),
                (Obrazovka::Report, "📊 Report"),
            ];
            for (o, t) in taby {
                if ui.selectable_label(self.obr == o, RichText::new(t).size(15.0)).clicked() {
                    self.obr = o;
                    if o == Obrazovka::Report {
                        self.zateze = None;
                    }
                }
            }
            ui.separator();
            let novy = egui::Button::new(RichText::new("✚ Vytvořit nový rok…").color(Color32::WHITE).strong())
                .fill(Color32::from_rgb(40, 140, 70));
            if ui.add_enabled(self.beh.is_none(), novy).clicked() {
                self.pruvodce = Some(Pruvodce::new(&self.p));
            }
            if ui.button("💾 Uložit").clicked() {
                self.uloz();
            }
            if ui.button("📁 Otevřít").clicked() {
                match otevrit_dialog() {
                    Some(Some(c)) => self.otevri(c),
                    Some(None) => {}
                    None => self.otevrit_okno = true,
                }
            }
        });
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new(format!("Rok {}", self.p.rok.label)).strong());
            ui.separator();
            let limit = self.p.skola.nastaveni.casovy_limit_s;
            if let Some(b) = &self.beh {
                let p = b.prubeh.lock().map(|x| x.clone()).unwrap_or_default();
                ui.spinner();
                ui.label(format!(
                    "{} · {:.0}/{} s · iterací {} · tvrdé {} · měkké {}",
                    p.faze, p.cas_s, limit, p.iterace, p.tvrde, p.mekke
                ));
                if ui.button("⏹ Zastavit").on_hover_text("Ukončí žíhání a použije dosud nejlepší rozvrh").clicked()
                {
                    b.stop.store(true, Ordering::Relaxed);
                }
            } else if ui
                .button(RichText::new("▶ Generovat").strong())
                .on_hover_text(format!("Vygeneruje rozvrh (limit {} s, lze změnit v Nastavení)", limit))
                .clicked()
            {
                self.spust();
            }
            ui.separator();
            ui.label(RichText::new(&self.zprava).weak());
        });
    }

    // ───────────── obrazovka Rozvrh ─────────────

    fn obrazovka_rozvrh(&mut self, ui: &mut Ui) {
        let mut akce: Vec<Akce> = Vec::new();
        let skola = &self.p.skola;
        ui.horizontal_wrapped(|ui| {
            ui.label("Pohled:");
            ui.selectable_value(&mut self.druh, Druh::Trida, "třída");
            ui.selectable_value(&mut self.druh, Druh::Ucitel, "učitel");
            ui.selectable_value(&mut self.druh, Druh::Mistnost, "místnost");
            ui.selectable_value(&mut self.druh, Druh::Zak, "student");
            ui.separator();
            match self.druh {
                Druh::Trida | Druh::Zak => {
                    combo_trida(ui, "rz-trida", &skola.tridy, &mut self.sel_trida);
                }
                Druh::Ucitel => {
                    combo_ucitel(ui, "rz-ucitel", &skola.ucitele, &mut self.sel_ucitel, 180.0);
                }
                Druh::Mistnost => {
                    egui::ComboBox::from_id_source("rz-mistnost")
                        .selected_text(&self.sel_mistnost)
                        .width(200.0)
                        .show_ui(ui, |ui| {
                            for m in &skola.mistnosti {
                                ui.selectable_value(
                                    &mut self.sel_mistnost,
                                    m.id.clone(),
                                    format!("{} – {}", m.id, m.nazev),
                                );
                            }
                        });
                }
            }
            if self.druh == Druh::Zak {
                let zaci: Vec<&Student> =
                    self.p.rok.studenti.iter().filter(|s| s.trida == self.sel_trida && s.v_rozvrhu()).collect();
                if !zaci.iter().any(|s| Some(s.id) == self.sel_zak) {
                    self.sel_zak = zaci.first().map(|s| s.id);
                }
                let text = self.sel_zak.and_then(|id| self.p.rok.student(id)).map_or("—".into(), |s| s.jmeno.clone());
                egui::ComboBox::from_id_source("rz-zak").selected_text(text).width(200.0).show_ui(ui, |ui| {
                    for s in zaci {
                        ui.selectable_value(
                            &mut self.sel_zak,
                            Some(s.id),
                            format!("{} ({})", s.jmeno, s.skupina_aj.nazev()),
                        );
                    }
                });
            }
            ui.separator();
            let je = self.p.rozvrh.is_some();
            if ui
                .add_enabled(je, egui::Button::new("⬇ Tisk tříd (HTML)"))
                .on_hover_text("Všechny třídy, A4 na šířku, 1 třída/strana ➡ otevřít a Ctrl+P")
                .clicked()
            {
                akce.push(Akce::Export(Exp::Tridy));
            }
            if ui.add_enabled(je, egui::Button::new("⬇ Učitelé (HTML)")).clicked() {
                akce.push(Akce::Export(Exp::Ucitele));
            }
            if ui
                .add_enabled(je, egui::Button::new("⬇ Žáci (HTML)"))
                .on_hover_text("Osobní rozvrh každého žáka (1 strana)")
                .clicked()
            {
                akce.push(Akce::Export(Exp::Zaci));
            }
            if ui.add_enabled(je, egui::Button::new("⬇ CSV")).clicked() {
                akce.push(Akce::Export(Exp::Csv));
            }
            if ui.add_enabled(je, egui::Button::new("⬇ XML")).clicked() {
                akce.push(Akce::Export(Exp::Xml));
            }
            if ui
                .add_enabled(self.beh.is_none(), egui::Button::new("↻ Přegenerovat"))
                .on_hover_text("Připnuté hodiny (📌) zůstanou na místě")
                .clicked()
            {
                akce.push(Akce::Generuj);
            }
            if ui.add_enabled(!self.undo.is_empty(), egui::Button::new("↩ Zpět (Ctrl+Z)")).clicked() {
                akce.push(Akce::Undo);
            }
        });

        let Some(v) = self.p.rozvrh.as_ref() else {
            ui.add_space(30.0);
            ui.heading("Rozvrh zatím není vygenerován.");
            ui.label("Spusťte ▶ Generovat v horní liště, nebo projděte průvodcem „✚ Vytvořit nový rok“.");
            self.aplikuj(akce);
            return;
        };
        if v.rok != self.p.rok.label {
            ui.label(
                RichText::new(format!("⚠ Rozvrh byl vygenerován pro rok {} – přegenerujte.", v.rok)).color(ORANZOVA),
            );
        }
        if v.problemy.is_empty() {
            ui.label(RichText::new("✔ Rozvrh je bez tvrdých problémů").color(ZELENA));
        } else {
            egui::CollapsingHeader::new(
                RichText::new(format!("⚠ {} problémů – rozbalit", v.problemy.len())).color(CERVENA),
            )
            .id_source("rz-problemy")
            .default_open(true)
            .show(ui, |ui| {
                egui::ScrollArea::vertical().id_source("rz-problemy-sc").max_height(110.0).show(ui, |ui| {
                    for p in &v.problemy {
                        ui.label(RichText::new(p).color(CERVENA).small());
                    }
                });
            });
        }

        // detail vybrané hodiny
        let pinovane: BTreeSet<usize> =
            v.lekce.iter().filter(|l| self.p.piny.contains_key(&l.klic)).map(|l| l.jednotka).collect();
        if let Some(i) = self.vybrana.filter(|&i| i < v.lekce.len()) {
            let l = &v.lekce[i];
            egui::Frame::group(ui.style()).show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.label(RichText::new("Vybráno:").strong());
                    ui.label(RichText::new(solver::popis_lekce(l)).color(MODRA).strong());
                    if let Some(s) = v.slot[i] {
                        let (d, k) = den_a_slot(s as usize);
                        ui.label(format!("· {} {}. hod ({})", NAZVY_DNU[d], k, CASY[k]));
                    }
                    ui.label(format!(
                        "· {} · {} žáků · {} h",
                        v.mistnost[i].as_deref().unwrap_or("bez místnosti"),
                        v.pocet_realnych_zaku(i),
                        l.len
                    ));
                    let clenu = v.clenove_jednotky(l.jednotka).len();
                    if clenu > 1 {
                        ui.label(format!("· jednotka {} lekcí (hýbou se společně)", clenu));
                    }
                    if pinovane.contains(&l.jednotka) {
                        if ui.button("📌 Odepnout").clicked() {
                            akce.push(Akce::Odepni(i));
                        }
                    } else if ui
                        .button("📌 Připnout")
                        .on_hover_text("Při přegenerování zůstane hodina na místě")
                        .clicked()
                    {
                        akce.push(Akce::Pin(i));
                    }
                    if ui.button("✖ Zrušit výběr (Esc)").clicked() {
                        akce.push(Akce::Vyber(None));
                    }
                    ui.label(RichText::new("➡ klikněte na „➡ sem“ v cílové buňce").weak());
                });
            });
        }

        let pohled = self.pohled();
        let maska = match &pohled {
            Pohled::Trida(t) => v.limity.get(t).map(|l| solver::maska_tridy(*l, skola.nastaveni.odpoledne_od as usize)),
            Pohled::Zak(z) => v
                .zaci_tridy
                .get(z)
                .and_then(|t| v.limity.get(t))
                .map(|l| solver::maska_tridy(*l, skola.nastaveni.odpoledne_od as usize)),
            _ => None,
        };
        let dark = ui.visuals().dark_mode;
        let sirka = ((ui.available_width() - 60.0) / SLOTU as f32 - 8.0).clamp(80.0, 160.0);
        let barva_textu = if dark { Color32::from_gray(245) } else { Color32::from_gray(15) };
        let vybrana = self.vybrana.filter(|&i| i < v.lekce.len());
        let hodin: usize =
            (0..N_SLOTU).filter(|&t| !export::lekce_v_bunce(v, &pohled, t / SLOTU, t % SLOTU).is_empty()).count();
        ui.label(
            RichText::new(format!("Obsazeno {} hodin týdně · tmavší buňky = mimo limity P1", hodin)).weak().small(),
        );
        egui::ScrollArea::both().id_source("rz-mrizka").auto_shrink([false, false]).show(ui, |ui| {
            egui::Grid::new("rz-grid").spacing([3.0, 3.0]).show(ui, |ui| {
                ui.label("");
                for k in 0..SLOTU {
                    ui.vertical_centered(|ui| {
                        ui.set_width(sirka);
                        ui.label(RichText::new(format!("{}.", k)).strong());
                        ui.label(RichText::new(format!("{}–{}", CASY[k], CASY_KONEC[k])).small().weak());
                    });
                }
                ui.end_row();
                for d in 0..DNY {
                    ui.label(RichText::new(NAZVY_DNU[d]).strong().size(17.0));
                    for k in 0..SLOTU {
                        let mimo = maska.is_some_and(|m| !m[k]);
                        let fill = match (mimo, dark) {
                            (true, true) => Color32::from_rgb(28, 18, 18),
                            (true, false) => Color32::from_rgb(222, 212, 212),
                            (false, true) => Color32::from_gray(38),
                            (false, false) => Color32::from_gray(246),
                        };
                        egui::Frame::none().fill(fill).rounding(4.0).inner_margin(2.0).show(ui, |ui| {
                            ui.set_width(sirka);
                            ui.set_min_height(60.0);
                            ui.vertical(|ui| {
                                for i in export::lekce_v_bunce(v, &pohled, d, k) {
                                    let l = &v.lekce[i];
                                    let problem = v.problemove_lekce.binary_search(&i).is_ok();
                                    let pin = if pinovane.contains(&l.jednotka) { "📌" } else { "" };
                                    let varov = if problem { "⚠" } else { "" };
                                    let sk =
                                        if l.skupina.is_empty() { String::new() } else { format!(" {}", l.skupina) };
                                    let horni = format!("{}{}{}{}{}", varov, pin, l.tyden.prefix(), l.predmet, sk);
                                    let mistnost = v.mistnost[i].as_deref().unwrap_or("?");
                                    let dolni = match self.druh {
                                        Druh::Trida | Druh::Zak => format!("{} · {}", l.ucitel, mistnost),
                                        Druh::Ucitel => {
                                            format!("{} · {}", export::tridy_kratce(skola, &l.tridy), mistnost)
                                        }
                                        Druh::Mistnost => {
                                            format!("{} · {}", export::tridy_kratce(skola, &l.tridy), l.ucitel)
                                        }
                                    };
                                    let mut b = egui::Button::new(
                                        RichText::new(format!("{}\n{}", horni, dolni)).size(11.5).color(barva_textu),
                                    )
                                    .fill(barva_predmetu(&l.predmet, dark))
                                    .min_size(egui::vec2(sirka - 4.0, 0.0));
                                    if vybrana.is_some_and(|x| v.lekce[x].jednotka == l.jednotka) {
                                        b = b.stroke(egui::Stroke::new(2.5, MODRA));
                                    } else if problem {
                                        b = b.stroke(egui::Stroke::new(1.5, CERVENA));
                                    }
                                    let hover = format!(
                                        "{}\n{} · {} žáků · {}\nklíč: {}",
                                        solver::popis_lekce(l),
                                        skola.predmet(&l.predmet).map_or(l.predmet.as_str(), |p| p.nazev.as_str()),
                                        v.pocet_realnych_zaku(i),
                                        mistnost,
                                        l.klic
                                    );
                                    if ui.add(b).on_hover_text(hover).clicked() {
                                        akce.push(Akce::Vyber(if vybrana == Some(i) { None } else { Some(i) }));
                                    }
                                }
                                if let Some(sel) = vybrana {
                                    let start = (d * SLOTU + k) as u8;
                                    if k + v.lekce[sel].len as usize <= SLOTU
                                        && v.slot[sel] != Some(start)
                                        && ui
                                            .small_button("➡ sem")
                                            .on_hover_text("Přesunout vybranou hodinu (celou jednotku) sem")
                                            .clicked()
                                    {
                                        akce.push(Akce::Presun(sel, start));
                                    }
                                }
                            });
                        });
                    }
                    ui.end_row();
                }
            });
        });
        self.aplikuj(akce);
    }

    // ───────────── obrazovka Studenti ─────────────

    fn obrazovka_studenti(&mut self, ui: &mut Ui) {
        let skola = &self.p.skola;
        let rok = &mut self.p.rok;
        ui.horizontal_wrapped(|ui| {
            ui.label("Třída:");
            let text = if self.filtr.is_empty() { "(všechny)".to_string() } else { skola.nazev_tridy(&self.filtr) };
            egui::ComboBox::from_id_source("st-filtr").selected_text(text).width(140.0).show_ui(ui, |ui| {
                ui.selectable_value(&mut self.filtr, String::new(), "(všechny)");
                for t in &skola.tridy {
                    ui.selectable_value(&mut self.filtr, t.id.clone(), &t.nazev);
                }
            });
            if !self.filtr.is_empty()
                && ui.button("⚖ Rozdělit AJ 15:15").on_hover_text("Vyrovnaně podle pohlaví").clicked()
            {
                rok::rozdel_tridu(rok, &self.filtr);
            }
            let zaci: Vec<&Student> = rok
                .studenti
                .iter()
                .filter(|s| s.v_rozvrhu() && (self.filtr.is_empty() || s.trida == self.filtr))
                .collect();
            let d = zaci.iter().filter(|s| s.pohlavi == Pohlavi::D).count();
            let a = zaci.iter().filter(|s| s.skupina_aj == SkupinaAj::A).count();
            ui.label(format!(
                "{} žáků ({} D + {} CH) · AJa {} / AJb {}",
                zaci.len(),
                d,
                zaci.len() - d,
                a,
                zaci.len() - a
            ));
        });
        ui.horizontal(|ui| {
            ui.label("Nový žák:");
            ui.add(egui::TextEdit::singleline(&mut self.novy_jmeno).hint_text("Jméno Příjmení").desired_width(200.0));
            combo_trida(ui, "st-novy-trida", &skola.tridy, &mut self.novy_trida);
            if ui.button("+ Přidat").clicked() && !self.novy_jmeno.trim().is_empty() {
                let a =
                    rok.studenti.iter().filter(|s| s.trida == self.novy_trida && s.skupina_aj == SkupinaAj::A).count();
                let b =
                    rok.studenti.iter().filter(|s| s.trida == self.novy_trida && s.skupina_aj == SkupinaAj::B).count();
                let id = rok.nove_id();
                let jmeno = self.novy_jmeno.trim().to_string();
                rok.studenti.push(Student {
                    id,
                    pohlavi: rok::odhad_pohlavi(&jmeno),
                    jmeno,
                    trida: self.novy_trida.clone(),
                    skupina_aj: if a <= b { SkupinaAj::A } else { SkupinaAj::B },
                    volby: BTreeMap::new(),
                    status: StavStudenta::Aktivni,
                });
                self.novy_jmeno.clear();
            }
        });
        ui.separator();
        let mut smazat = None;
        egui::ScrollArea::vertical().id_source("st-sc").auto_shrink([false, false]).show(ui, |ui| {
            egui::Grid::new("st-grid").striped(true).num_columns(7).show(ui, |ui| {
                for h in ["Jméno", "Třída", "AJ", "Pohlaví", "Stav", "Volby", ""] {
                    ui.label(RichText::new(h).strong());
                }
                ui.end_row();
                for s in rok.studenti.iter_mut().filter(|s| self.filtr.is_empty() || s.trida == self.filtr) {
                    let neaktivni = s.status == StavStudenta::Odesel;
                    ui.add_sized([200.0, 20.0], egui::TextEdit::singleline(&mut s.jmeno));
                    combo_trida(ui, ("st-t", s.id), &skola.tridy, &mut s.trida);
                    egui::ComboBox::from_id_source(("st-aj", s.id))
                        .selected_text(s.skupina_aj.nazev())
                        .width(60.0)
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut s.skupina_aj, SkupinaAj::A, "AJa");
                            ui.selectable_value(&mut s.skupina_aj, SkupinaAj::B, "AJb");
                        });
                    egui::ComboBox::from_id_source(("st-p", s.id))
                        .selected_text(s.pohlavi.zkratka())
                        .width(50.0)
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut s.pohlavi, Pohlavi::D, "D");
                            ui.selectable_value(&mut s.pohlavi, Pohlavi::CH, "CH");
                        });
                    let barva = if neaktivni {
                        Color32::GRAY
                    } else if s.status == StavStudenta::Propada {
                        ORANZOVA
                    } else {
                        ui.visuals().text_color()
                    };
                    egui::ComboBox::from_id_source(("st-s", s.id))
                        .selected_text(RichText::new(s.status.nazev()).color(barva))
                        .width(90.0)
                        .show_ui(ui, |ui| {
                            for st in [StavStudenta::Aktivni, StavStudenta::Propada, StavStudenta::Odesel] {
                                ui.selectable_value(&mut s.status, st, st.nazev());
                            }
                        });
                    let volby: Vec<String> = s.volby.iter().map(|(m, v)| format!("{}: {}", m, v.join(", "))).collect();
                    ui.label(format!("{} menu", s.volby.len())).on_hover_text(if volby.is_empty() {
                        "žádné".into()
                    } else {
                        volby.join("\n")
                    });
                    if ui.small_button("✖").on_hover_text("Smazat žáka").clicked() {
                        smazat = Some(s.id);
                    }
                    ui.end_row();
                }
            });
        });
        if let Some(id) = smazat {
            rok.studenti.retain(|s| s.id != id);
        }
    }

    // ───────────── obrazovka Učební plán ─────────────

    fn obrazovka_plan(&mut self, ui: &mut Ui) {
        let Skola { tridy, ucitele, predmety, plan, .. } = &mut self.p.skola;
        if tridy.iter().all(|t| t.id != self.plan_trida) {
            self.plan_trida = tridy.first().map(|t| t.id.clone()).unwrap_or_default();
        }
        let hodin: u32 = plan.iter().filter(|p| p.trida == self.plan_trida).map(|p| p.hodin as u32).sum();
        let volitelne: u32 = self
            .p
            .rok
            .menu
            .iter()
            .filter(|m| m.tridy.contains(&self.plan_trida))
            .map(|m| m.hodin_tydne as u32 * m.pocet_voleb as u32)
            .sum();
        ui.horizontal(|ui| {
            ui.label("Třída:");
            combo_trida(ui, "pl-trida", tridy, &mut self.plan_trida);
            ui.label(format!("Plán {} h + volitelné {} h = {} h týdně", hodin, volitelne, hodin + volitelne));
        });
        ui.label(RichText::new("Rozdělení: celá třída / AJa-AJb (nezávislé skupiny) / dívky-chlapci (paralelně) / L↔S = lichý-sudý týden").weak().small());
        ui.separator();
        let mut smazat = None;
        egui::ScrollArea::vertical().id_source("pl-sc").auto_shrink([false, false]).show(ui, |ui| {
            egui::Grid::new("pl-grid").striped(true).num_columns(6).show(ui, |ui| {
                for h in ["Předmět", "Hodin", "2h blok", "Rozdělení", "Učitelé", ""] {
                    ui.label(RichText::new(h).strong());
                }
                ui.end_row();
                for (idx, ph) in plan.iter_mut().enumerate().filter(|(_, p)| p.trida == self.plan_trida) {
                    let stridave =
                        matches!(ph.struktura, Struktura::Stridave { .. } | Struktura::StridaveSkupiny { .. });
                    if stridave {
                        ui.add_sized([90.0, 20.0], egui::TextEdit::singleline(&mut ph.predmet))
                            .on_hover_text("Popisek (předměty jsou u L/S párů)");
                    } else {
                        combo_predmet(ui, ("pl-p", idx), predmety, &mut ph.predmet);
                    }
                    ui.add(egui::DragValue::new(&mut ph.hodin).clamp_range(1..=12));
                    ui.checkbox(&mut ph.dvoj, "");
                    let mut druh = druh_struktury(&ph.struktura);
                    let puvodni = druh;
                    egui::ComboBox::from_id_source(("pl-d", idx))
                        .selected_text(ph.struktura.nazev_druhu())
                        .width(120.0)
                        .show_ui(ui, |ui| {
                            for (i, n) in ["celá třída", "AJa / AJb", "dívky / chlapci", "L↔S třída", "L↔S skupiny"]
                                .iter()
                                .enumerate()
                            {
                                ui.selectable_value(&mut druh, i, *n);
                            }
                        });
                    if druh != puvodni {
                        ph.struktura = preved_strukturu(&ph.struktura, &ph.predmet, druh);
                    }
                    ui.horizontal(|ui| match &mut ph.struktura {
                        Struktura::CelouTridou(u) => {
                            combo_ucitel(ui, ("pl-u", idx, 0), ucitele, u, 90.0);
                        }
                        Struktura::Rozdeleno { a, b } => {
                            ui.label("AJa");
                            combo_ucitel(ui, ("pl-u", idx, 0), ucitele, a, 90.0);
                            ui.label("AJb");
                            combo_ucitel(ui, ("pl-u", idx, 1), ucitele, b, 90.0);
                        }
                        Struktura::Pohlavi { div, chl } => {
                            ui.label("D");
                            combo_ucitel(ui, ("pl-u", idx, 0), ucitele, div, 90.0);
                            ui.label("CH");
                            combo_ucitel(ui, ("pl-u", idx, 1), ucitele, chl, 90.0);
                        }
                        Struktura::Stridave { l, s } => {
                            ui.label("L");
                            combo_predmet(ui, ("pl-lp", idx, 0), predmety, &mut l.predmet);
                            combo_ucitel(ui, ("pl-u", idx, 0), ucitele, &mut l.ucitel, 90.0);
                            ui.label("S");
                            combo_predmet(ui, ("pl-lp", idx, 1), predmety, &mut s.predmet);
                            combo_ucitel(ui, ("pl-u", idx, 1), ucitele, &mut s.ucitel, 90.0);
                        }
                        Struktura::StridaveSkupiny { a, b } => {
                            ui.vertical(|ui| {
                                for (g, pary) in [("AJa", a), ("AJb", b)] {
                                    ui.horizontal(|ui| {
                                        for (w, pol) in pary.iter_mut().enumerate() {
                                            ui.label(format!("{} {}", g, if w == 0 { "L" } else { "S" }));
                                            combo_predmet(ui, ("pl-sp", idx, g, w), predmety, &mut pol.predmet);
                                            combo_ucitel(ui, ("pl-su", idx, g, w), ucitele, &mut pol.ucitel, 80.0);
                                        }
                                    });
                                }
                            });
                        }
                    });
                    if ui.small_button("✖").clicked() {
                        smazat = Some(idx);
                    }
                    ui.end_row();
                }
            });
            if ui.button("+ Přidat předmět").clicked() {
                let u = ucitele.first().map(|u| u.id.clone()).unwrap_or_default();
                let p = predmety.first().map(|p| p.id.clone()).unwrap_or_default();
                plan.push(PlanovaHodina {
                    trida: self.plan_trida.clone(),
                    predmet: p,
                    hodin: 1,
                    struktura: Struktura::CelouTridou(u),
                    dvoj: false,
                });
            }
        });
        if let Some(i) = smazat {
            plan.remove(i);
        }
    }

    // ───────────── obrazovka Číselníky ─────────────

    fn obrazovka_ciselniky(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            for (i, n) in ["Třídy", "Místnosti", "Učitelé", "Předměty"].iter().enumerate() {
                ui.selectable_value(&mut self.cis, i as u8, *n);
            }
            ui.separator();
            ui.add(egui::TextEdit::singleline(&mut self.novy_id).hint_text("nové ID").desired_width(100.0));
            if ui.button("+ Přidat").clicked() && !self.novy_id.trim().is_empty() {
                let id = self.novy_id.trim().to_string();
                let s = &mut self.p.skola;
                match self.cis {
                    0 if s.trida(&id).is_none() => s.tridy.push(Trida {
                        id: id.clone(),
                        nazev: id.clone(),
                        rocnik: 1,
                        typ: TypStudia::Osmilete,
                        kmenova: id.clone(),
                        naslednik: None,
                        omezeni_prepsat: None,
                    }),
                    1 if s.mistnost(&id).is_none() => {
                        s.mistnosti.push(Mistnost { id: id.clone(), nazev: id.clone(), kapacita: 30, kmenova: false })
                    }
                    2 if s.ucitel(&id).is_none() => s.ucitele.push(Ucitel {
                        id: id.clone(),
                        jmeno: "(doplnit)".into(),
                        volno: vec![],
                        max_den: 7,
                        preferovana_mistnost: None,
                    }),
                    3 if s.predmet(&id).is_none() => s.predmety.push(Predmet {
                        id: id.clone(),
                        nazev: id.clone(),
                        specialni_mistnosti: vec![],
                        kmenova_ok: true,
                    }),
                    _ => self.zprava = format!("ID {} už existuje.", id),
                }
                self.novy_id.clear();
            }
        });
        ui.separator();
        let Skola { tridy, mistnosti, ucitele, predmety, .. } = &mut self.p.skola;
        let mut smazat: Option<usize> = None;
        egui::ScrollArea::vertical().id_source("cis-sc").auto_shrink([false, false]).show(ui, |ui| match self.cis {
            0 => {
                let ids: Vec<(String, String)> = tridy.iter().map(|t| (t.id.clone(), t.nazev.clone())).collect();
                egui::Grid::new("cis-t").striped(true).show(ui, |ui| {
                    for h in ["ID", "Název", "Ročník", "Typ", "Kmenová", "Následník", "Limity P1", ""] {
                        ui.label(RichText::new(h).strong());
                    }
                    ui.end_row();
                    for (i, t) in tridy.iter_mut().enumerate() {
                        ui.label(&t.id);
                        ui.add_sized([110.0, 20.0], egui::TextEdit::singleline(&mut t.nazev));
                        ui.add(egui::DragValue::new(&mut t.rocnik).clamp_range(1..=8));
                        egui::ComboBox::from_id_source(("cis-typ", i))
                            .selected_text(t.typ.nazev())
                            .width(70.0)
                            .show_ui(ui, |ui| {
                                ui.selectable_value(&mut t.typ, TypStudia::Osmilete, "8leté");
                                ui.selectable_value(&mut t.typ, TypStudia::Ctyrlete, "4leté");
                            });
                        egui::ComboBox::from_id_source(("cis-km", i)).selected_text(&t.kmenova).width(90.0).show_ui(
                            ui,
                            |ui| {
                                for m in mistnosti.iter() {
                                    ui.selectable_value(&mut t.kmenova, m.id.clone(), &m.id);
                                }
                            },
                        );
                        let text = t.naslednik.as_deref().unwrap_or("– (maturita)").to_string();
                        egui::ComboBox::from_id_source(("cis-na", i)).selected_text(text).width(110.0).show_ui(
                            ui,
                            |ui| {
                                ui.selectable_value(&mut t.naslednik, None, "– (maturita)");
                                for (id, n) in &ids {
                                    if *id != t.id {
                                        ui.selectable_value(&mut t.naslednik, Some(id.clone()), n);
                                    }
                                }
                            },
                        );
                        ui.label(t.omezeni().popis());
                        if ui.small_button("✖").clicked() {
                            smazat = Some(i);
                        }
                        ui.end_row();
                    }
                });
                if let Some(i) = smazat {
                    tridy.remove(i);
                }
            }
            1 => {
                egui::Grid::new("cis-m").striped(true).show(ui, |ui| {
                    for h in ["ID", "Název", "Kapacita", "Kmenová", ""] {
                        ui.label(RichText::new(h).strong());
                    }
                    ui.end_row();
                    for (i, m) in mistnosti.iter_mut().enumerate() {
                        ui.label(&m.id);
                        ui.add_sized([240.0, 20.0], egui::TextEdit::singleline(&mut m.nazev));
                        ui.add(egui::DragValue::new(&mut m.kapacita).clamp_range(1..=200));
                        ui.checkbox(&mut m.kmenova, "");
                        if ui.small_button("✖").clicked() {
                            smazat = Some(i);
                        }
                        ui.end_row();
                    }
                });
                if let Some(i) = smazat {
                    mistnosti.remove(i);
                }
            }
            2 => {
                egui::Grid::new("cis-u").striped(true).show(ui, |ui| {
                    for h in ["Zkratka", "Jméno", "Max h/den", "Preferovaná učebna", "Volno", ""] {
                        ui.label(RichText::new(h).strong());
                    }
                    ui.end_row();
                    for (i, u) in ucitele.iter_mut().enumerate() {
                        ui.label(&u.id);
                        ui.add_sized([150.0, 20.0], egui::TextEdit::singleline(&mut u.jmeno));
                        ui.add(egui::DragValue::new(&mut u.max_den).clamp_range(1..=12));
                        let text = u.preferovana_mistnost.clone().unwrap_or_else(|| "—".into());
                        egui::ComboBox::from_id_source(("cis-pm", i)).selected_text(text).width(90.0).show_ui(
                            ui,
                            |ui| {
                                ui.selectable_value(&mut u.preferovana_mistnost, None, "—");
                                for m in mistnosti.iter() {
                                    ui.selectable_value(&mut u.preferovana_mistnost, Some(m.id.clone()), &m.id);
                                }
                            },
                        );
                        let aktivni = self.volno_ucitel.as_deref() == Some(u.id.as_str());
                        if ui
                            .selectable_label(aktivni, format!("📅 {} slotů", u.volno.len()))
                            .on_hover_text("Upravit dny/hodiny, kdy neučí")
                            .clicked()
                        {
                            self.volno_ucitel = if aktivni { None } else { Some(u.id.clone()) };
                        }
                        if ui.small_button("✖").clicked() {
                            smazat = Some(i);
                        }
                        ui.end_row();
                    }
                });
                if let Some(u) = self.volno_ucitel.as_ref().and_then(|id| ucitele.iter_mut().find(|u| &u.id == id)) {
                    ui.separator();
                    ui.label(
                        RichText::new(format!("Volno učitele {} ({}) – zaškrtnuté = neučí", u.id, u.jmeno)).strong(),
                    );
                    egui::Grid::new("cis-volno").show(ui, |ui| {
                        ui.label("");
                        for k in 0..SLOTU {
                            ui.label(RichText::new(CASY[k]).small());
                        }
                        ui.end_row();
                        for d in 0..DNY {
                            ui.label(NAZVY_DNU[d]);
                            for k in 0..SLOTU {
                                let s = (d * SLOTU + k) as u8;
                                let mut b = u.volno.contains(&s);
                                if ui.checkbox(&mut b, "").changed() {
                                    if b {
                                        u.volno.push(s);
                                        u.volno.sort_unstable();
                                    } else {
                                        u.volno.retain(|&x| x != s);
                                    }
                                }
                            }
                            ui.end_row();
                        }
                    });
                }
                if let Some(i) = smazat {
                    ucitele.remove(i);
                }
            }
            _ => {
                egui::Grid::new("cis-p").striped(true).show(ui, |ui| {
                    for h in ["ID", "Název", "Smí do kmenové", "Odborné místnosti", ""] {
                        ui.label(RichText::new(h).strong());
                    }
                    ui.end_row();
                    for (i, p) in predmety.iter_mut().enumerate() {
                        ui.label(&p.id);
                        ui.add_sized([220.0, 20.0], egui::TextEdit::singleline(&mut p.nazev));
                        ui.checkbox(&mut p.kmenova_ok, "");
                        let text = if p.specialni_mistnosti.is_empty() {
                            "—".to_string()
                        } else {
                            p.specialni_mistnosti.join(", ")
                        };
                        ui.menu_button(format!("{} …", text), |ui| {
                            for m in mistnosti.iter().filter(|m| !m.kmenova) {
                                let mut b = p.specialni_mistnosti.contains(&m.id);
                                if ui.checkbox(&mut b, format!("{} – {}", m.id, m.nazev)).changed() {
                                    if b {
                                        p.specialni_mistnosti.push(m.id.clone());
                                    } else {
                                        p.specialni_mistnosti.retain(|x| x != &m.id);
                                    }
                                }
                            }
                        });
                        if ui.small_button("✖").clicked() {
                            smazat = Some(i);
                        }
                        ui.end_row();
                    }
                });
                if let Some(i) = smazat {
                    predmety.remove(i);
                }
            }
        });
    }

    // ───────────── obrazovka Nastavení ─────────────

    fn obrazovka_nastaveni(&mut self, ui: &mut Ui) {
        let Skola { tridy, nastaveni: n, .. } = &mut self.p.skola;
        egui::ScrollArea::vertical().id_source("na-sc").auto_shrink([false, false]).show(ui, |ui| {
            ui.heading("Řešič");
            ui.add(egui::Slider::new(&mut n.casovy_limit_s, 5..=600).logarithmic(true).text("s časový limit"));
            ui.horizontal(|ui| {
                ui.label("Semínko (varianta rozvrhu):");
                ui.add(egui::DragValue::new(&mut n.seed));
                if ui.button("🎲 jiná varianta").clicked() {
                    n.seed = n.seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407) % 100_000;
                }
            });
            ui.separator();
            ui.heading("Omezení ročníků (P1)");
            ui.horizontal(|ui| {
                ui.label("Limity raných/odpoledních hodin platí");
                ui.radio_value(&mut n.limity_za_den, true, "za den");
                ui.radio_value(&mut n.limity_za_den, false, "za týden");
            });
            ui.horizontal(|ui| {
                ui.label("Odpoledne začíná hodinou:");
                egui::ComboBox::from_id_source("na-odpo").selected_text(format!("{}. ({})", n.odpoledne_od, CASY[n.odpoledne_od as usize])).show_ui(ui, |ui| {
                    for k in 4..SLOTU as u8 {
                        ui.selectable_value(&mut n.odpoledne_od, k, format!("{}. ({})", k, CASY[k as usize]));
                    }
                });
            });
            ui.checkbox(&mut n.auto_uvolneni, "Auto-uvolnění: když limity nestačí na hodinovou dotaci, minimálně zvýšit odpolední limit (hlásí se v Reportu)");
            ui.label(RichText::new(format!(
                "Pravidlo 7 hodin (P5): nejvýše {} hodin v kuse, pak pauza ≥ 1 hodina – tvrdé omezení, kontroluje se pro každého žáka. Dále max {} h/den a max {} h stejného předmětu/den.",
                MAX_RUN, MAX_DEN_TRIDA, MAX_STEJNY_PREDMET_DEN
            )).weak());
            ui.separator();
            ui.heading("Váhy měkkých kritérií");
            let v = &mut n.vahy;
            ui.add(egui::Slider::new(&mut v.okno, 0..=20).text("okno (díra) ve dni žáka / slot"));
            ui.add(egui::Slider::new(&mut v.rana, 0..=20).text("den s hodinou v 7:20"));
            ui.add(egui::Slider::new(&mut v.odpoledne_nizsi, 0..=20).text("odpolední hodina (ročník ≤ 5)"));
            ui.add(egui::Slider::new(&mut v.odpoledne_vyssi, 0..=20).text("odpolední hodina (ročník ≥ 6)"));
            ui.add(egui::Slider::new(&mut v.seminar_odpoledne, 0..=20).text("seminář odpoledne / h"));
            ui.add(egui::Slider::new(&mut v.rozlozeni, 0..=20).text("stejný předmět 2× za den"));
            ui.add(egui::Slider::new(&mut v.ucitel_okno, 0..=20).text("okno učitele"));
            if ui.button("Obnovit výchozí váhy").clicked() {
                *v = Vahy::default();
            }
            ui.separator();
            ui.heading("Limity tříd");
            egui::Grid::new("na-limity").striped(true).show(ui, |ui| {
                for h in ["Třída", "Efekt. ročník", "Výchozí", "Přepsat", "Raných", "Odpoledních", "Následník"] {
                    ui.label(RichText::new(h).strong());
                }
                ui.end_row();
                for t in tridy.iter_mut() {
                    ui.label(&t.nazev);
                    ui.label(t.efektivni_rocnik().to_string());
                    ui.label(Omezeni::pro_rocnik(t.efektivni_rocnik()).popis());
                    let mut prepsat = t.omezeni_prepsat.is_some();
                    if ui.checkbox(&mut prepsat, "").changed() {
                        t.omezeni_prepsat = prepsat.then(|| Omezeni::pro_rocnik(t.efektivni_rocnik()));
                    }
                    if let Some(o) = t.omezeni_prepsat.as_mut() {
                        editor_limitu(ui, &mut o.rane);
                        editor_limitu(ui, &mut o.odpoledni);
                    } else {
                        ui.label("–");
                        ui.label("–");
                    }
                    ui.label(t.naslednik.as_deref().unwrap_or("maturita"));
                    ui.end_row();
                }
            });
        });
    }

    // ───────────── obrazovka Report ─────────────

    fn obrazovka_report(&mut self, ui: &mut Ui) {
        if self.zateze.is_none() {
            self.zateze = Some(solver::zateze(&self.p.skola, &self.p.rok));
        }
        if self.kontrola.is_none() {
            if let Some(v) = &self.p.rozvrh {
                self.kontrola = Some(validation::zkontroluj(&self.p.skola, v));
            }
        }
        let skola = &self.p.skola;
        ui.columns(2, |cols| {
            let ui = &mut cols[0];
            ui.heading("Zátěže učitelů");
            ui.label(
                RichText::new("h/týden z plánu a voleb (L/S hodina = 0,5) · ≥ 26 oranžově, ≥ 31 červeně")
                    .weak()
                    .small(),
            );
            egui::ScrollArea::vertical().id_source("re-z").auto_shrink([false, false]).show(ui, |ui| {
                egui::Grid::new("re-zg").striped(true).show(ui, |ui| {
                    for (u, h) in self.zateze.as_deref().unwrap_or_default() {
                        let (barva, pruh) = if *h >= 31.0 {
                            (CERVENA, CERVENA)
                        } else if *h >= 26.0 {
                            (ORANZOVA, ORANZOVA)
                        } else {
                            (ui.visuals().text_color(), ZELENA)
                        };
                        ui.label(RichText::new(u).strong());
                        ui.label(skola.ucitel(u).map_or("?", |x| x.jmeno.as_str()));
                        ui.label(RichText::new(format!("{:.1}", h)).color(barva));
                        let pomer = (h / 31.0).min(1.0);
                        ui.add(egui::ProgressBar::new(pomer).desired_width(120.0).fill(pruh.gamma_multiply(0.8)));
                        ui.end_row();
                    }
                });
            });
            let ui = &mut cols[1];
            let Some(v) = &self.p.rozvrh else {
                ui.heading("Rozvrh zatím není vygenerován");
                ui.label("Zátěže vlevo se počítají i bez generování.");
                return;
            };
            ui.heading(format!("Rozvrh {}", v.rok));
            ui.label(format!(
                "Lekcí {} · skóre tvrdé {} / měkké {} · iterací {} · {:.1} s",
                v.lekce.len(),
                v.skore.0,
                v.skore.1,
                v.iteraci,
                v.cas_s
            ));
            egui::ScrollArea::vertical().id_source("re-v").auto_shrink([false, false]).show(ui, |ui| {
                ui.collapsing(RichText::new(format!("Varování ({})", v.varovani.len())).color(ORANZOVA), |ui| {
                    for x in &v.varovani {
                        ui.label(RichText::new(format!("⚠ {}", x)).color(ORANZOVA));
                    }
                });
                if v.problemy.is_empty() {
                    ui.label(
                        RichText::new("✔ Žádné tvrdé problémy (kolize, pravidlo 7 h, limity, maxima)").color(ZELENA),
                    );
                } else {
                    egui::CollapsingHeader::new(
                        RichText::new(format!("Problémy ({})", v.problemy.len())).color(CERVENA),
                    )
                    .default_open(true)
                    .show(ui, |ui| {
                        for x in &v.problemy {
                            ui.label(RichText::new(format!("✖ {}", x)).color(CERVENA));
                        }
                    });
                }
                if let Some(k) = &self.kontrola {
                    ui.separator();
                    ui.label(RichText::new("Statistiky tříd (max přes žáky)").strong());
                    egui::Grid::new("re-st").striped(true).show(ui, |ui| {
                        for h in ["Třída", "Žáků", "Hodin", "Dny 7:20", "Odpol. h", "Nejdelší blok", "Okna průměr"]
                        {
                            ui.label(RichText::new(h).strong());
                        }
                        ui.end_row();
                        for s in &k.statistiky {
                            ui.label(skola.nazev_tridy(&s.trida));
                            ui.label(s.zaku.to_string());
                            ui.label(if s.hodin_min == s.hodin_max {
                                s.hodin_max.to_string()
                            } else {
                                format!("{}–{}", s.hodin_min, s.hodin_max)
                            });
                            ui.label(s.rane_dny.to_string());
                            ui.label(s.odpoledni.to_string());
                            let blok = RichText::new(s.nejdelsi_blok.to_string());
                            ui.label(if s.nejdelsi_blok > MAX_RUN { blok.color(CERVENA) } else { blok });
                            ui.label(format!("{:.1}", s.okna_prumer));
                            ui.end_row();
                        }
                    });
                }
            });
        });
    }

    // ───────────── průvodce novým rokem ─────────────

    fn okno_pruvodce(&mut self, ctx: &egui::Context) {
        let Some(mut pr) = self.pruvodce.take() else { return };
        let mut otevreno = true;
        let mut dokoncit = false;
        egui::Window::new("✚ Vytvořit nový rok")
            .open(&mut otevreno)
            .default_size([1050.0, 720.0])
            .collapsible(false)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    for (i, n) in ["0 Rok", "1 Noví žáci", "2 Změny", "3 Volitelné", "4 Dokončení"].iter().enumerate()
                    {
                        let t = RichText::new(*n);
                        ui.label(if i == pr.krok { t.strong().color(MODRA) } else { t.weak() });
                        if i < 4 {
                            ui.label("➡");
                        }
                    }
                });
                ui.separator();
                let vyska = (ui.available_height() - 40.0).max(200.0);
                egui::ScrollArea::vertical().id_source("pr-sc").max_height(vyska).auto_shrink([false, false]).show(
                    ui,
                    |ui| match pr.krok {
                        0 => pr.krok0(ui, &self.p),
                        1 => pr.krok1(ui, &self.p),
                        2 => pr.krok2(ui, &self.p),
                        3 => pr.krok3(ui, &self.p),
                        _ => pr.krok4(ui, &self.p),
                    },
                );
                ui.separator();
                ui.horizontal(|ui| {
                    if pr.krok > 0 && ui.button("◀ Zpět").clicked() {
                        pr.krok -= 1;
                    }
                    if pr.krok < 4 && ui.button(RichText::new("Další ▶").strong()).clicked() {
                        if pr.krok == 2 {
                            pr.novy = Some(rok::novy_rok(&self.p.skola, &self.p.rok, &pr.zmeny, &pr.novi, &pr.label));
                            pr.menu_sel = 0;
                        }
                        pr.krok += 1;
                    }
                    if pr.krok == 4 {
                        let b = egui::Button::new(
                            RichText::new("✔ Vytvořit rok a vygenerovat rozvrh").color(Color32::WHITE).strong(),
                        )
                        .fill(Color32::from_rgb(40, 140, 70));
                        if ui.add_enabled(pr.novy.is_some(), b).clicked() {
                            dokoncit = true;
                        }
                    }
                });
            });
        if dokoncit {
            if let Some(novy) = pr.novy.take() {
                let stary = std::mem::replace(&mut self.p.rok, novy);
                self.p.historie.push(stary);
                self.p.piny.clear();
                self.p.rozvrh = None;
                self.undo.clear();
                self.vybrana = None;
                self.menu_sel = 0;
                self.zateze = None;
                self.kontrola = None;
                self.spust();
                self.obr = Obrazovka::Report;
                self.zprava = format!("Vytvořen rok {} – generuji rozvrh…", self.p.rok.label);
            }
            return;
        }
        if otevreno {
            self.pruvodce = Some(pr);
        }
    }

    fn okno_otevrit(&mut self, ctx: &egui::Context) {
        if !self.otevrit_okno {
            return;
        }
        let mut otevreno = true;
        let mut nacist = false;
        egui::Window::new("📁 Otevřít datový soubor").open(&mut otevreno).show(ctx, |ui| {
            ui.label("Cesta k souboru JSON:");
            ui.add_sized([420.0, 20.0], egui::TextEdit::singleline(&mut self.cesta_text));
            if ui.button("Otevřít").clicked() {
                nacist = true;
            }
        });
        if nacist {
            self.otevri(PathBuf::from(self.cesta_text.trim()));
            otevreno = false;
        }
        self.otevrit_okno = otevreno;
    }
}

impl Pruvodce {
    fn new(p: &Projekt) -> Pruvodce {
        let vstupni = p.skola.vstupni_tridy();
        Pruvodce {
            krok: 0,
            label: dalsi_label(&p.rok.label),
            texty: vstupni.iter().map(|t| (t.clone(), String::new())).collect(),
            novi: vstupni.iter().map(|t| (t.clone(), Vec::new())).collect(),
            zmeny: p.rok.studenti.iter().map(|s| (s.id, rok::vychozi_zmena(s))).collect(),
            filtr: String::new(),
            novy: None,
            menu_sel: 0,
            rozdelit: None,
        }
    }

    fn krok0(&mut self, ui: &mut Ui, p: &Projekt) {
        ui.heading("Nový školní rok");
        ui.horizontal(|ui| {
            ui.label("Označení nového roku:");
            ui.text_edit_singleline(&mut self.label);
        });
        let aktivni = p.rok.studenti.iter().filter(|s| s.v_rozvrhu()).count();
        let maturanti = p
            .rok
            .studenti
            .iter()
            .filter(|s| s.v_rozvrhu() && p.skola.trida(&s.trida).is_some_and(|t| t.naslednik.is_none()))
            .count();
        ui.label(format!(
            "Aktuální rok {}: {} aktivních žáků, z toho {} maturantů (odejdou automaticky).",
            p.rok.label, aktivni, maturanti
        ));
        ui.label("Postup: 1) noví žáci do vstupních tříd, 2) propadnutí a odchody, 3) volitelné předměty, 4) vytvoření roku a generování rozvrhu.");
        ui.label(
            RichText::new(
                "Aktuální rok se uloží do historie (zdroj pro import voleb a katalogů). Připnuté hodiny se vymažou.",
            )
            .weak(),
        );
    }

    fn krok1(&mut self, ui: &mut Ui, p: &Projekt) {
        ui.label("Zadejte jména nových žáků – jeden žák na řádek. Pohlaví se odhadne ze jména (lze doplnit „; D“ / „; CH“ nebo upravit v tabulce).");
        for t in p.skola.vstupni_tridy() {
            ui.separator();
            ui.heading(format!("{} – noví žáci", p.skola.nazev_tridy(&t)));
            ui.horizontal_top(|ui| {
                let text = self.texty.entry(t.clone()).or_default();
                ui.add(
                    egui::TextEdit::multiline(text)
                        .desired_rows(14)
                        .desired_width(260.0)
                        .hint_text("Jana Nováková\nPetr Svoboda\n…"),
                );
                ui.vertical(|ui| {
                    if ui.button(RichText::new("Načíst a rozdělit (15:15)").strong()).clicked() {
                        let zaci = rok::novi_zaci_z_textu(self.texty.get(&t).map(|s| s.as_str()).unwrap_or(""));
                        self.novi.insert(t.clone(), zaci);
                    }
                    let zaci = self.novi.entry(t.clone()).or_default();
                    if zaci.is_empty() {
                        ui.label(RichText::new("zatím žádní žáci").weak());
                        return;
                    }
                    let d = zaci.iter().filter(|z| z.pohlavi == Pohlavi::D).count();
                    let a = zaci.iter().filter(|z| z.skupina == SkupinaAj::A).count();
                    ui.label(
                        RichText::new(format!(
                            "{} žáků ({} D + {} CH) · AJa {} / AJb {}",
                            zaci.len(),
                            d,
                            zaci.len() - d,
                            a,
                            zaci.len() - a
                        ))
                        .strong(),
                    );
                    if ui.button("⚖ Přerozdělit 15:15").clicked() {
                        let vstup: Vec<(String, Pohlavi)> = zaci.iter().map(|z| (z.jmeno.clone(), z.pohlavi)).collect();
                        for (z, g) in zaci.iter_mut().zip(rok::rozdel_1515(&vstup)) {
                            z.skupina = g;
                        }
                    }
                    let mut smazat = None;
                    egui::Grid::new(("pr1", &t)).striped(true).show(ui, |ui| {
                        for (i, z) in zaci.iter_mut().enumerate() {
                            ui.add_sized([180.0, 20.0], egui::TextEdit::singleline(&mut z.jmeno));
                            egui::ComboBox::from_id_source(("pr1p", &t, i))
                                .selected_text(z.pohlavi.zkratka())
                                .width(45.0)
                                .show_ui(ui, |ui| {
                                    ui.selectable_value(&mut z.pohlavi, Pohlavi::D, "D");
                                    ui.selectable_value(&mut z.pohlavi, Pohlavi::CH, "CH");
                                });
                            ui.label(RichText::new(z.skupina.nazev()).strong());
                            if ui.small_button("➡AJa").clicked() {
                                z.skupina = SkupinaAj::A;
                            }
                            if ui.small_button("➡AJb").clicked() {
                                z.skupina = SkupinaAj::B;
                            }
                            if ui.small_button("✖").clicked() {
                                smazat = Some(i);
                            }
                            ui.end_row();
                        }
                    });
                    if let Some(i) = smazat {
                        zaci.remove(i);
                    }
                });
            });
        }
    }

    fn krok2(&mut self, ui: &mut Ui, p: &Projekt) {
        ui.label("Kdo propadá (zůstává ve stejné třídě) a kdo odchází. Maturanti odcházejí automaticky.");
        ui.horizontal(|ui| {
            ui.label("Třída:");
            let text = if self.filtr.is_empty() { "(všechny)".to_string() } else { p.skola.nazev_tridy(&self.filtr) };
            egui::ComboBox::from_id_source("pr2-f").selected_text(text).show_ui(ui, |ui| {
                ui.selectable_value(&mut self.filtr, String::new(), "(všechny)");
                for t in &p.skola.tridy {
                    ui.selectable_value(&mut self.filtr, t.id.clone(), &t.nazev);
                }
            });
            let prop = self.zmeny.values().filter(|z| **z == Zmena::Propada).count();
            let odch = self.zmeny.values().filter(|z| **z == Zmena::Odchazi).count();
            ui.label(format!("propadá {} · odchází {}", prop, odch));
        });
        egui::Grid::new("pr2").striped(true).show(ui, |ui| {
            for h in ["Žák", "Třída", "Nový rok", "Změna"] {
                ui.label(RichText::new(h).strong());
            }
            ui.end_row();
            for s in p.rok.studenti.iter().filter(|s| self.filtr.is_empty() || s.trida == self.filtr) {
                let zmena = self.zmeny.entry(s.id).or_insert(Zmena::Postupuje);
                let odesel = s.status == StavStudenta::Odesel;
                let cil = if odesel { None } else { rok::cilova_trida(&p.skola, s, *zmena) };
                let seda = odesel || (cil.is_none() && *zmena == Zmena::Postupuje);
                let jm = RichText::new(&s.jmeno);
                ui.label(if seda { jm.weak() } else { jm });
                ui.label(p.skola.nazev_tridy(&s.trida));
                let cil_text = match (&cil, odesel, *zmena) {
                    (_, true, _) => "už odešel".to_string(),
                    (Some(c), _, _) => p.skola.nazev_tridy(c),
                    (None, _, Zmena::Postupuje) => "maturita".to_string(),
                    (None, _, _) => "odchází".to_string(),
                };
                ui.label(if seda { RichText::new(cil_text).weak() } else { RichText::new(cil_text) });
                ui.horizontal(|ui| {
                    ui.add_enabled_ui(!odesel, |ui| {
                        for z in [Zmena::Postupuje, Zmena::Propada, Zmena::Odchazi] {
                            ui.radio_value(zmena, z, z.nazev());
                        }
                    });
                });
                ui.end_row();
            }
        });
    }

    fn krok3(&mut self, ui: &mut Ui, p: &Projekt) {
        let Some(novy) = self.novy.as_mut() else {
            ui.label("Nejdřív projděte krok 2.");
            return;
        };
        ui.label(RichText::new("Volby se automaticky přenesly z minulého roku (pokračující jazyk, VV/DV…). Návrat do kroků 1–2 úpravy voleb zahodí.").weak());
        editor_voleb(ui, &p.skola, novy, Some(&p.rok), &mut self.menu_sel, &mut self.rozdelit, "pr3");
    }

    fn krok4(&mut self, ui: &mut Ui, p: &Projekt) {
        let Some(novy) = self.novy.as_ref() else {
            ui.label("Nejdřív projděte kroky 1–3.");
            return;
        };
        ui.heading(format!("Souhrn roku {}", novy.label));
        egui::Grid::new("pr4").striped(true).show(ui, |ui| {
            for h in ["Třída", "Žáků", "D / CH", "AJa / AJb"] {
                ui.label(RichText::new(h).strong());
            }
            ui.end_row();
            for t in &p.skola.tridy {
                let z: Vec<&Student> = novy.studenti.iter().filter(|s| s.trida == t.id).collect();
                let d = z.iter().filter(|s| s.pohlavi == Pohlavi::D).count();
                let a = z.iter().filter(|s| s.skupina_aj == SkupinaAj::A).count();
                ui.label(&t.nazev);
                ui.label(z.len().to_string());
                ui.label(format!("{} / {}", d, z.len() - d));
                ui.label(format!("{} / {}", a, z.len() - a));
                ui.end_row();
            }
        });
        ui.separator();
        ui.label(RichText::new("Kompletnost voleb").strong());
        for m in &novy.menu {
            let clenove: Vec<&Student> = novy.studenti.iter().filter(|s| m.tridy.contains(&s.trida)).collect();
            let chybi =
                clenove.iter().filter(|s| s.volby.get(&m.id).map_or(0, |v| v.len()) < m.pocet_voleb as usize).count();
            let t = format!("{}: {} žáků, nekompletních {}", m.nazev, clenove.len(), chybi);
            ui.label(if chybi > 0 {
                RichText::new(format!("⚠ {}", t)).color(ORANZOVA)
            } else {
                RichText::new(format!("✔ {}", t)).color(ZELENA)
            });
        }
        ui.label(
            RichText::new("Po potvrzení se starý rok přesune do historie, piny se vymažou a spustí se generování.")
                .weak(),
        );
    }
}

impl eframe::App for RozvrhApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.kontrola_behu(ctx);
        if !ctx.wants_keyboard_input() && self.obr == Obrazovka::Rozvrh {
            if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
                self.vybrana = None;
            }
            if ctx.input(|i| i.modifiers.command && i.key_pressed(egui::Key::Z)) {
                self.zpet();
            }
        }
        egui::TopBottomPanel::top("lista").show(ctx, |ui| {
            ui.add_space(3.0);
            self.horni_lista(ui);
            ui.add_space(2.0);
        });
        egui::CentralPanel::default().show(ctx, |ui| match self.obr {
            Obrazovka::Rozvrh => self.obrazovka_rozvrh(ui),
            Obrazovka::Studenti => self.obrazovka_studenti(ui),
            Obrazovka::Volitelne => {
                let stary = self.p.historie.last();
                editor_voleb(ui, &self.p.skola, &mut self.p.rok, stary, &mut self.menu_sel, &mut self.rozdelit, "vol");
            }
            Obrazovka::Plan => self.obrazovka_plan(ui),
            Obrazovka::Ciselniky => self.obrazovka_ciselniky(ui),
            Obrazovka::Nastaveni => self.obrazovka_nastaveni(ui),
            Obrazovka::Report => self.obrazovka_report(ui),
        });
        self.okno_pruvodce(ctx);
        self.okno_otevrit(ctx);
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        if let Some(b) = &self.beh {
            b.stop.store(true, Ordering::Relaxed);
        }
        if let Err(e) = self.p.uloz(&self.cesta) {
            eprintln!("Automatické uložení selhalo: {e}");
        }
    }
}

// ───────────────────────────── editor voleb (obrazovka i průvodce) ─────────────────────────────

fn editor_voleb(
    ui: &mut Ui,
    skola: &Skola,
    rok: &mut SkolniRok,
    stary: Option<&SkolniRok>,
    sel: &mut usize,
    rozdelit: &mut Rozdelit,
    salt: &str,
) {
    let mut nove_menu = false;
    let mut smazat_menu = false;
    let mut import_katalogu = false;
    let mut import_voleb = false;
    let mut doplnit = false;
    if !rok.menu.is_empty() {
        *sel = (*sel).min(rok.menu.len() - 1);
    }
    ui.horizontal_wrapped(|ui| {
        ui.label("Menu:");
        let text = rok.menu.get(*sel).map_or("—".to_string(), |m| m.nazev.clone());
        egui::ComboBox::from_id_source((salt, "menu")).selected_text(text).width(340.0).show_ui(ui, |ui| {
            for (i, m) in rok.menu.iter().enumerate() {
                ui.selectable_value(sel, i, &m.nazev);
            }
        });
        nove_menu = ui.button("+ Nové menu").clicked();
        smazat_menu = !rok.menu.is_empty() && ui.button("✖ Smazat menu").clicked();
        if stary.is_some() {
            import_katalogu = ui
                .button("⬇ Katalog z minulého roku")
                .on_hover_text("Nahradí katalog voleb tohoto menu verzí z minulého roku")
                .clicked();
            import_voleb = ui
                .button("⬇ Volby z minulého roku")
                .on_hover_text("Doplní volby, které žáci měli loni a v menu existují")
                .clicked();
        }
        doplnit = ui
            .button("📝 Doplnit chybějící")
            .on_hover_text("Žákům bez volby přiřadí nejméně obsazenou volbu (s ohledem na kapacity) – pro rychlý návrh")
            .clicked();
    });
    if nove_menu {
        let n = rok.menu.len() + 1;
        rok.menu.push(Menu {
            id: format!("menu-{}", n),
            nazev: format!("Nové menu {}", n),
            tridy: vec![],
            volby: vec![],
            hodin_tydne: 2,
            dvojhodina: false,
            pocet_voleb: 1,
        });
        *sel = rok.menu.len() - 1;
        return;
    }
    if rok.menu.is_empty() {
        ui.label("Žádné menu voleb.");
        return;
    }
    let mi = *sel;
    let menu_id = rok.menu[mi].id.clone();
    if smazat_menu {
        rok.menu.remove(mi);
        *sel = 0;
        return;
    }
    if let (true, Some(st)) = (import_katalogu, stary) {
        rok::import_katalogu(rok, st, &menu_id);
    }
    if let (true, Some(st)) = (import_voleb, stary) {
        rok::import_voleb(rok, st, Some(&menu_id));
    }
    if doplnit {
        rok::doplnit_volby(rok, &menu_id, rand_seed());
    }

    let obs = rok::obsazenost(rok, &rok.menu[mi]);
    {
        let m = &mut rok.menu[mi];
        ui.horizontal_wrapped(|ui| {
            ui.label("Název:");
            ui.add_sized([260.0, 20.0], egui::TextEdit::singleline(&mut m.nazev));
            ui.label("h/týden");
            ui.add(egui::DragValue::new(&mut m.hodin_tydne).clamp_range(1..=12));
            ui.checkbox(&mut m.dvojhodina, "2h bloky");
            ui.label("voleb na žáka");
            ui.add(egui::DragValue::new(&mut m.pocet_voleb).clamp_range(1..=10));
            ui.label(
                RichText::new(if m.pocet_voleb == 1 {
                    "(všechny volby paralelně v jednom slotu)"
                } else {
                    "(volby se plánují podle skutečných kombinací žáků)"
                })
                .weak(),
            );
        });
        ui.horizontal_wrapped(|ui| {
            ui.label("Třídy:");
            for t in &skola.tridy {
                let mut b = m.tridy.contains(&t.id);
                if ui.checkbox(&mut b, &t.nazev).changed() {
                    if b {
                        m.tridy.push(t.id.clone());
                    } else {
                        m.tridy.retain(|x| x != &t.id);
                    }
                }
            }
        });
        ui.separator();
        ui.label(RichText::new("Katalog voleb").strong());
        let mut smazat = None;
        egui::Grid::new((salt, "katalog")).striped(true).show(ui, |ui| {
            for h in ["ID", "Název", "Předmět", "Učitel", "Žáků", "Kapacita", "", ""] {
                ui.label(RichText::new(h).strong());
            }
            ui.end_row();
            for (vi, v) in m.volby.iter_mut().enumerate() {
                ui.label(&v.id);
                ui.add_sized([200.0, 20.0], egui::TextEdit::singleline(&mut v.nazev));
                combo_predmet(ui, (salt, "kp", vi), &skola.predmety, &mut v.predmet);
                combo_ucitel(ui, (salt, "ku", vi), &skola.ucitele, &mut v.ucitel, 80.0);
                let pocet = obs.get(&v.id).copied().unwrap_or(0);
                let pres = v.kapacita.is_some_and(|k| pocet > k as usize);
                let t = RichText::new(pocet.to_string()).strong();
                ui.label(if pres { t.color(CERVENA) } else { t });
                ui.horizontal(|ui| {
                    let mut ma = v.kapacita.is_some();
                    if ui.checkbox(&mut ma, "").changed() {
                        v.kapacita = ma.then_some(v.kapacita.unwrap_or(24));
                    }
                    if let Some(k) = v.kapacita.as_mut() {
                        ui.add(egui::DragValue::new(k).clamp_range(1..=200));
                    }
                });
                if ui
                    .small_button("✂")
                    .on_hover_text("Rozdělit skupinu na dvě (druhá polovina žáků k jinému učiteli)")
                    .clicked()
                {
                    *rozdelit = Some((menu_id.clone(), v.id.clone(), v.ucitel.clone()));
                }
                if ui.small_button("✖").clicked() {
                    smazat = Some(vi);
                }
                ui.end_row();
            }
        });
        if let Some(i) = smazat {
            m.volby.remove(i);
        }
        if ui.button("+ Přidat volbu").clicked() {
            let mut n = m.volby.len() + 1;
            while m.volby.iter().any(|v| v.id == format!("V{}", n)) {
                n += 1;
            }
            m.volby.push(Volba {
                id: format!("V{}", n),
                nazev: format!("Volba {}", n),
                predmet: skola.predmety.first().map(|p| p.id.clone()).unwrap_or_default(),
                ucitel: skola.ucitele.first().map(|u| u.id.clone()).unwrap_or_default(),
                kapacita: None,
            });
        }
    }
    let mut provest: Option<(String, String, String)> = None;
    let mut zrusit = false;
    if let Some((mid, vid, uc)) = rozdelit.as_mut() {
        if *mid == menu_id {
            egui::Frame::group(ui.style()).show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(format!("✂ Rozdělit volbu {} – učitel nové skupiny:", vid)).strong());
                    combo_ucitel(ui, (salt, "roz"), &skola.ucitele, uc, 90.0);
                    if ui.button("Rozdělit").clicked() {
                        provest = Some((mid.clone(), vid.clone(), uc.clone()));
                    }
                    if ui.button("Zrušit").clicked() {
                        zrusit = true;
                    }
                });
            });
        }
    }
    if let Some((mid, vid, uc)) = provest {
        rok::rozdel_skupinu(rok, &mid, &vid, &uc);
        *rozdelit = None;
    }
    if zrusit {
        *rozdelit = None;
    }

    ui.separator();
    let menu = rok.menu[mi].clone();
    let k = menu.pocet_voleb.max(1) as usize;
    let mut nekompletni = 0;
    let mut celkem = 0;
    let nazev_volby = |id: &str| menu.volby.iter().find(|v| v.id == id).map_or("—".to_string(), |v| v.nazev.clone());
    egui::ScrollArea::vertical().id_source((salt, "zaci-sc")).auto_shrink([false, false]).show(ui, |ui| {
        egui::Grid::new((salt, "zaci")).striped(true).show(ui, |ui| {
            ui.label(RichText::new("Žák").strong());
            ui.label(RichText::new("Třída").strong());
            for c in 0..k {
                ui.label(RichText::new(format!("Volba {}", c + 1)).strong());
            }
            ui.end_row();
            for s in rok.studenti.iter_mut().filter(|s| s.v_rozvrhu() && menu.tridy.contains(&s.trida)) {
                celkem += 1;
                let puvodni = s.volby.get(&menu.id).cloned().unwrap_or_default();
                let mut v = puvodni.clone();
                if v.len() < k {
                    v.resize(k, String::new());
                }
                ui.label(&s.jmeno);
                ui.label(skola.nazev_tridy(&s.trida));
                for (c, x) in v.iter_mut().enumerate().take(k) {
                    egui::ComboBox::from_id_source((salt, "zv", s.id, c))
                        .selected_text(nazev_volby(x))
                        .width(170.0)
                        .show_ui(ui, |ui| {
                            ui.selectable_value(x, String::new(), "—");
                            for vol in &menu.volby {
                                ui.selectable_value(x, vol.id.clone(), &vol.nazev);
                            }
                        });
                }
                ui.end_row();
                let mut nove: Vec<String> = Vec::new();
                for x in v {
                    if !x.is_empty() && !nove.contains(&x) {
                        nove.push(x);
                    }
                }
                if nove.len() < k {
                    nekompletni += 1;
                }
                if nove != puvodni {
                    if nove.is_empty() {
                        s.volby.remove(&menu.id);
                    } else {
                        s.volby.insert(menu.id.clone(), nove);
                    }
                }
            }
        });
    });
    let t = format!("{} žáků v menu · {} nemá kompletní volby", celkem, nekompletni);
    ui.label(if nekompletni > 0 {
        RichText::new(format!("⚠ {}", t)).color(ORANZOVA)
    } else {
        RichText::new(format!("✔ {}", t)).color(ZELENA)
    });
}

// ───────────────────────────── pomocné widgety ─────────────────────────────

fn combo_trida(ui: &mut Ui, id: impl std::hash::Hash, tridy: &[Trida], val: &mut String) {
    let text = tridy.iter().find(|t| &t.id == val).map_or(val.clone(), |t| t.nazev.clone());
    egui::ComboBox::from_id_source(id).selected_text(text).width(110.0).show_ui(ui, |ui| {
        for t in tridy {
            ui.selectable_value(val, t.id.clone(), &t.nazev);
        }
    });
}

fn combo_ucitel(ui: &mut Ui, id: impl std::hash::Hash, ucitele: &[Ucitel], val: &mut String, sirka: f32) {
    egui::ComboBox::from_id_source(id).selected_text(val.as_str()).width(sirka).show_ui(ui, |ui| {
        for u in ucitele {
            ui.selectable_value(val, u.id.clone(), format!("{} – {}", u.id, u.jmeno));
        }
    });
}

fn combo_predmet(ui: &mut Ui, id: impl std::hash::Hash, predmety: &[Predmet], val: &mut String) {
    egui::ComboBox::from_id_source(id).selected_text(val.as_str()).width(80.0).show_ui(ui, |ui| {
        for p in predmety {
            ui.selectable_value(val, p.id.clone(), format!("{} – {}", p.id, p.nazev));
        }
    });
}

fn editor_limitu(ui: &mut Ui, x: &mut Option<u8>) {
    ui.horizontal(|ui| {
        let mut omezeno = x.is_some();
        if ui.checkbox(&mut omezeno, "").on_hover_text("odškrtnuto = bez limitu").changed() {
            *x = omezeno.then_some(0);
        }
        match x.as_mut() {
            Some(v) => {
                ui.add(egui::DragValue::new(v).clamp_range(0..=12));
            }
            None => {
                ui.label("∞");
            }
        }
    });
}

fn druh_struktury(s: &Struktura) -> usize {
    match s {
        Struktura::CelouTridou(_) => 0,
        Struktura::Rozdeleno { .. } => 1,
        Struktura::Pohlavi { .. } => 2,
        Struktura::Stridave { .. } => 3,
        Struktura::StridaveSkupiny { .. } => 4,
    }
}

fn preved_strukturu(s: &Struktura, predmet: &str, druh: usize) -> Struktura {
    let uc = s.ucitele();
    let u = uc.first().map(|x| x.to_string()).unwrap_or_default();
    let u2 = uc.get(1).map(|x| x.to_string()).unwrap_or_else(|| u.clone());
    let pol = |uc: &str| Pol { predmet: predmet.to_string(), ucitel: uc.to_string() };
    match druh {
        0 => Struktura::CelouTridou(u),
        1 => Struktura::Rozdeleno { a: u, b: u2 },
        2 => Struktura::Pohlavi { div: u, chl: u2 },
        3 => Struktura::Stridave { l: pol(&u), s: pol(&u2) },
        _ => Struktura::StridaveSkupiny { a: [pol(&u), pol(&u2)], b: [pol(&u2), pol(&u)] },
    }
}

fn barva_predmetu(predmet: &str, dark: bool) -> Color32 {
    let mut h: u32 = 2_166_136_261;
    for b in predmet.bytes() {
        h = (h ^ b as u32).wrapping_mul(16_777_619);
    }
    let hue = (h % 360) as f32 / 360.0;
    let (s, v) = if dark { (0.45, 0.40) } else { (0.28, 0.97) };
    Color32::from(egui::ecolor::Hsva::new(hue, s, v, 1.0))
}

fn dalsi_label(l: &str) -> String {
    if let Some((a, _)) = l.split_once('/') {
        if let Ok(y) = a.trim().parse::<u32>() {
            return format!("{}/{:02}", y + 1, (y + 2) % 100);
        }
    }
    format!("{} (nový)", l)
}

fn rand_seed() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(7, |d| d.as_nanos() as u64)
}

/// Kam uložit export. Windows/macOS: nativní dialog; Linux: složka ./export (bez GTK).
fn cesta_pro_ulozeni(nazev: &str) -> Option<PathBuf> {
    #[cfg(any(target_os = "windows", target_os = "macos"))]
    {
        rfd::FileDialog::new().set_file_name(nazev).save_file()
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        let d = PathBuf::from("export");
        let _ = std::fs::create_dir_all(&d);
        Some(d.join(nazev))
    }
}

/// Some(cesta/None) = nativní dialog proběhl; None = dialog není k dispozici (Linux) ➡ textové okno.
fn otevrit_dialog() -> Option<Option<PathBuf>> {
    #[cfg(any(target_os = "windows", target_os = "macos"))]
    {
        Some(rfd::FileDialog::new().add_filter("JSON", &["json"]).pick_file())
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        None
    }
}
