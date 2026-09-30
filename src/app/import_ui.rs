//! Import a export seznamu žáků v GUI: lišta na obrazovce Studenti, náhled importu (režimy,
//! kontrola tříd, problémy), načtení do průvodce novým rokem, výběr souboru bez nativního dialogu
//! (Linux) a přetažení souboru do okna.

use super::*;
use rozvrhovnik::studenti_io::{self, Format, ImportVysledek, Nacteno, Rezim};
use std::path::Path;

/// K čemu se načtený soubor použije.
#[derive(Clone, PartialEq, Debug)]
pub(super) enum Ucel {
    /// Import do aktuálního roku (náhled + režim).
    Studenti,
    /// Noví žáci vstupní třídy v průvodci (id třídy).
    Pruvodce(String),
    /// Rozdělení do skupin (semináře, jazyky) – obrazovka Volitelné.
    Skupiny,
}

pub(super) struct VyberSouboru {
    ucel: Ucel,
    cesta: String,
    chyba: String,
}

pub(super) struct Nahled {
    cesta: PathBuf,
    nacteno: Nacteno,
    rezim: Rezim,
    /// Třída pro řádky, které ji v souboru nemají (id; prázdné = nezařazovat).
    vychozi_trida: String,
}

#[derive(Default)]
pub(super) struct ImportStav {
    vyber: Option<VyberSouboru>,
    nahled: Option<Nahled>,
    vysledek: Option<ImportVysledek>,
    txt_jen_jmena: bool,
}

/// Nativní dialog pro výběr souboru (Windows/macOS). `None` = dialog není k dispozici (Linux).
fn vyber_import_dialog() -> Option<Option<PathBuf>> {
    #[cfg(any(target_os = "windows", target_os = "macos"))]
    {
        Some(rfd::FileDialog::new().add_filter("Seznam žáků", &studenti_io::PRIPONY_IMPORTU).pick_file())
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        None
    }
}

/// Kam uložit export. Windows/macOS: dialog „Uložit jako“; Linux: složka ./export.
fn cesta_pro_export(nazev: &str, f: Format) -> Option<PathBuf> {
    #[cfg(any(target_os = "windows", target_os = "macos"))]
    {
        rfd::FileDialog::new().set_file_name(nazev).add_filter(f.nazev(), &[f.pripona()]).save_file()
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        let _ = f;
        let d = PathBuf::from("export");
        let _ = std::fs::create_dir_all(&d);
        Some(d.join(nazev))
    }
}

/// Soubory se seznamy žáků v aktuální složce a v ./import (pro výběr bez nativního dialogu).
fn najdi_soubory() -> Vec<PathBuf> {
    let mut out = Vec::new();
    for dir in [".", "import"] {
        let Ok(rd) = std::fs::read_dir(dir) else { continue };
        let mut v: Vec<PathBuf> = rd
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.is_file() && studenti_io::je_soubor_importu(p))
            .filter(|p| !p.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.starts_with("skola_data")))
            .collect();
        v.sort();
        out.extend(v);
    }
    out.truncate(40);
    out
}

impl RozvrhApp {
    // ───────────── lišta na obrazovce Studenti ─────────────

    pub(super) fn io_lista(&mut self, ui: &mut Ui) {
        let mut import = false;
        let mut export: Option<Format> = None;
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new("Seznam žáků:").strong());
            if ui
                .button("⬆ Import…")
                .on_hover_text(
                    "txt, csv, Excel (xlsx / xls / ods), DBF nebo json – soubor můžete také přetáhnout do okna",
                )
                .clicked()
            {
                import = true;
            }
            ui.separator();
            ui.label("⬇ Export:");
            for f in Format::VSECHNY {
                if ui.button(f.nazev()).on_hover_text(format!("Uložit seznam žáků jako {}", f.nazev())).clicked() {
                    export = Some(f);
                }
            }
            ui.checkbox(&mut self.io.txt_jen_jmena, "TXT jen jména").on_hover_text("Jeden žák na řádek, jen jméno");
            let rozsah = if self.filtr.is_empty() {
                "všichni žáci".to_string()
            } else {
                self.p.skola.nazev_tridy(&self.filtr)
            };
            ui.label(RichText::new(format!("(export: {rozsah} – podle filtru třídy)")).weak());
        });
        if import {
            self.zahaj_import(Ucel::Studenti);
        }
        if let Some(f) = export {
            self.exportuj_studenty(f);
        }
    }

    fn exportuj_studenty(&mut self, f: Format) {
        let poradi = |t: &str| self.p.skola.tridy.iter().position(|x| x.id == t).unwrap_or(usize::MAX);
        let mut zaci: Vec<&Student> =
            self.p.rok.studenti.iter().filter(|s| self.filtr.is_empty() || s.trida == self.filtr).collect();
        if zaci.is_empty() {
            self.zprava = "Není co exportovat – seznam žáků je prázdný.".into();
            return;
        }
        zaci.sort_by_key(|s| poradi(&s.trida)); // stabilní: pořadí uvnitř třídy zůstává
        let nazev = if self.filtr.is_empty() {
            format!("studenti.{}", f.pripona())
        } else {
            format!("studenti_{}.{}", self.filtr, f.pripona())
        };
        let Some(cesta) = cesta_pro_export(&nazev, f) else { return };
        let moznosti = studenti_io::Moznosti { txt_jen_jmena: self.io.txt_jen_jmena };
        let vysledek = studenti_io::exportuj(&self.p.skola, &self.p.rok, &zaci, f, &cesta, moznosti);
        let n = zaci.len();
        self.zprava = match vysledek {
            Ok(()) => format!("⬇ Exportováno {} žáků ({}) ➡ {}", n, f.nazev(), cesta.display()),
            Err(e) => e,
        };
    }

    // ───────────── načtení souboru ─────────────

    pub(super) fn zahaj_import(&mut self, ucel: Ucel) {
        match vyber_import_dialog() {
            Some(Some(c)) => {
                if let Err(e) = self.nacti_import(c, ucel) {
                    self.zprava = e;
                }
            }
            Some(None) => {}
            None => self.io.vyber = Some(VyberSouboru { ucel, cesta: String::new(), chyba: String::new() }),
        }
    }

    fn nacti_import(&mut self, cesta: PathBuf, ucel: Ucel) -> Result<(), String> {
        if ucel == Ucel::Skupiny {
            return self.nacti_skupiny(cesta);
        }
        let n = studenti_io::nacti(&self.p.skola, &cesta)?;
        match ucel {
            Ucel::Studenti => {
                self.io.nahled = Some(Nahled {
                    cesta,
                    nacteno: n,
                    rezim: Rezim::PridatAktualizovat,
                    vychozi_trida: self.filtr.clone(),
                });
                self.obr = Obrazovka::Studenti;
            }
            Ucel::Pruvodce(t) => self.pruvodce_z_nacteneho(&t, &n, &cesta),
            Ucel::Skupiny => {}
        }
        Ok(())
    }

    /// Noví žáci vstupní třídy v průvodci ze souboru (jména, pohlaví a skupina, jsou-li v souboru;
    /// jinak odhad pohlaví a rozdělení 15:15).
    fn pruvodce_z_nacteneho(&mut self, trida: &str, n: &Nacteno, cesta: &Path) {
        let Some(pr) = self.pruvodce.as_mut() else { return };
        let mut ignorovano = 0;
        let mut zaci: Vec<NovyZak> = Vec::new();
        let mut zadana_skupina: Vec<bool> = Vec::new();
        for z in &n.zaznamy {
            let patri = match &z.trida {
                None => true,
                Some(x) => studenti_io::najdi_tridu(&self.p.skola, x).as_deref() == Some(trida),
            };
            if !patri {
                ignorovano += 1;
                continue;
            }
            zaci.push(NovyZak {
                jmeno: z.jmeno.clone(),
                pohlavi: z.pohlavi.unwrap_or_else(|| rok::odhad_pohlavi(&z.jmeno)),
                skupina: z.skupina.unwrap_or(SkupinaAj::A),
            });
            zadana_skupina.push(z.skupina.is_some());
        }
        if zaci.is_empty() {
            self.zprava = format!(
                "Soubor {} neobsahuje žáky třídy {} (řádků jiných tříd: {}).",
                cesta.display(),
                self.p.skola.nazev_tridy(trida),
                ignorovano
            );
            return;
        }
        if zadana_skupina.iter().all(|z| !z) {
            let vstup: Vec<(String, Pohlavi)> = zaci.iter().map(|z| (z.jmeno.clone(), z.pohlavi)).collect();
            for (z, g) in zaci.iter_mut().zip(rok::rozdel_1515(&vstup)) {
                z.skupina = g;
            }
        } else {
            for i in 0..zaci.len() {
                if zadana_skupina[i] {
                    continue;
                }
                let a = (0..zaci.len()).filter(|&k| k != i && zaci[k].skupina == SkupinaAj::A).count();
                let b = (0..zaci.len()).filter(|&k| k != i && zaci[k].skupina == SkupinaAj::B).count();
                zaci[i].skupina = if a <= b { SkupinaAj::A } else { SkupinaAj::B };
            }
        }
        pr.texty.insert(trida.to_string(), zaci.iter().map(|z| z.jmeno.as_str()).collect::<Vec<_>>().join("\n"));
        let pocet = zaci.len();
        pr.novi.insert(trida.to_string(), zaci);
        let mut zprava = format!(
            "Načteno {} žáků do třídy {} ze souboru {}.",
            pocet,
            self.p.skola.nazev_tridy(trida),
            cesta.display()
        );
        if ignorovano > 0 {
            zprava.push_str(&format!(" Přeskočeno {} řádků jiných tříd.", ignorovano));
        }
        self.zprava = zprava;
    }

    // ───────────── okna ─────────────

    pub(super) fn okna_io(&mut self, ctx: &egui::Context) {
        self.zpracuj_pustene_soubory(ctx);
        self.okno_vyber_souboru(ctx);
        self.okno_nahled_importu(ctx);
        self.okno_vysledek_importu(ctx);
    }

    fn zpracuj_pustene_soubory(&mut self, ctx: &egui::Context) {
        if ctx.input(|i| !i.raw.hovered_files.is_empty()) {
            egui::Area::new(egui::Id::new("drop-hint"))
                .order(egui::Order::Foreground)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .interactable(false)
                .show(ctx, |ui| {
                    egui::Frame::popup(ui.style()).show(ui, |ui| {
                        ui.heading("⬇ Pusťte soubor se seznamem žáků (txt, csv, xlsx, dbf, json)");
                    });
                });
        }
        let dropped: Vec<PathBuf> = ctx.input(|i| i.raw.dropped_files.iter().filter_map(|f| f.path.clone()).collect());
        let Some(cesta) = dropped.into_iter().next() else { return };
        if self.pruvodce.is_some() {
            self.zprava = "Průvodce je otevřený – použijte tlačítko „📂 Načíst ze souboru…“ v kroku 1.".into();
            return;
        }
        self.io.vyber = None;
        // na obrazovce Volitelné se přetažený soubor bere jako rozdělení do skupin
        let ucel = if self.obr == Obrazovka::Volitelne { Ucel::Skupiny } else { Ucel::Studenti };
        if let Err(e) = self.nacti_import(cesta, ucel) {
            self.zprava = e;
        }
    }

    /// Výběr souboru bez nativního dialogu (Linux): cesta textem nebo klik na nalezený soubor.
    fn okno_vyber_souboru(&mut self, ctx: &egui::Context) {
        let Some(mut v) = self.io.vyber.take() else { return };
        let mut otevreno = true;
        let mut nacist = false;
        egui::Window::new("⬆ Import seznamu žáků").open(&mut otevreno).collapsible(false).show(ctx, |ui| {
            ui.label("Formáty: txt, csv, Excel (xlsx, xls, ods), DBF, json. Soubor můžete také přetáhnout do okna.");
            ui.label("Cesta k souboru:");
            ui.horizontal(|ui| {
                ui.add_sized(
                    [420.0, 20.0],
                    egui::TextEdit::singleline(&mut v.cesta).hint_text("např. import/zaci.xlsx"),
                );
                if ui.button("Načíst").clicked() {
                    nacist = true;
                }
            });
            let soubory = najdi_soubory();
            if !soubory.is_empty() {
                ui.separator();
                ui.label(RichText::new("Nalezené soubory (aktuální složka a ./import):").weak());
                egui::ScrollArea::vertical().id_source("io-soubory").max_height(180.0).show(ui, |ui| {
                    for s in soubory {
                        if ui.button(s.display().to_string()).clicked() {
                            v.cesta = s.display().to_string();
                            nacist = true;
                        }
                    }
                });
            }
            if !v.chyba.is_empty() {
                ui.label(RichText::new(&v.chyba).color(CERVENA));
            }
        });
        if nacist {
            match self.nacti_import(PathBuf::from(v.cesta.trim()), v.ucel.clone()) {
                Ok(()) => otevreno = false,
                Err(e) => v.chyba = e,
            }
        }
        if otevreno {
            self.io.vyber = Some(v);
        }
    }

    fn okno_nahled_importu(&mut self, ctx: &egui::Context) {
        let Some(mut nh) = self.io.nahled.take() else { return };
        let skola = &self.p.skola;
        let plan = studenti_io::naplanuj(skola, &nh.nacteno.zaznamy, &nh.vychozi_trida);
        let bez_tridy = nh.nacteno.zaznamy.iter().filter(|z| z.trida.is_none()).count();
        let stavajicich = self.p.rok.studenti.len();
        let ke_smazani = match nh.rezim {
            Rezim::PridatAktualizovat => 0,
            Rezim::NahraditVse => stavajicich,
            Rezim::NahraditTridy => {
                let tridy: std::collections::BTreeSet<&str> = plan.polozky.iter().map(|p| p.trida.as_str()).collect();
                self.p.rok.studenti.iter().filter(|s| tridy.contains(s.trida.as_str())).count()
            }
        };
        let nazev_souboru = nh.cesta.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        let mut otevreno = true;
        let mut importovat = false;
        let mut zrusit = false;
        egui::Window::new(format!("⬆ Import žáků – {nazev_souboru}"))
            .id(egui::Id::new("io-nahled"))
            .open(&mut otevreno)
            .collapsible(false)
            .default_width(780.0)
            .show(ctx, |ui| {
                ui.label(format!(
                    "{} · {} žáků v souboru · {}",
                    nh.nacteno.format.nazev(),
                    nh.nacteno.zaznamy.len(),
                    nh.nacteno.popis_sloupcu
                ));
                if !nh.nacteno.upozorneni.is_empty() {
                    egui::CollapsingHeader::new(
                        RichText::new(format!("⚠ {} upozornění při čtení", nh.nacteno.upozorneni.len()))
                            .color(ORANZOVA),
                    )
                    .id_source("io-upozorneni")
                    .show(ui, |ui| {
                        egui::ScrollArea::vertical().id_source("io-upoz-sc").max_height(90.0).show(ui, |ui| {
                            for u in &nh.nacteno.upozorneni {
                                ui.label(RichText::new(u).color(ORANZOVA).small());
                            }
                        });
                    });
                }
                if bez_tridy > 0 {
                    ui.horizontal(|ui| {
                        ui.label(format!("{bez_tridy} žáků nemá v souboru třídu – zařadit do:"));
                        let text = if nh.vychozi_trida.is_empty() {
                            "(nezařazovat)".to_string()
                        } else {
                            skola.nazev_tridy(&nh.vychozi_trida)
                        };
                        egui::ComboBox::from_id_source("io-vychozi").selected_text(text).width(140.0).show_ui(
                            ui,
                            |ui| {
                                ui.selectable_value(&mut nh.vychozi_trida, String::new(), "(nezařazovat)");
                                for t in &skola.tridy {
                                    ui.selectable_value(&mut nh.vychozi_trida, t.id.clone(), &t.nazev);
                                }
                            },
                        );
                    });
                }
                ui.separator();
                let podle = plan.podle_trid(skola);
                ui.horizontal_wrapped(|ui| {
                    ui.label(RichText::new(format!("Bude zařazeno {} žáků:", plan.polozky.len())).strong());
                    for (t, n) in &podle {
                        ui.label(format!("{} {}", skola.nazev_tridy(t), n));
                        ui.label("·");
                    }
                });
                if !plan.problemy.is_empty() {
                    egui::CollapsingHeader::new(
                        RichText::new(format!("✖ {} řádků nelze zařadit", plan.problemy.len())).color(CERVENA),
                    )
                    .id_source("io-problemy")
                    .default_open(true)
                    .show(ui, |ui| {
                        egui::ScrollArea::vertical().id_source("io-prob-sc").max_height(90.0).show(ui, |ui| {
                            for p in &plan.problemy {
                                ui.label(RichText::new(p).color(CERVENA).small());
                            }
                        });
                    });
                }
                ui.separator();
                ui.label(RichText::new("Jak importovat").strong());
                for r in Rezim::VSECHNY {
                    ui.radio_value(&mut nh.rezim, r, r.nazev());
                }
                ui.label(RichText::new(nh.rezim.popis()).weak().small());
                if ke_smazani > 0 {
                    ui.label(
                        RichText::new(format!("⚠ Smaže se {ke_smazani} stávajících žáků (včetně jejich voleb)."))
                            .color(CERVENA)
                            .strong(),
                    );
                }
                ui.separator();
                ui.label(RichText::new("Náhled (prvních 12 žáků)").strong());
                egui::Grid::new("io-nahled-grid").striped(true).show(ui, |ui| {
                    for h in ["Jméno", "Třída", "AJ", "Pohlaví", "Stav", "Volby"] {
                        ui.label(RichText::new(h).strong());
                    }
                    ui.end_row();
                    for p in plan.polozky.iter().take(12) {
                        let z = &p.zaznam;
                        ui.label(&z.jmeno);
                        ui.label(skola.nazev_tridy(&p.trida));
                        ui.label(z.skupina.map_or("auto".to_string(), |g| g.nazev().to_string()));
                        ui.label(z.pohlavi.map_or("odhad".to_string(), |g| g.zkratka().to_string()));
                        ui.label(z.stav.map_or("aktivní".to_string(), |s| s.nazev().to_string()));
                        ui.label(if z.volby.is_empty() { String::new() } else { format!("{} menu", z.volby.len()) });
                        ui.end_row();
                    }
                });
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    let b = egui::Button::new(
                        RichText::new(format!("✔ Importovat {} žáků", plan.polozky.len()))
                            .color(Color32::WHITE)
                            .strong(),
                    )
                    .fill(Color32::from_rgb(40, 140, 70));
                    if ui.add_enabled(!plan.polozky.is_empty(), b).clicked() {
                        importovat = true;
                    }
                    if ui.button("Zrušit").clicked() {
                        zrusit = true;
                    }
                });
            });
        if importovat {
            let mut v = studenti_io::aplikuj(&self.p.skola, &mut self.p.rok, &plan, nh.rezim);
            let mut vsechna = plan.problemy.clone();
            vsechna.append(&mut v.varovani);
            v.varovani = vsechna;
            self.zprava = format!("{} Přegenerujte rozvrh.", v.shrnuti());
            self.zateze = None;
            self.vyuka = None;
            self.kontrola = None;
            self.io.vysledek = Some(v);
            return;
        }
        if otevreno && !zrusit {
            self.io.nahled = Some(nh);
        }
    }

    fn okno_vysledek_importu(&mut self, ctx: &egui::Context) {
        let Some(v) = self.io.vysledek.take() else { return };
        let mut otevreno = true;
        egui::Window::new("✔ Výsledek importu").open(&mut otevreno).collapsible(false).show(ctx, |ui| {
            ui.label(RichText::new(v.shrnuti()).strong());
            if v.varovani.is_empty() {
                ui.label(RichText::new("Bez upozornění.").color(ZELENA));
            } else {
                ui.label(RichText::new(format!("⚠ {} upozornění:", v.varovani.len())).color(ORANZOVA));
                egui::ScrollArea::vertical().id_source("io-vysl-sc").max_height(220.0).show(ui, |ui| {
                    for x in &v.varovani {
                        ui.label(RichText::new(x).color(ORANZOVA).small());
                    }
                });
            }
            ui.label(RichText::new("Rozvrh je třeba přegenerovat (▶ Generovat).").weak());
        });
        if otevreno && !ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.io.vysledek = Some(v);
        }
    }
}
