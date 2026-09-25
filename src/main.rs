//! Rozvrhovník GFP – desktopová aplikace (eframe/egui) + režim příkazové řádky.
//!
//! Spuštění GUI:           rozvrhovnik [soubor.json]
//! Generování bez GUI:     rozvrhovnik --generuj [--data soubor.json] [--cas 60] [--seed 1]
//!                                     [--export adresar] [--uloz]

#![cfg_attr(all(not(debug_assertions), target_os = "windows"), windows_subsystem = "windows")]
#![allow(clippy::needless_range_loop, clippy::unnecessary_map_or, clippy::type_complexity)]

mod app;

use rozvrhovnik::{data, export, solver, validation};
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;

const VYCHOZI_SOUBOR: &str = "skola_data.json";

fn nacti_nebo_vychozi(cesta: &Path) -> data::Projekt {
    if cesta.exists() {
        match data::Projekt::nacti(cesta) {
            Ok(p) => return p,
            Err(e) => eprintln!("{e}\nPoužívám výchozí data."),
        }
    }
    data::vychozi()
}

fn hodnota(args: &[String], jmeno: &str) -> Option<String> {
    args.iter().position(|a| a == jmeno).and_then(|i| args.get(i + 1).cloned())
}

fn cli(args: &[String]) -> std::process::ExitCode {
    let cesta = PathBuf::from(hodnota(args, "--data").unwrap_or_else(|| VYCHOZI_SOUBOR.into()));
    let mut p = nacti_nebo_vychozi(&cesta);
    if let Some(c) = hodnota(args, "--cas").and_then(|x| x.parse().ok()) {
        p.skola.nastaveni.casovy_limit_s = c;
    }
    if let Some(s) = hodnota(args, "--seed").and_then(|x| x.parse().ok()) {
        p.skola.nastaveni.seed = s;
    }
    eprintln!(
        "Generuji rozvrh {} ({} žáků, limit {} s)…",
        p.rok.label,
        p.rok.studenti.len(),
        p.skola.nastaveni.casovy_limit_s
    );
    let stop = AtomicBool::new(false);
    let v = solver::generuj(&p.skola, &p.rok, &p.piny, &stop, None);
    let k = validation::zkontroluj(&p.skola, &v);
    let z = solver::zateze(&p.skola, &p.rok);
    println!("{}", export::text_reportu(&p.skola, &v, &k, &z));
    if let Some(dir) = hodnota(args, "--export") {
        let dir = PathBuf::from(dir);
        let _ = std::fs::create_dir_all(&dir);
        let soubory = [
            ("rozvrh_tridy.html", export::html_tridy(&p.skola, &v)),
            ("rozvrh_ucitele.html", export::html_ucitele(&p.skola, &v)),
            ("rozvrh_zaci.html", export::html_zaci(&p.skola, &p.rok, &v)),
            ("rozvrh.csv", export::csv(&p.skola, &v)),
            ("rozvrh.xml", export::xml(&p.skola, &v)),
        ];
        for (nazev, obsah) in soubory {
            let c = dir.join(nazev);
            match std::fs::write(&c, obsah) {
                Ok(()) => eprintln!("Zapsáno {}", c.display()),
                Err(e) => eprintln!("Nelze zapsat {}: {e}", c.display()),
            }
        }
    }
    let bez_problemu = k.problemy.is_empty();
    if args.iter().any(|a| a == "--uloz") {
        p.rozvrh = Some(v);
        if let Err(e) = p.uloz(&cesta) {
            eprintln!("{e}");
        } else {
            eprintln!("Uloženo do {}", cesta.display());
        }
    }
    if bez_problemu {
        std::process::ExitCode::SUCCESS
    } else {
        std::process::ExitCode::from(2)
    }
}

fn main() -> std::process::ExitCode {
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|a| a == "--generuj") {
        return cli(&args);
    }
    if args.iter().any(|a| a == "--help" || a == "-h") {
        println!("{}", include_str!("napoveda.txt"));
        return std::process::ExitCode::SUCCESS;
    }
    let cesta = args
        .iter()
        .skip(1)
        .find(|a| !a.starts_with("--"))
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(VYCHOZI_SOUBOR));
    let projekt = nacti_nebo_vychozi(&cesta);
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([1500.0, 950.0])
            .with_min_inner_size([900.0, 600.0])
            .with_title("Rozvrhovník GFP – generátor rozvrhů"),
        ..Default::default()
    };
    let vysledek = eframe::run_native(
        "Rozvrhovník GFP",
        options,
        Box::new(move |cc| Box::new(app::RozvrhApp::new(cc, projekt, cesta))),
    );
    match vysledek {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("Chyba GUI: {e}");
            std::process::ExitCode::FAILURE
        }
    }
}
