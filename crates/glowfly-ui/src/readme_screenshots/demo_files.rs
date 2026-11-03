//! The demo project's files at each step of its history: `tidewatch`, a small made-up Rust
//! command-line tool that prints tide tables for coastal stations. Code lines stay short so a
//! split diff shows them whole.

pub(super) const CARGO_TOML: &str = r#"[package]
name = "tidewatch"
version = "0.1.0"
edition = "2021"

[dependencies]
serde = { version = "1", features = ["derive"] }
toml = "0.8"
"#;

pub(super) const GITIGNORE: &str = "/target\n";

pub(super) const README: &str = "# tidewatch\n\nTide tables for coastal stations, in your terminal.\n";

pub(super) const README_CONFIG: &str = "# tidewatch

Tide tables for coastal stations, in your terminal.

## Configuration

`~/.config/tidewatch/config.toml` holds your home station:

```toml
station = \"harbour-point\"
```
";

pub(super) const MAIN: &str = r#"use std::env;

fn main() {
    let station = env::args().nth(1);
    let station = station.as_deref().unwrap_or("harbour-point");
    println!("Tides for {station}");
}
"#;

pub(super) const MAIN_STATIONS: &str = r#"mod stations;

use std::env;

fn main() {
    let id = env::args().nth(1);
    let id = id.as_deref().unwrap_or("harbour-point");
    let all = stations::load("stations.toml").unwrap();
    match all.find(id) {
        Some(s) => println!("Tides for {}", s.name),
        None => eprintln!("unknown station: {id}"),
    }
}
"#;

pub(super) const STATIONS_TOML: &str = r#"[[station]]
id = "harbour-point"
name = "Harbour Point"

[[station]]
id = "gull-rock"
name = "Gull Rock"
"#;

pub(super) const STATIONS: &str = r#"use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct Station {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Deserialize)]
pub struct Stations {
    station: Vec<Station>,
}

impl Stations {
    pub fn find(&self, id: &str) -> Option<&Station> {
        self.station.iter().find(|s| s.id == id)
    }
}

pub fn load(path: &str) -> Result<Stations, Box<dyn std::error::Error>> {
    Ok(toml::from_str(&std::fs::read_to_string(path)?)?)
}
"#;

pub(super) const CI: &str = r#"name: CI
on: [push, pull_request]
jobs:
  test:
    strategy:
      matrix:
        os: [ubuntu-latest, macos-latest, windows-latest]
    runs-on: ${{ matrix.os }}
    steps:
      - uses: actions/checkout@v4
      - run: cargo test
"#;

pub(super) const FORECAST: &str = r#"use serde::Deserialize;

/// One predicted high or low water.
#[derive(Debug, Deserialize)]
pub struct Tide {
    pub time: String,
    pub height_m: f64,
    pub high: bool,
}

pub fn fetch(station: &str, days: u32) -> Result<Vec<Tide>, ureq::Error> {
    let url = format!("https://tides.example.org/v1/{station}?days={days}");
    Ok(ureq::get(&url).call()?.into_json()?)
}
"#;

/// The table as first printed.
pub(super) const RENDER: &str = r#"//! The tide table.

use crate::forecast::Tide;
use crate::paint::*;

const HEADER: &str = "Time   Tide  Height";

/// Prints one row per tide.
pub fn table(tides: &[Tide]) {
    println!("{HEADER}");
    for w in tides.windows(2) {
        row(&w[0], w[1].high);
    }
}

/// One tide's row.
fn row(t: &Tide, up: bool) {
    let style = Style::new().dim();
    let kind = kind(t.high);
    let h = t.height_m;
    let s = format!("{kind} {h:.1}");
    let cell = style.paint(&s);
    let time = &t.time;
    let a = arrow(up);
    println!("{time} {cell} {a}");
}

/// The tide's kind, as printed.
fn kind(hi: bool) -> &'static str {
    if hi { "High" } else { "Low " }
}

/// What follows a row.
fn arrow(up: bool) -> &'static str {
    if up { "+" } else { "-" }
}
"#;

pub(super) const PAINT: &str = r#"//! Terminal styles.

#[derive(Clone, Copy, Default)]
pub struct Style {
    dim: bool,
}

impl Style {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn dim(self) -> Self {
        Self { dim: true }
    }

    pub fn paint(self, text: &str) -> String {
        if self.dim { format!("\x1b[2m{text}\x1b[0m") } else { text.into() }
    }
}
"#;

pub(super) const PAINT_COLOUR: &str = r#"//! Terminal styles.

#[derive(Clone, Copy)]
pub struct Color(u8);

pub const CYAN: Color = Color(36);
pub const BLUE: Color = Color(34);

#[derive(Clone, Copy, Default)]
pub struct Style {
    fg: Option<Color>,
}

impl Style {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn fg(self, c: Color) -> Self {
        Self { fg: Some(c) }
    }

    pub fn paint(self, text: &str) -> String {
        match self.fg {
            Some(c) => format!("\x1b[{}m{text}\x1b[0m", c as u8),
            None => text.into(),
        }
    }
}
"#;

pub(super) const CSV: &str = r#"//! The table as CSV, for spreadsheets.

use crate::forecast::Tide;

pub fn write(tides: &[Tide]) -> String {
    let mut out = String::from("time,tide,height\n");
    for t in tides {
        let kind = if t.high { "high" } else { "low" };
        out += &format!("{},{kind},{:.2}\n", t.time, t.height_m);
    }
    out
}
"#;

/// The daily summary, before main and moon-phase each changed it.
pub(super) const SUMMARY: &str = r#"//! The summary printed above the table.

use crate::forecast::Tide;

/// The next high water, in one line.
pub fn line(tide: &Tide) -> String {
    let time = &tide.time;
    let height = tide.height;
    format!("High water {time} {height:.1}m")
}

/// How wide the summary's first column is.
const COLUMN_WIDTH: usize = 10;

/// The label column, padded.
pub fn label(text: &str) -> String {
    format!("{text:<COLUMN_WIDTH$}")
}
"#;

pub(super) const MOON: &str = r#"//! The moon's phase, for the daily summary.

/// Days from one new moon to the next.
const MONTH: f64 = 29.530_588;

/// The phase on a day, from 0.0 (new) to 1.0.
pub fn phase(days_since_new: f64) -> f64 {
    (days_since_new / MONTH).fract()
}

/// The phase's name.
pub fn name(phase: f64) -> &'static str {
    match (phase * 8.0) as u8 {
        0 | 7 => "new",
        1 | 2 => "waxing",
        3 | 4 => "full",
        _ => "waning",
    }
}
"#;

pub(super) const UNITS: &str = r#"//! Heights in metres or feet.

pub fn feet(metres: f64) -> f64 {
    metres * 3.2808
}
"#;
