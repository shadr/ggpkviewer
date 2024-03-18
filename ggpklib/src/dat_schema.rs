#[derive(Debug, serde::Deserialize)]
pub struct SchemaFile {
    pub version: u32,
    #[serde(rename = "createdAt")]
    pub created_at: u32,
    pub tables: Vec<SchemaTable>,
    pub enumerations: Vec<SchemaEnumeration>,
}

impl SchemaFile {
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
