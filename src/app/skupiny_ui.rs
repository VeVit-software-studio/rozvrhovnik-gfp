//! Import skupin (semináře, jazykové skupiny) ve formátu školního nástroje – náhled nanečisto
//! a přenesení do menu voleb.

use super::*;
use rozvrhovnik::skupiny_io::{self, Moznosti, NactenoSkupiny};

pub(super) struct NahledSkupin {
    cesta: PathBuf,
    nacteno: NactenoSkupiny,
    menu_id: String,
    moznosti: Moznosti,
    /// Výsledek zkušebního importu na kopii dat pro (menu, možnosti).
    zkouska: Option<((String, Moznosti), skupiny_io::Vysledek)>,
}

#[derive(Default)]
pub(super) struct SkupinyStav {
    nahled: Option<NahledSkupin>,
    vysledek: Option<skupiny_io::Vysledek>,
}

impl RozvrhApp {
    /// Lišta nad editorem voleb.
    pub(super) fn skupiny_lista(&mut self, ui: &mut Ui) {
        let mut import = false;
        ui.horizontal_wrapped(|ui| {
            if ui
                .button("⬆ Import skupin…")
                .on_hover_text(
                    "Rozdělení žáků do seminářů / jazykových skupin – formát nástroje na semináře:\n\
                     Ročník | Seminář | Mezitřídní | Třídy | Počet žáků | Jméno žáka | Třída žáka (| Blok | Učitel)\n\
                     csv, txt, Excel nebo DBF; soubor lze také přetáhnout do okna",
                )
                .clicked()
            {
                import = true;
            }
            ui.label(
                RichText::new("rozdělení do skupin z nástroje na semináře (csv / Excel / DBF) – přepíše volby menu")
                    .weak(),
            );
        });
        if import {
            self.zahaj_import(import_ui::Ucel::Skupiny);
        }
    }

    pub(super) fn nacti_skupiny(&mut self, cesta: PathBuf) -> Result<(), String> {
        let n = skupiny_io::nacti(&cesta)?;
        let menu_id = skupiny_io::navrhni_menu(&self.p.skola, &self.p.rok, &n)
            .or_else(|| self.p.rok.menu.get(self.menu_sel).map(|m| m.id.clone()))
            .unwrap_or_default();
        self.skupiny.nahled =
            Some(NahledSkupin { cesta, nacteno: n, menu_id, moznosti: Moznosti::default(), zkouska: None });
        self.obr = Obrazovka::Volitelne;
        Ok(())
    }

    pub(super) fn okna_skupin(&mut self, ctx: &egui::Context) {
        self.okno_nahled_skupin(ctx);
        self.okno_vysledek_skupin(ctx);
    }

    fn okno_nahled_skupin(&mut self, ctx: &egui::Context) {
        let Some(mut nh) = self.skupiny.nahled.take() else { return };
        // zkušební import na kopii dat (přepočítá se jen při změně menu / možností)
        let klic = (nh.menu_id.clone(), nh.moznosti);
        if nh.zkouska.as_ref().map(|(k, _)| k) != Some(&klic) {
            let (mut skola, mut rok) = (self.p.skola.clone(), self.p.rok.clone());
            let v = skupiny_io::aplikuj(&mut skola, &mut rok, &nh.menu_id, &nh.nacteno, nh.moznosti);
            nh.zkouska = Some((klic, v));
        }
        let skola = &self.p.skola;
        let rok = &self.p.rok;
        let nazev = nh.cesta.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        let mut otevreno = true;
        let (mut importovat, mut zrusit) = (false, false);
        egui::Window::new(format!("⬆ Import skupin – {nazev}"))
            .id(egui::Id::new("sk-nahled"))
            .open(&mut otevreno)
            .collapsible(false)
            .default_width(820.0)
            .show(ctx, |ui| {
                let zaku: usize = nh.nacteno.skupiny.iter().map(|g| g.zaci.len()).sum();
                ui.label(format!("{} skupin, {} zápisů žáků · {}", nh.nacteno.skupiny.len(), zaku, nh.nacteno.popis_sloupcu));
                if !nh.nacteno.upozorneni.is_empty() {
                    egui::CollapsingHeader::new(
                        RichText::new(format!("⚠ {} upozornění v souboru", nh.nacteno.upozorneni.len())).color(ORANZOVA),
                    )
                    .id_source("sk-upoz")
                    .show(ui, |ui| {
                        for u in &nh.nacteno.upozorneni {
                            ui.label(RichText::new(u).color(ORANZOVA).small());
                        }
                    });
                }
                egui::ScrollArea::vertical().id_source("sk-skupiny").max_height(200.0).show(ui, |ui| {
                    egui::Grid::new("sk-grid").striped(true).show(ui, |ui| {
                        for h in ["Skupina", "Ročník", "Třídy", "Žáků", "Blok", "Učitel"] {
                            ui.label(RichText::new(h).strong());
                        }
                        ui.end_row();
                        for g in &nh.nacteno.skupiny {
                            ui.label(RichText::new(&g.kod).strong());
                            ui.label(&g.rocnik);
                            ui.label(&g.tridy);
                            let pocet = match g.pocet_uvedeny {
                                Some(p) if p != g.zaci.len() => {
                                    RichText::new(format!("{} (uvedeno {p})", g.zaci.len())).color(ORANZOVA)
                                }
                                _ => RichText::new(g.zaci.len().to_string()),
                            };
                            ui.label(pocet);
                            ui.label(g.blok.map_or(String::new(), |b| b.to_string()));
                            ui.label(g.ucitel.clone().unwrap_or_default());
                            ui.end_row();
                        }
                    });
                });
                ui.separator();
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Do menu:").strong());
                    let text = rok.menu.iter().find(|m| m.id == nh.menu_id).map_or("—".to_string(), |m| m.nazev.clone());
                    egui::ComboBox::from_id_source("sk-menu").selected_text(text).width(340.0).show_ui(ui, |ui| {
                        for m in &rok.menu {
                            ui.selectable_value(&mut nh.menu_id, m.id.clone(), &m.nazev);
                        }
                    });
                });
                ui.checkbox(&mut nh.moznosti.pridat_chybejici_zaky, "Žáky, kteří v seznamu studentů nejsou, přidat")
                    .on_hover_text("Jinak se nahlásí a přeskočí");
                ui.checkbox(&mut nh.moznosti.odebrat_volby_mimo_soubor, "Z katalogu menu odebrat volby, které v souboru nejsou");
                ui.label(
                    RichText::new(
                        "Soubor je úplný seznam pro dané menu: volby tohoto menu u žáků jeho tříd se nahradí obsahem souboru.",
                    )
                    .weak()
                    .small(),
                );
                ui.separator();
                if let Some((_, v)) = &nh.zkouska {
                    ui.label(RichText::new(format!("Zkušebně: {}", v.shrnuti())).strong());
                    let mut info = Vec::new();
                    if !v.predmety_nove.is_empty() {
                        info.push(format!("nové předměty: {}", v.predmety_nove.join(", ")));
                    }
                    if !v.tridy_pridane.is_empty() {
                        info.push(format!("třídy přidané do menu: {}", v.tridy_pridane.join(", ")));
                    }
                    if !v.volby_odebrane.is_empty() {
                        info.push(format!("odebrané volby: {}", v.volby_odebrane.join(", ")));
                    }
                    for i in info {
                        ui.label(RichText::new(i).small());
                    }
                    for (nadpis, polozky, barva, id) in
                        [("problémů", &v.problemy, CERVENA, "sk-prob"), ("upozornění", &v.varovani, ORANZOVA, "sk-var")]
                    {
                        if polozky.is_empty() {
                            continue;
                        }
                        egui::CollapsingHeader::new(RichText::new(format!("{} {nadpis}", polozky.len())).color(barva))
                            .id_source(id)
                            .default_open(barva == CERVENA)
                            .show(ui, |ui| {
                                egui::ScrollArea::vertical().id_source((id, "sc")).max_height(120.0).show(ui, |ui| {
                                    for x in polozky {
                                        ui.label(RichText::new(x).color(barva).small());
                                    }
                                });
                            });
                    }
                }
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    let b = egui::Button::new(RichText::new("✔ Importovat skupiny").color(Color32::WHITE).strong())
                        .fill(Color32::from_rgb(40, 140, 70));
                    if ui.add_enabled(!nh.menu_id.is_empty(), b).clicked() {
                        importovat = true;
                    }
                    if ui.button("Zrušit").clicked() {
                        zrusit = true;
                    }
                });
                let _ = skola;
            });
        if importovat {
            let v = skupiny_io::aplikuj(&mut self.p.skola, &mut self.p.rok, &nh.menu_id, &nh.nacteno, nh.moznosti);
            if let Some(i) = self.p.rok.menu.iter().position(|m| m.id == nh.menu_id) {
                self.menu_sel = i;
            }
            self.zprava = format!("{} Přegenerujte rozvrh.", v.shrnuti());
            self.zateze = None;
            self.vyuka = None;
            self.kontrola = None;
            self.skupiny.vysledek = Some(v);
            return;
        }
        if otevreno && !zrusit {
            self.skupiny.nahled = Some(nh);
        }
    }

    fn okno_vysledek_skupin(&mut self, ctx: &egui::Context) {
        let Some(v) = self.skupiny.vysledek.take() else { return };
        let mut otevreno = true;
        egui::Window::new("✔ Výsledek importu skupin").open(&mut otevreno).collapsible(false).show(ctx, |ui| {
            ui.label(RichText::new(v.shrnuti()).strong());
            for (nadpis, polozky, barva) in [("Problémy", &v.problemy, CERVENA), ("Upozornění", &v.varovani, ORANZOVA)]
            {
                if polozky.is_empty() {
                    continue;
                }
                ui.label(RichText::new(format!("{nadpis} ({}):", polozky.len())).color(barva));
                egui::ScrollArea::vertical().id_source(nadpis).max_height(160.0).show(ui, |ui| {
                    for x in polozky {
                        ui.label(RichText::new(x).color(barva).small());
                    }
                });
            }
            ui.label(
                RichText::new("Zkontrolujte učitele nových skupin v katalogu voleb a přegenerujte rozvrh.").weak(),
            );
        });
        if otevreno && !ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.skupiny.vysledek = Some(v);
        }
    }
}

// ───────────────────────────── obrazovka Bloky seminářů ─────────────────────────────

#[derive(Default)]
pub(super) struct BlokyStav {
    menu: String,
    vybrana: Option<String>,
    info: String,
    /// (menu, pořadí žáků: jméno, jeho volby, počet bloků po změně)
    kdo_brani: Option<(String, Vec<(String, Vec<String>, usize)>)>,
}

fn barva_kolize(pocet: usize) -> Color32 {
    match pocet {
        0 => Color32::TRANSPARENT,
        1..=2 => Color32::from_rgb(120, 90, 40),
        3..=5 => Color32::from_rgb(150, 80, 40),
        _ => Color32::from_rgb(170, 50, 45),
    }
}

impl RozvrhApp {
    pub(super) fn obrazovka_bloky(&mut self, ui: &mut Ui) {
        use rozvrhovnik::bloky;
        let seminarove: Vec<(String, String)> =
            self.p.rok.menu.iter().filter(|m| m.pocet_voleb > 1).map(|m| (m.id.clone(), m.nazev.clone())).collect();
        if seminarove.is_empty() {
            ui.heading("Bloky seminářů");
            ui.label("Žádné menu s více volbami na žáka (semináře). Založte ho na obrazovce Volitelné.");
            return;
        }
        if !seminarove.iter().any(|(id, _)| *id == self.bloky.menu) {
            self.bloky.menu = seminarove[0].0.clone();
        }
        let Some(mi) = self.p.rok.menu.iter().position(|m| m.id == self.bloky.menu) else { return };
        let a = bloky::analyzuj(&self.p.skola, &self.p.rok, &self.p.rok.menu[mi]);
        let mut prirazeni = bloky::bloky_menu(&self.p.rok.menu[mi], &a);
        let max_blok = prirazeni.iter().flatten().copied().max().unwrap_or(0);
        let kolize = bloky::kontrola(&a, &prirazeni);
        let v_kolizi: BTreeSet<usize> = kolize.iter().flat_map(|k| [k.a, k.b]).collect();

        let mut akce_navrh: Option<bool> = None; // Some(true) = jen nepřiřazené
        let mut zrusit = false;
        let mut export = false;
        ui.horizontal_wrapped(|ui| {
            ui.heading("⊞ Bloky seminářů");
            ui.label("Menu:");
            let text = seminarove.iter().find(|(id, _)| *id == self.bloky.menu).map_or("", |(_, n)| n.as_str());
            egui::ComboBox::from_id_source("bl-menu").selected_text(text).width(300.0).show_ui(ui, |ui| {
                for (id, n) in &seminarove {
                    ui.selectable_value(&mut self.bloky.menu, id.clone(), n);
                }
            });
            if ui
                .button(RichText::new("🔀 Navrhnout bloky").strong())
                .on_hover_text(
                    "Nejmenší počet bloků bez kolizí (společní žáci, učitel, jediná učebna), vyrovnané počty žáků",
                )
                .clicked()
            {
                akce_navrh = Some(false);
            }
            if ui
                .button("Doplnit nepřiřazené")
                .on_hover_text("Stávající bloky zůstanou, navrhnou se jen chybějící")
                .clicked()
            {
                akce_navrh = Some(true);
            }
            if ui.button("✖ Zrušit bloky").clicked() {
                zrusit = true;
            }
            if ui
                .button("⬇ Export (CSV)")
                .on_hover_text("Blok, seminář, učitel, třídy a seznam žáků – lze znovu naimportovat")
                .clicked()
            {
                export = true;
            }
        });
        let dolni = a.max_voleb_zaka;
        ui.label(format!(
            "{} seminářů · {} žáků s volbou · nejvíc voleb jednoho žáka {} (= nejméně bloků) · kolizních dvojic {} · přiřazeno {} bloků",
            a.n(),
            a.zaku_celkem,
            dolni,
            a.kolize.len(),
            max_blok
        ));
        if !self.bloky.info.is_empty() {
            ui.label(RichText::new(&self.bloky.info).color(MODRA));
        }
        // vejdou se bloky do týdne?
        if max_blok > 0 {
            let zatizeni = bloky::tydenni_zatizeni(&self.p.skola, &self.p.rok, &self.p.rok.menu[mi], max_blok as usize);
            let limit = (DNY * MAX_DEN_TRIDA) as u32;
            let pres: Vec<String> = zatizeni
                .iter()
                .filter(|(_, p, b)| p + b > limit)
                .map(|(t, p, b)| format!("{} {} h + {} h bloků = {} h", self.p.skola.nazev_tridy(t), p, b, p + b))
                .collect();
            let hodin_bloku = zatizeni.first().map_or(0, |z| z.2);
            let hodin_zaka = self.p.rok.menu[mi].pocet_voleb as u32 * self.p.rok.menu[mi].hodin_tydne as u32;
            if pres.is_empty() {
                ui.label(RichText::new(format!(
                    "Bloky zaberou {hodin_bloku} h týdně, žák v nich má {hodin_zaka} h seminářů (zbytek = volno mezi semináři)."
                )).weak());
            } else {
                ui.label(
                    RichText::new(format!(
                        "⚠ Bloky se nevejdou do týdne (max {limit} h): {} – je potřeba méně bloků, tj. upravit volby žáků.",
                        pres.join("; ")
                    ))
                    .color(CERVENA),
                );
            }
        }
        // kdo brání menšímu počtu bloků
        let slabe = bloky::slabe_kolize(&a, 2);
        if !slabe.is_empty() {
            let mut spocitat = false;
            egui::CollapsingHeader::new(
                RichText::new(format!("Kdo brání menšímu počtu bloků – {} kolizí způsobených 1–2 žáky", slabe.len()))
                    .color(ORANZOVA),
            )
            .id_source("bl-slabe")
            .show(ui, |ui| {
                if ui
                    .button("Najít žáky, kteří brání menšímu počtu bloků")
                    .on_hover_text("Postupně vybírá žáky s nejvíc kolizemi, které nikdo jiný nesdílí")
                    .clicked()
                {
                    spocitat = true;
                }
                if let Some((m, poradi)) = &self.bloky.kdo_brani {
                    if *m == self.bloky.menu {
                        if poradi.is_empty() {
                            ui.label("Menší počet bloků nebrání jednotlivci – kolize sdílí víc žáků (je třeba změnit nabídku).");
                        }
                        egui::Grid::new("bl-kdo").striped(true).show(ui, |ui| {
                            for h in ["Žák (změnit jednu z voleb)", "Jeho volby", "Pak stačí bloků"] {
                                ui.label(RichText::new(h).strong());
                            }
                            ui.end_row();
                            for (i, (jmeno, volby, k)) in poradi.iter().enumerate() {
                                ui.label(format!("{}. {}", i + 1, jmeno));
                                ui.label(volby.join(", "));
                                ui.label(RichText::new(k.to_string()).strong());
                                ui.end_row();
                            }
                        });
                    }
                }
                ui.label(RichText::new("Kolize, které způsobuje jen 1–2 žáků:").weak());
                egui::ScrollArea::vertical().id_source("bl-slabe-sc").max_height(150.0).show(ui, |ui| {
                    for (i, j, z) in &slabe {
                        ui.label(format!("{} × {} – jen {}", a.volby[*i], a.volby[*j], z.join(", ")));
                    }
                });
            });
            if spocitat {
                let jmena: BTreeMap<u32, String> =
                    self.p.rok.studenti.iter().map(|s| (s.id, s.jmeno.clone())).collect();
                let poradi = bloky::kdo_brani(&a, &jmena, std::time::Duration::from_secs(4));
                self.bloky.kdo_brani = Some((self.bloky.menu.clone(), poradi));
            }
        }
        if kolize.is_empty() {
            if max_blok > 0 && prirazeni.iter().all(|b| b.is_some()) {
                ui.label(
                    RichText::new("✔ Bloky jsou bez kolizí – řešič plánuje semináře jednoho bloku současně.")
                        .color(ZELENA),
                );
            } else if max_blok > 0 {
                ui.label(RichText::new("⚠ Některé semináře nemají blok – plánují se samostatně.").color(ORANZOVA));
            }
        } else {
            egui::CollapsingHeader::new(RichText::new(format!("✖ {} kolizí v blocích", kolize.len())).color(CERVENA))
                .id_source("bl-kolize")
                .default_open(true)
                .show(ui, |ui| {
                    for k in &kolize {
                        let d: Vec<String> = k.duvody.iter().map(|d| d.popis()).collect();
                        ui.label(
                            RichText::new(format!(
                                "Blok {}: {} × {} – {}",
                                k.blok,
                                a.volby[k.a],
                                a.volby[k.b],
                                d.join("; ")
                            ))
                            .color(CERVENA),
                        );
                    }
                });
        }
        ui.separator();

        // bloky vedle sebe
        let mut presun: Option<(usize, Option<u8>)> = None;
        let menu = &self.p.rok.menu[mi];
        egui::ScrollArea::vertical()
            .id_source("bl-sc")
            .auto_shrink([false, false])
            .max_height((ui.available_height() - 40.0).max(200.0))
            .show(ui, |ui| {
                let sloupce: Vec<Option<u8>> = (1..=max_blok)
                    .map(Some)
                    .chain(std::iter::once(None))
                    .filter(|b| b.is_some() || prirazeni.iter().any(|x| x.is_none()))
                    .collect();
                let na_radek = ((ui.available_width() + 8.0) / 232.0).floor().max(1.0) as usize;
                for radek in sloupce.chunks(na_radek) {
                    ui.horizontal_top(|ui| {
                        for &blok in radek {
                            let clenove: Vec<usize> = (0..a.n()).filter(|&i| prirazeni[i] == blok).collect();
                            egui::Frame::group(ui.style()).show(ui, |ui| {
                                ui.set_width(205.0);
                                ui.vertical(|ui| {
                                    match blok {
                                        Some(b) => {
                                            let (v_bloku, volno) = bloky::obsazeni_bloku(&a, &prirazeni, b);
                                            ui.label(RichText::new(format!("Blok {b}")).strong().size(16.0));
                                            ui.label(
                                                RichText::new(format!("{v_bloku} žáků · {volno} má volno")).small(),
                                            );
                                        }
                                        None => {
                                            ui.label(RichText::new("Bez bloku").strong().size(16.0).color(ORANZOVA));
                                        }
                                    }
                                    ui.separator();
                                    for i in clenove {
                                        let v = &menu.volby[i];
                                        let problem = v_kolizi.contains(&i);
                                        let text =
                                            format!("{}  ·  {} žáků\n{} · {}", v.id, a.pocty[i], v.ucitel, v.nazev);
                                        let mut rt = RichText::new(text).size(12.0);
                                        if problem {
                                            rt = rt.color(CERVENA);
                                        }
                                        let vybrana = self.bloky.vybrana.as_deref() == Some(v.id.as_str());
                                        ui.horizontal(|ui| {
                                            if ui.selectable_label(vybrana, rt).clicked() {
                                                self.bloky.vybrana = if vybrana { None } else { Some(v.id.clone()) };
                                            }
                                            let kam = bloky::kam_muze(&a, &prirazeni, i);
                                            egui::ComboBox::from_id_source(("bl-kam", i))
                                                .selected_text("➡")
                                                .width(30.0)
                                                .show_ui(ui, |ui| {
                                                    for (b, k) in &kam {
                                                        let t = if k.is_empty() {
                                                            format!("Blok {b} ✔")
                                                        } else {
                                                            format!("Blok {b} ⚠ {} kolizí", k.len())
                                                        };
                                                        if ui.selectable_label(prirazeni[i] == Some(*b), t).clicked() {
                                                            presun = Some((i, Some(*b)));
                                                        }
                                                    }
                                                    if ui
                                                        .selectable_label(prirazeni[i].is_none(), "bez bloku")
                                                        .clicked()
                                                    {
                                                        presun = Some((i, None));
                                                    }
                                                });
                                        });
                                    }
                                });
                            });
                        }
                    });
                }
                // detail vybraného semináře
                if let Some(i) = self.bloky.vybrana.as_ref().and_then(|id| a.index(id)) {
                    ui.separator();
                    ui.label(RichText::new(format!("Kam může {} ({} žáků):", a.volby[i], a.pocty[i])).strong());
                    for (b, k) in bloky::kam_muze(&a, &prirazeni, i) {
                        if k.is_empty() {
                            ui.label(RichText::new(format!("Blok {b}: ✔ bez kolizí")).color(ZELENA));
                        } else {
                            let popis: Vec<String> = k
                                .iter()
                                .map(|(j, d)| {
                                    format!(
                                        "{} ({})",
                                        a.volby[*j],
                                        d.iter().map(|x| x.popis()).collect::<Vec<_>>().join("; ")
                                    )
                                })
                                .collect();
                            ui.label(RichText::new(format!("Blok {b}: ✖ {}", popis.join(", "))).color(CERVENA));
                        }
                    }
                }
                ui.separator();
                egui::CollapsingHeader::new(
                    "Matice kolizí (počet společných žáků, U = stejný učitel, M = stejná učebna)",
                )
                .id_source("bl-matice")
                .show(ui, |ui| {
                    egui::Grid::new("bl-matice-grid").spacing([2.0, 2.0]).show(ui, |ui| {
                        ui.label("");
                        for id in &a.volby {
                            ui.label(RichText::new(id).small().strong());
                        }
                        ui.end_row();
                        for i in 0..a.n() {
                            ui.label(RichText::new(&a.volby[i]).small().strong());
                            for j in 0..a.n() {
                                let mut text = String::new();
                                let pocet = a.spolecnych_zaku(i, j);
                                if pocet > 0 {
                                    text.push_str(&pocet.to_string());
                                }
                                for d in a.duvody(i, j) {
                                    match d {
                                        bloky::Duvod::Ucitel(_) => text.push('U'),
                                        bloky::Duvod::Mistnost(_) => text.push('M'),
                                        bloky::Duvod::Zaci(_) => {}
                                    }
                                }
                                let fill = if i == j {
                                    Color32::from_gray(60)
                                } else if a.koliduje(i, j) {
                                    barva_kolize(pocet.max(3))
                                } else {
                                    Color32::TRANSPARENT
                                };
                                egui::Frame::none().fill(fill).inner_margin(2.0).show(ui, |ui| {
                                    ui.set_min_size(egui::vec2(28.0, 16.0));
                                    ui.label(RichText::new(text).small());
                                });
                            }
                            ui.end_row();
                        }
                    });
                });
            });

        // akce
        if let Some((i, b)) = presun {
            prirazeni[i] = b;
            bloky::nastav_bloky(&mut self.p.rok.menu[mi], &a, &prirazeni);
            self.bloky.info.clear();
        }
        if let Some(jen_chybejici) = akce_navrh {
            let pevne: Vec<Option<u8>> = if jen_chybejici { prirazeni.clone() } else { vec![None; a.n()] };
            let n = bloky::navrhni(&a, &pevne, std::time::Duration::from_secs(3));
            let nove: Vec<Option<u8>> = n.bloky.iter().map(|&b| Some(b)).collect();
            bloky::nastav_bloky(&mut self.p.rok.menu[mi], &a, &nove);
            let proc = if n.klika.len() >= a.max_voleb_zaka && n.klika.len() > 1 {
                format!(
                    "méně nejde – tyto semináře se navzájem vylučují (každá dvojice má společného žáka, učitele nebo učebnu): {}",
                    n.klika.join(", ")
                )
            } else {
                format!("méně nejde – žák může mít až {} seminářů, každý v jiném bloku", a.max_voleb_zaka)
            };
            self.bloky.info = if n.optimalni {
                format!("Navrženo {} bloků, nejmenší možný počet: {proc}.", n.pocet_bloku)
            } else {
                format!(
                    "Navrženo {} bloků (v časovém limitu nedokázáno, že méně nejde; dolní mez {}).",
                    n.pocet_bloku, n.dolni_mez
                )
            };
            self.zprava = "Bloky seminářů upraveny – přegenerujte rozvrh.".into();
        }
        if zrusit {
            bloky::nastav_bloky(&mut self.p.rok.menu[mi], &a, &vec![None; a.n()]);
            self.bloky.info = "Bloky zrušeny – semináře se plánují samostatně podle voleb žáků.".into();
        }
        if export {
            let menu = &self.p.rok.menu[mi];
            let obsah = bloky::export_csv(&self.p.skola, &self.p.rok, menu);
            if let Some(c) = cesta_pro_ulozeni(&format!("bloky_{}.csv", menu.id)) {
                self.zprava = match std::fs::write(&c, obsah) {
                    Ok(()) => format!("⬇ Bloky uloženy: {}", c.display()),
                    Err(e) => format!("Nelze zapsat {}: {e}", c.display()),
                };
            }
        }
    }
}
