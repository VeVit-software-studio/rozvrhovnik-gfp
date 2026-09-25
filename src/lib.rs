//! Generátor rozvrhů – Gymnázium Františka Palackého, Neratovice.
//!
//! Knihovní část (bez GUI): datový model a seed dat, přechod mezi školními roky,
//! řešič rozvrhu, validace a exporty. GUI je v `src/app.rs` (binárka).

// Smyčky přes indexy dnů/slotů (0..DNY, 0..SLOTU) jsou v rozvrhu čitelnější než iterátory;
// `% 2 == 0` a `map_or` ponecháváme kvůli kompatibilitě se staršími překladači.
#![allow(
    clippy::needless_range_loop,
    clippy::manual_is_multiple_of,
    clippy::unnecessary_map_or,
    clippy::type_complexity,
    clippy::explicit_counter_loop,
    clippy::manual_repeat_n
)]

pub mod data;
pub mod export;
pub mod profily;
pub mod rok;
pub mod solver;
pub mod validation;
