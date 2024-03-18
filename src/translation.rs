use std::collections::{BTreeMap, HashMap};

use once_cell::sync::Lazy;
use regex::Regex;

static ROW_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"^[\s]*(?P<minmax>(?:[0-9\-\|#!]+[ \t]+)+)"(?P<description>.*\s*)"(?P<quantifier>(?:[ \t]*[\w%]+)*)[ \t]*[\r\n]*$"#).unwrap()
});
static DESCRIPTION_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"(?:^"(?P<header>.*)"$)|(?:^include "(?P<include>.*)")|(?:^no_description[\s]*(?P<no_description>[\w+%]*)[\s]*$)|(?P<description>^description[\s]*(?P<identifier>[\S]*)[\s]*$)"#).unwrap()
});
static STATS_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r#"^[\s]*(?P<stat_id_count>[0-9]+) (?P<stat_ids>.*+)$"#).unwrap());
static LANG_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r#"^[\s]*lang "(?P<language>[\w ]+)"[\s]*$"#).unwrap());
static ROW_COUNT_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r#"^[\s]*(?P<rows>[0-9]+)[\s]*$"#).unwrap());

pub struct TranslationFile {
    file: String,
}

#[derive(Debug)]
enum State {
    Description,
    Stats,
    Lang,
    RowCount,
    Rows,
}

impl TranslationFile {
    pub fn new(file: String) -> Self {
        let file = file.trim_start_matches('\u{feff}').to_string();
        Self { file }
    }

    pub fn parse(&self) -> HashMap<&str, BTreeMap<StatKey, Vec<TranslationRow>>> {
        let mut state = State::Description;
        let mut lang = "English";
        let mut row_count = 0;
        let mut stats_ids = StatKey::Single(String::new());
        let mut map: HashMap<&str, BTreeMap<StatKey, Vec<TranslationRow>>> = HashMap::new();
        for line in self.file.lines() {
            if line.trim().is_empty() {
                continue;
            }
            match state {
                State::Description => {
                    if let Some(cap) = DESCRIPTION_REGEX.captures(line) {
                        if cap.name("description").is_some() {
                            state = State::Stats;
                        }
                    }
                }
                State::Stats => {
                    let stats = STATS_REGEX.captures(line).unwrap();
                    let new_stats_ids = stats
                        .name("stat_ids")
                        .unwrap()
                        .as_str()
                        .split(' ')
                        .map(|s| s.to_string())
                        .collect::<Vec<_>>();
                    if new_stats_ids.len() == 1 {
                        stats_ids = StatKey::Single(new_stats_ids[0].clone());
                    } else {
                        stats_ids = StatKey::Multiple(new_stats_ids);
                    }
                    state = State::Lang;
                    lang = "English";
                }
                State::Lang => {
                    if let Some(cap) = LANG_REGEX.captures(line) {
                        let new_lang = cap.name("language").unwrap().as_str();
                        lang = new_lang;
                        state = State::RowCount;
                    } else if let Some(cap) = ROW_COUNT_REGEX.captures(line) {
                        row_count = cap.name("rows").unwrap().as_str().parse().unwrap();
                        state = State::Rows;
                    } else if let Some(cap) = DESCRIPTION_REGEX.captures(line) {
                        if cap.name("description").is_some() {
                            state = State::Stats;
                        }
                    }
                }
                State::RowCount => {
                    let cap = ROW_COUNT_REGEX.captures(line).unwrap();
                    row_count = cap.name("rows").unwrap().as_str().parse().unwrap();
                    state = State::Rows;
                }
                State::Rows => {
                    row_count -= 1;
                    let cap = ROW_REGEX.captures(line).unwrap();
                    let format_string = cap.name("description").unwrap().as_str().to_string();
                    let condition = cap.name("minmax").unwrap().as_str().to_string();
                    let modifiers = cap.name("quantifier").unwrap().as_str().to_string();
                    let row = TranslationRow {
                        condition,
                        format_string,
                        modifiers,
                    };
                    map.entry(lang)
                        .or_default()
                        .entry(stats_ids.clone())
                        .or_default()
                        .push(row);
                    if row_count == 0 {
                        state = State::Lang;
                    }
                }
            }
        }
        map
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum StatKey {
    Single(String),
    Multiple(Vec<String>),
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct TranslationRow {
    pub condition: String,
    pub format_string: String,
    pub modifiers: String,
}
