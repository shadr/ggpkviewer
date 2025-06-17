use std::{
    collections::HashMap,
    io::{Cursor, Seek, SeekFrom},
    ops::Range,
};

use byteorder::{LittleEndian, ReadBytesExt};

use crate::dat_schema::{ColumnType, TableColumn};

type ReadFn = fn(&mut Cursor<&[u8]>, &[u8]) -> DatValue;

#[derive(Debug)]
pub struct DatFile {
    data: Vec<u8>,
    row_count: u32,
    row_length: usize,
    fixed_data_range: Range<usize>,
    variable_data_range: Range<usize>,
}

impl DatFile {
    pub fn new(data: Vec<u8>) -> Self {
        let row_count = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
        let boundary = data
            .windows(8)
            .position(|wind| wind.iter().all(|b| *b == 0xBB))
            .unwrap();
        let row_length = ((boundary as u32 - 4) / row_count) as usize;

        let fixed_data_range = 4..boundary;
        let variable_data_range = boundary..data.len();

        Self {
            data,
            row_count,
            row_length,
            fixed_data_range,
            variable_data_range,
        }
    }

    /// Returns the row length in bytes
    pub fn row_length(&self) -> usize {
        self.row_length
    }

    /// Returns the number of rows
    pub fn row_count(&self) -> u32 {
        self.row_count
    }

    /// Returns byte slice of data where fixed length data is located, length of the slice is equal
    /// to the row length in bytes * the number of rows
    pub fn fixed_data(&self) -> &[u8] {
        &self.data[self.fixed_data_range.clone()]
    }

    /// Returns byte slice of data where variable length data is located
    pub fn variable_data(&self) -> &[u8] {
        &self.data[self.variable_data_range.clone()]
    }

    /// Returns the nth row
    pub fn nth_row(&self, n: usize) -> DatRow {
        let start = n * self.row_length;
        let end = start + self.row_length;
        DatRow {
            fixed_cursor: Cursor::new(&self.fixed_data()[start..end]),
            variable_data: self.variable_data(),
        }
    }

    /// Returns an iterator over the rows
    pub fn iter_rows(&self) -> impl Iterator<Item = DatRow> {
        (0..self.row_count as usize).map(move |n| self.nth_row(n))
    }

    /// Returns an iterator over the rows, reading rows with schema to Vec
    pub fn iter_rows_vec<'a>(
        &'a self,
        columns: &'a [TableColumn],
    ) -> impl Iterator<Item = Vec<DatValue>> + 'a {
        self.iter_rows()
            .map(|mut row| row.read_with_schema(columns))
    }

    /// Returns an iterator over the rows, reading rows with schema to HashMap
    pub fn iter_rows_map<'a>(
        &'a self,
        columns: &'a [TableColumn],
    ) -> impl Iterator<Item = HashMap<String, DatValue>> + 'a {
        self.iter_rows().map(|mut row| row.read_to_map(columns))
    }
}

pub fn read_variable_string(data: &[u8], offset: usize) -> String {
    let data = &data[offset..];
    let length = data
        .windows(4)
        .enumerate()
        .position(|(index, wind)| wind == [0, 0, 0, 0] && index % 2 == 0)
        .unwrap();
    let vecu16: Vec<u16> = data[..length]
        .chunks_exact(2)
        .map(|a| u16::from_ne_bytes([a[0], a[1]]))
        .collect();
    String::from_utf16_lossy(&vecu16)
}

#[derive(Debug)]
pub struct DatRow<'a> {
    fixed_cursor: Cursor<&'a [u8]>,
    variable_data: &'a [u8],
}

impl<'a> AsRef<[u8]> for DatRow<'a> {
    fn as_ref(&self) -> &[u8] {
        self.fixed_cursor.get_ref()
    }
}

impl<'a> DatRow<'a> {
    /// Parse a row using provided column definitions and return a Vec of parsed values
    pub fn read_with_schema(&mut self, columns: &[TableColumn]) -> Vec<DatValue> {
        let mut values = Vec::new();
        for column in columns {
            let value = if column.array {
                self.read_array(column)
            } else {
                self.read_scalar(column)
            };
            values.push(value);
        }
        values
    }

    /// Parse a row using provided column definitions and return a HashMap where keys are column names
    pub fn read_to_map(&mut self, columns: &[TableColumn]) -> HashMap<String, DatValue> {
        let mut unknown_column_count = 0;
        let values = self.read_with_schema(columns);
        let mut map = HashMap::new();
        for (value, column) in values.into_iter().zip(columns.iter()) {
            let column_name = column.name.clone().unwrap_or_else(|| {
                let s = format!("Unknown{unknown_column_count}");
                unknown_column_count += 1;
                s
            });
            map.insert(column_name, value);
        }
        map
    }

    fn get_fn(column: &TableColumn) -> ReadFn {
        match column.ttype {
            ColumnType::Bool => read_bool,
            ColumnType::String => read_string,
            ColumnType::I16 => read_i16,
            ColumnType::I32 => read_i32,
            ColumnType::U16 => read_u16,
            ColumnType::U32 => read_u32,
            ColumnType::F32 => todo!(),
            ColumnType::Array => read_unknown_array,
            ColumnType::Row => read_key,
            ColumnType::ForeignRow => read_foreign_key,
            ColumnType::EnumRow => read_enum_row,
        }
    }

    fn read_scalar(&mut self, column: &TableColumn) -> DatValue {
        let f = Self::get_fn(column);
        f(&mut self.fixed_cursor, self.variable_data)
    }

    fn read_array(&mut self, column: &TableColumn) -> DatValue {
        let f = Self::get_fn(column);
        let array_length = self.fixed_cursor.read_u64::<LittleEndian>().unwrap();
        let mut arr = Vec::new();
        let variable_offset = self.fixed_cursor.read_u64::<LittleEndian>().unwrap();
        let mut variable_reader = Cursor::new(self.variable_data);
        variable_reader
            .seek(SeekFrom::Start(variable_offset))
            .unwrap();
        for _ in 0..array_length {
            arr.push(f(&mut variable_reader, self.variable_data))
        }
        DatValue::Array(arr)
    }
}

fn read_string(fixed_reader: &mut Cursor<&[u8]>, variable_data: &[u8]) -> DatValue {
    let string_offset = fixed_reader.read_u64::<LittleEndian>().unwrap();
    let string = read_variable_string(variable_data, string_offset as usize);
    DatValue::String(string)
}

fn read_i32(fixed_reader: &mut Cursor<&[u8]>, _: &[u8]) -> DatValue {
    let value = fixed_reader.read_i32::<LittleEndian>().unwrap();
    DatValue::I32(value)
}

fn read_u32(fixed_reader: &mut Cursor<&[u8]>, _: &[u8]) -> DatValue {
    let value = fixed_reader.read_u32::<LittleEndian>().unwrap();
    DatValue::U32(value)
}

fn read_i16(fixed_reader: &mut Cursor<&[u8]>, _: &[u8]) -> DatValue {
    let value = fixed_reader.read_i16::<LittleEndian>().unwrap();
    DatValue::I16(value)
}

fn read_u16(fixed_reader: &mut Cursor<&[u8]>, _: &[u8]) -> DatValue {
    let value = fixed_reader.read_u16::<LittleEndian>().unwrap();
    DatValue::U16(value)
}

fn read_foreign_key(fixed_reader: &mut Cursor<&[u8]>, _: &[u8]) -> DatValue {
    let rid = wrap_usize(fixed_reader.read_u64::<LittleEndian>().unwrap() as usize);
    let unknown = wrap_usize(fixed_reader.read_u64::<LittleEndian>().unwrap() as usize);
    DatValue::ForeignRow { rid, unknown }
}

fn read_enum_row(fixed_reader: &mut Cursor<&[u8]>, _: &[u8]) -> DatValue {
    let row = fixed_reader.read_i32::<LittleEndian>().unwrap();
    DatValue::EnumRow(row as usize)
}

fn read_bool(fixed_reader: &mut Cursor<&[u8]>, _: &[u8]) -> DatValue {
    let value = fixed_reader.read_u8().unwrap();
    DatValue::Bool(value > 0)
}

fn read_key(fixed_reader: &mut Cursor<&[u8]>, _: &[u8]) -> DatValue {
    let row = wrap_usize(fixed_reader.read_u64::<LittleEndian>().unwrap() as usize);
    DatValue::Row(row)
}

fn read_unknown_array(fixed_reader: &mut Cursor<&[u8]>, _: &[u8]) -> DatValue {
    let array_length = fixed_reader.read_u64::<LittleEndian>().unwrap();
    let variable_offset = fixed_reader.read_u64::<LittleEndian>().unwrap();
    DatValue::UnknownArray(variable_offset, array_length)
}

const fn wrap_usize(value: usize) -> Option<usize> {
    if value == 0xfefefefefefefefe {
        None
    } else {
        Some(value)
    }
}

#[derive(Debug, Clone)]
pub enum DatValue {
    Bool(bool),
    String(String),
    I16(i16),
    I32(i32),
    U16(u16),
    U32(u32),
    F32(f32),
    UnknownArray(u64, u64),
    Array(Vec<DatValue>),
    Row(Option<usize>),
    ForeignRow {
        rid: Option<usize>,
        unknown: Option<usize>,
    },
    EnumRow(usize),
}

impl DatValue {
    /// Gets the value as a bool
    ///
    /// # Panics:
    /// If the DatValue is not a DatValue::Bool variant
    pub fn as_bool(&self) -> bool {
        match self {
            Self::Bool(b) => *b,
            _ => panic!("Expected DatValue::Bool variant, got {:?}", self),
        }
    }

    /// Gets the value as a string
    ///
    /// # Panics:
    /// If the DatValue is not a DatValue::String variant
    pub fn as_string(&self) -> String {
        match self {
            Self::String(s) => s.clone(),
            _ => panic!("Expected DatValue::String variant, got {:?}", self),
        }
    }

    /// Gets the value as an i32
    ///
    /// # Panics:
    /// If the DatValue is not a DatValue::I32 variant
    pub fn as_i32(&self) -> i32 {
        match self {
            Self::I32(i) => *i,
            _ => panic!("Expected DatValue::I32 variant, got {:?}", self),
        }
    }

    /// Gets the value as a enum row index
    ///
    /// # Panics:
    /// If the DatValue is not a DatValue::EnumRow variant
    pub fn as_enum_row_index(&self) -> usize {
        match self {
            Self::EnumRow(i) => *i,
            _ => panic!("Expected DatValue::EnumRow variant, got {:?}", self),
        }
    }

    /// Gets the value as a foreign row index
    ///
    /// # Panics:
    /// If the DatValue is not a DatValue::ForeignRow variant
    pub fn as_foreign_row_index(&self) -> Option<usize> {
        match self {
            Self::ForeignRow { rid, .. } => *rid,
            _ => panic!("Expected DatValue::ForeignRow variant, got {:?}", self),
        }
    }

    /// Gets the value as an row index
    ///
    /// # Panics:
    /// If the DatValue is not a DatValue::Row variant
    pub fn as_row_index(&self) -> Option<usize> {
        match self {
            Self::Row(i) => *i,
            _ => panic!("Expected DatValue::Row variant, got {:?}", self),
        }
    }

    /// Gets the value as an array
    ///
    /// # Panics:
    /// If the DatValue is not a DatValue::Array variant
    pub fn as_array(&self) -> Vec<DatValue> {
        match self {
            Self::Array(a) => a.clone(),
            _ => panic!("Expected DatValue::Array variant, got {:?}", self),
        }
    }

    /// Gets the value as an array of specific type
    ///
    /// # Usage:
    /// ```
    /// let i32_array = datvalue.as_array_with(DatValue::as_i32);
    ///
    /// ```
    ///
    /// # Panics:
    /// If the DatValue is not a DatValue::Array variant
    /// or if any element panics when casting using passed function
    pub fn as_array_with<T>(&self, f: impl Fn(&Self) -> T) -> Vec<T> {
        match self {
            Self::Array(a) => a.iter().map(f).collect(),
            _ => panic!("Expected DatValue::Array variant, got {:?}", self),
        }
    }
}
