use std::path::Path;

#[derive(Debug, serde::Deserialize)]
pub struct SchemaFile {
    pub version: u32,
    #[serde(rename = "createdAt")]
    pub created_at: u32,
    pub tables: Vec<SchemaTable>,
    pub enumerations: Vec<SchemaEnumeration>,
}

impl SchemaFile {
    /// Reads the content of the file and deserializes it
    pub fn read_from_file(path: impl AsRef<Path>) -> Result<Self, anyhow::Error> {
        let content = std::fs::read_to_string(path)?;
        Ok(serde_json::from_str(&content)?)
    }

    pub fn read_from_str(content: &str) -> Result<Self, anyhow::Error> {
        Ok(serde_json::from_str(content)?)
    }

    pub fn read_from_online() -> Result<Self, anyhow::Error> {
        let response = reqwest::blocking::get(
            "https://github.com/poe-tool-dev/dat-schema/releases/download/latest/schema.min.json",
        )?;
        let text = response.text()?;
        Self::read_from_str(&text)
    }

    pub fn find_table(&self, table_name: &str) -> Option<&SchemaTable> {
        self.tables
            .iter()
            .find(|t| t.name.to_lowercase() == table_name)
    }
}

#[derive(Debug, serde::Deserialize)]
pub struct SchemaTable {
    pub name: String,
    pub columns: Vec<TableColumn>,
    pub tags: Vec<String>,
}

#[derive(Debug, serde::Deserialize)]
pub struct TableColumn {
    pub name: Option<String>,
    pub description: Option<String>,
    pub array: bool,
    #[serde(rename = "type")]
    pub ttype: ColumnType,
    pub unique: bool,
    pub localized: bool,
    pub until: Option<String>,
    pub references: Option<Reference>,
    pub file: Option<String>,
    pub files: Option<Vec<String>>,
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ColumnType {
    Bool,
    String,
    I32,
    F32,
    Array,
    Row,
    ForeignRow,
    EnumRow,
}

#[derive(Debug, serde::Deserialize)]
#[serde(untagged)]
pub enum Reference {
    RefUsingRowIndex { table: String },
    RefUsingColumn { table: String, column: String },
}

#[derive(Debug, serde::Deserialize)]
pub struct SchemaEnumeration {
    pub name: String,
    pub indexing: u8,
    pub enumerators: Vec<Option<String>>,
}
