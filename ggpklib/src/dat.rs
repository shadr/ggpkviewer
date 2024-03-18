use std::{
    io::{Cursor, Seek, SeekFrom},
    ops::Range,
};

use byteorder::{LittleEndian, ReadBytesExt};

use crate::dat_schema::{ColumnType, TableColumn};

type ReadFn = fn(&mut Cursor<&[u8]>, &[u8]) -> DatValue;

#[derive(Debug)]
pub struct DatFile {
    pub data: Vec<u8>,
    pub row_count: u32,
    pub boundary: usize,
    pub row_length: usize,
    pub fixed_data_range: Range<usize>,
    pub variable_data_range: Range<usize>,
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
            boundary,
            row_length,
            fixed_data_range,
            variable_data_range,
        }
    }

    pub fn fixed_data(&self) -> &[u8] {
        &self.data[self.fixed_data_range.clone()]
    }

    pub fn variable_data(&self) -> &[u8] {
        &self.data[self.variable_data_range.clone()]
    }

    pub fn nth_row(&self, n: usize) -> DatRow {
        let start = n * self.row_length;
        let end = start + self.row_length;
        DatRow {
            fixed_cursor: Cursor::new(&self.fixed_data()[start..end]),
            variable_data: self.variable_data(),
        }
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
    pub fn read_u32(&mut self) -> u32 {
        self.fixed_cursor.read_u32::<LittleEndian>().unwrap()
    }

    pub fn read_u64(&mut self) -> u64 {
        self.fixed_cursor.read_u64::<LittleEndian>().unwrap()
    }

    pub fn read_i32(&mut self) -> i32 {
        self.fixed_cursor.read_i32::<LittleEndian>().unwrap()
    }

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

    pub fn get_fn(column: &TableColumn) -> ReadFn {
        match column.ttype {
            ColumnType::Bool => read_bool,
            ColumnType::String => read_string,
            ColumnType::I32 => read_i32,
            ColumnType::F32 => todo!(),
            ColumnType::Array => todo!(),
            ColumnType::Row => read_key,
            ColumnType::ForeignRow => read_foreign_key,
            ColumnType::EnumRow => read_enum_row,
        }
    }

    pub fn read_scalar(&mut self, column: &TableColumn) -> DatValue {
        let f = Self::get_fn(column);
        f(&mut self.fixed_cursor, self.variable_data)
    }

    pub fn read_array(&mut self, column: &TableColumn) -> DatValue {
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

const fn wrap_usize(value: usize) -> Option<usize> {
    if value == 0xfefefefefefefefe {
        None
    } else {
        Some(value)
    }
}

#[derive(Debug)]
pub enum DatValue {
    Bool(bool),
    String(String),
    I32(i32),
    F32(f32),
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
    /// Panic:
    ///   - If the DatValue is not a DatValue::Bool variant
    pub fn as_bool(&self) -> bool {
        match self {
            Self::Bool(b) => *b,
            _ => panic!("DatValue is not a DatValue::Bool variant"),
        }
    }

    /// Gets the value as a string
    ///
    /// Panic:
    ///   - If the DatValue is not a DatValue::String variant
    pub fn as_string(&self) -> String {
        match self {
            Self::String(s) => s.clone(),
            _ => panic!("DatValue is not a DatValue::String variant"),
        }
    }

    /// Gets the value as an i32
    ///
    /// Panic:
    ///   - If the DatValue is not a DatValue::I32 variant
    pub fn as_i32(&self) -> i32 {
        match self {
            Self::I32(i) => *i,
            _ => panic!("DatValue is not a DatValue::I32 variant"),
        }
    }

    /// Gets the value as a enum row index
    ///
    /// Panic:
    ///   - If the DatValue is not a DatValue::EnumRow variant
    pub fn as_enum_row_index(&self) -> usize {
        match self {
            Self::EnumRow(i) => *i,
            _ => panic!("DatValue is not a DatValue::EnumRow variant"),
        }
    }

    /// Gets the value as a foreign row index
    ///
    /// Panic:
    ///   - If the DatValue is not a DatValue::ForeignRow variant
    pub fn as_foreign_row_index(&self) -> Option<usize> {
        match self {
            Self::ForeignRow { rid, .. } => *rid,
            _ => panic!("DatValue is not a DatValue::ForeignRow variant"),
        }
    }

    /// Gets the value as an row index
    ///
    /// Panic:
    ///   - If the DatValue is not a DatValue::Row variant
    pub fn as_row_index(&self) -> Option<usize> {
        match self {
            Self::Row(i) => *i,
            _ => panic!("DatValue is not a DatValue::Row variant"),
        }
    }
}
