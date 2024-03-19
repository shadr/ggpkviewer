use std::collections::{BTreeSet, HashMap};

use once_cell::sync::Lazy;
use regex::{Regex, RegexBuilder};

static HEADER_REGEX: Lazy<Regex> = Lazy::new(|| {
    RegexBuilder::new(r#"^version (?P<version>[0-9]+)[\r\n]*(?P<abstract>abstract)?[\r\n]*extends "(?P<extends>[\w\.\/_]+)"[\r\n]*(?P<remainder>.*)$"#)
        .multi_line(true)
        .build()
        .unwrap()
});

static SECTIONS_REGEX: Lazy<Regex> = Lazy::new(|| {
    RegexBuilder::new(r#"^(?P<key>[\w]+)[\r\n]+^\{(?P<contents>[^}]*)^}"#)
        .multi_line(true)
        .build()
        .unwrap()
});

static KEY_VALUE_REGEX: Lazy<Regex> = Lazy::new(|| {
    RegexBuilder::new(r#"^[\s]*(?P<key>[\S]+)[\s]*=[\s]*(?P<value>"[^"]*"|[\S]+)[\s]*$"#)
        .multi_line(true)
        .build()
        .unwrap()
});

#[derive(Debug)]
pub struct ITFile {
    pub version: u8,
    pub aabstract: bool,
    pub extends: String,
    pub sections: HashMap<String, HashMap<String, ITValue>>,
}

impl ITFile {
    pub fn parse(file: String) -> Self {
        let file = file.trim_start_matches('\u{feff}');
        let header = HEADER_REGEX.captures(&file).unwrap();
        let version = header.name("version").unwrap().as_str().parse().unwrap();
        let aabstract = header.name("abstract").is_some();
        let extends = header.name("extends").unwrap().as_str().to_string();

        let mut sections = HashMap::new();
        for section in SECTIONS_REGEX.captures_iter(&file) {
            let section_key = section.name("key").unwrap().as_str().to_string();
            let mut section_map = HashMap::new();

            let content = section.name("contents").unwrap().as_str();
            for keyvalue in KEY_VALUE_REGEX.captures_iter(content) {
                let key = keyvalue.name("key").unwrap().as_str().to_string();
                let value = keyvalue
                    .name("value")
                    .unwrap()
                    .as_str()
                    .trim_matches('"')
                    .to_string();
                let value = if section_key == "Base" && key == "tag" {
                    ITValue::new_list(value)
                } else {
                    ITValue::new(value)
                };
                section_map.insert(key, value);
            }

            sections.insert(section_key, section_map);
        }

        Self {
            version,
            aabstract,
            extends,
            sections,
        }
    }

    /// Merges two ITFile's
    ///
    /// If value keys exists in both ITFile then the value from `self` will be used, unless the
    /// type of the value is ITValue::Set, in which case the values from `other` will be added to
    /// the set
    pub fn merge(mut self, other: Self) -> Self {
        for (section_key, section_map) in other.sections {
            let Some(self_section) = self.sections.get_mut(&section_key) else {
                self.sections.insert(section_key, section_map);
                continue;
            };
            for (key, value) in section_map {
                if let Some(existing_value) = self_section.get_mut(&key) {
                    match (existing_value, value) {
                        (ITValue::Set(self_set), ITValue::Set(other_set)) => {
                            self_set.extend(other_set);
                        }
                        _ => (),
                    }
                } else {
                    self_section.insert(key, value);
                }
            }
        }
        Self {
            version: self.version,
            aabstract: self.aabstract,
            extends: self.extends,
            sections: self.sections,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ITValue {
    Number(i32),
    Set(BTreeSet<ITValue>),
    String(String),
}

impl ITValue {
    fn new(string: String) -> Self {
        if let Ok(number) = string.parse() {
            Self::Number(number)
        } else {
            Self::String(string)
        }
    }

    fn new_list(string: String) -> Self {
        Self::Set(BTreeSet::from([Self::new(string)]))
    }

    /// Gets the value as a string
    ///
    /// # Panics:
    /// If the `self` is not a ITValue::String variant
    pub fn as_string(&self) -> String {
        match self {
            Self::String(s) => s.clone(),
            _ => panic!("Expected ITValue::String variant, got {:?}", self),
        }
    }

    /// Gets the value as an i32
    ///
    /// # Panics:
    /// If the `self` is not a ITValue::Number variant
    pub fn as_number(&self) -> i32 {
        match self {
            Self::Number(n) => *n,
            _ => panic!("Expected ITValue::Number variant, got {:?}", self),
        }
    }

    /// Gets the value as a set
    ///
    /// # Panics:
    /// If the `self` is not a ITValue::Set variant
    pub fn as_set(&self) -> BTreeSet<ITValue> {
        match self {
            Self::Set(s) => s.clone(),
            _ => panic!("Expected ITValue::Set variant, got {:?}", self),
        }
    }

    /// Gets the value as a set of specific type
    ///
    /// # Panics:
    /// If the `self` is not a ITValue::Set variant
    /// or if any element panics when casting using passed function
    pub fn as_set_with<T: Ord>(&self, f: impl Fn(&ITValue) -> T) -> BTreeSet<T> {
        self.as_set().iter().map(|x| f(&x)).collect()
    }
}
