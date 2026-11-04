//! The request body for the Apps Script web app: two sheets, `Runs` (one row per Run) and
//! `Metrics` (one row per number per Run). Numbers go as JSON numbers; an empty cell is `""`. Every
//! row has as many cells as its sheet has headers.
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::{json, Value};

use super::store::{Row, Summary};

pub const RUNS_HEADERS: [&str; 17] = [
    "run id",
    "date and time",
    "party",
    "encounter id",
    "fights",
    "seed",
    "round cap",
    "AI heal threshold %",
    "changes",
    "win rate %",
    "win rate delta",
    "rounds to win p50",
    "rounds to win p50 delta",
    "damage per turn",
    "damage per turn delta",
    "deaths per fight",
    "note",
];

pub const METRICS_HEADERS: [&str; 11] = [
    "run id",
    "date and time",
    "party",
    "encounter id",
    "group",
    "who",
    "metric",
    "previous",
    "value",
    "delta",
    "delta %",
];

/// What identifies a Run and what the bench was set to.
#[derive(Clone, Debug, PartialEq)]
pub struct RunInfo {
    pub run_id: u64,
    pub when: String,
    pub party: Vec<String>,
    pub encounter: String,
    pub fights: u32,
    pub seed: u64,
    pub max_rounds: u16,
    pub heal_below_pct: u8,
    pub changes: String,
    pub note: String,
}

fn num(v: f64) -> Value {
    if v.is_finite() {
        json!(v)
    } else {
        json!(0)
    }
}

fn opt(v: Option<f64>) -> Value {
    v.map_or_else(|| json!(""), num)
}

/// A note starting with `=`, `+`, `-` or `@` gets a leading apostrophe so Sheets keeps it as text.
pub fn note_cell(note: &str) -> String {
    if note.starts_with(['=', '+', '-', '@']) {
        format!("'{note}")
    } else {
        note.to_string()
    }
}

/// `YYYY-MM-DD HH:MM:SS` in UTC.
pub fn format_utc(secs: u64) -> String {
    let (days, rest) = (secs / 86_400, secs % 86_400);
    // Days since 1970-01-01 to a civil date (Howard Hinnant's algorithm).
    let z = days as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + (month <= 2) as i64;
    format!(
        "{year:04}-{month:02}-{day:02} {:02}:{:02}:{:02}",
        rest / 3_600,
        rest % 3_600 / 60,
        rest % 60
    )
}

pub fn now_utc() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    format_utc(secs)
}

/// The two sheets of one Run. `before` is the summary of the previous Run with the same key.
pub fn request(info: &RunInfo, now: &Summary, before: Option<&Summary>, rows: &[Row]) -> Value {
    let party = info.party.join(" + ");
    let delta = |now: f64, was: Option<f64>| was.map(|w| now - w);
    let run = json!([
        info.run_id,
        info.when,
        party,
        info.encounter,
        info.fights,
        info.seed,
        info.max_rounds,
        info.heal_below_pct,
        info.changes,
        num(now.win_rate),
        opt(delta(now.win_rate, before.map(|b| b.win_rate))),
        num(now.rounds_p50),
        opt(delta(now.rounds_p50, before.map(|b| b.rounds_p50))),
        num(now.damage_per_turn),
        opt(delta(
            now.damage_per_turn,
            before.map(|b| b.damage_per_turn)
        )),
        num(now.deaths_per_fight),
        note_cell(&info.note),
    ]);
    let metrics: Vec<Value> = rows
        .iter()
        .map(|r| {
            json!([
                info.run_id,
                info.when,
                party,
                info.encounter,
                r.group,
                r.who,
                r.metric,
                opt(r.previous),
                num(r.value),
                opt(r.delta),
                opt(r.delta_pct),
            ])
        })
        .collect();
    json!({
        "sheets": [
            {"name": "Runs", "headers": RUNS_HEADERS, "rows": [run]},
            {"name": "Metrics", "headers": METRICS_HEADERS, "rows": metrics},
        ]
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn info(note: &str) -> RunInfo {
        RunInfo {
            run_id: 7,
            when: "2026-11-04 10:00:00".into(),
            party: vec!["mage".into(), "warrior".into()],
            encounter: "wolves".into(),
            fights: 1000,
            seed: 3,
            max_rounds: 60,
            heal_below_pct: 40,
            changes: "a: 1 -> 2".into(),
            note: note.into(),
        }
    }

    fn rows() -> Vec<Row> {
        vec![
            Row {
                group: "party".into(),
                who: "party".into(),
                metric: "win rate %".into(),
                previous: Some(50.0),
                value: 60.0,
                delta: Some(10.0),
                delta_pct: Some(20.0),
            },
            Row {
                group: "hero".into(),
                who: "mage".into(),
                metric: "crits".into(),
                previous: None,
                value: 2.5,
                delta: None,
                delta_pct: None,
            },
        ]
    }

    #[test]
    fn every_row_has_as_many_cells_as_headers() {
        let body = request(&info("x"), &Summary::default(), None, &rows());
        for sheet in body["sheets"].as_array().unwrap() {
            let headers = sheet["headers"].as_array().unwrap().len();
            for row in sheet["rows"].as_array().unwrap() {
                assert_eq!(row.as_array().unwrap().len(), headers, "{}", sheet["name"]);
            }
        }
        assert_eq!(body["sheets"][1]["rows"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn numbers_are_numbers_and_missing_values_are_empty_cells() {
        let now = Summary {
            win_rate: 60.0,
            rounds_p50: 9.0,
            damage_per_turn: 12.5,
            deaths_per_fight: 0.25,
        };
        let before = Summary {
            win_rate: 50.0,
            rounds_p50: 10.0,
            damage_per_turn: 10.0,
            deaths_per_fight: 0.5,
        };
        let body = request(&info("n"), &now, Some(&before), &rows());
        let run = &body["sheets"][0]["rows"][0];
        assert!(run[0].is_u64() && run[4].is_u64() && run[5].is_u64());
        assert_eq!(run[9], json!(60.0));
        assert_eq!(run[10], json!(10.0));
        assert_eq!(run[12], json!(-1.0));
        assert_eq!(run[14], json!(2.5));
        assert_eq!(run[2], json!("mage + warrior"));
        let first = &body["sheets"][1]["rows"][0];
        assert_eq!(first[7], json!(50.0));
        assert_eq!(first[10], json!(20.0));
        let second = &body["sheets"][1]["rows"][1];
        assert_eq!(
            (&second[7], &second[9], &second[10]),
            (&json!(""), &json!(""), &json!(""))
        );
        let body = request(&info("n"), &now, None, &[]);
        assert_eq!(body["sheets"][0]["rows"][0][10], json!(""));
        assert_eq!(body["sheets"][1]["rows"], json!([]));
    }

    #[test]
    fn a_note_that_looks_like_a_formula_gets_an_apostrophe() {
        for note in ["=1+1", "+5", "-x", "@me"] {
            assert_eq!(note_cell(note), format!("'{note}"));
        }
        assert_eq!(note_cell("plain"), "plain");
        assert_eq!(note_cell(""), "");
        let body = request(&info("=SUM(A1)"), &Summary::default(), None, &[]);
        assert_eq!(body["sheets"][0]["rows"][0][16], json!("'=SUM(A1)"));
    }

    #[test]
    fn utc_dates_are_right() {
        assert_eq!(format_utc(0), "1970-01-01 00:00:00");
        assert_eq!(format_utc(951_782_400), "2000-02-29 00:00:00");
        assert_eq!(format_utc(1_793_786_096), "2026-11-04 09:54:56");
    }
}
