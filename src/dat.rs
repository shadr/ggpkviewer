use std::{io::Cursor, ops::Range};

use byteorder::{LittleEndian, ReadBytesExt};

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

    pub fn nth_row<'a>(&'a self, n: usize) -> DatRow<'a> {
        let start = n * self.row_length;
        let end = start + self.row_length;
        DatRow {
            cursor: Cursor::new(&self.fixed_data()[start..end]),
        }
    }

    pub fn read_variable_string(&self, offset: usize) -> String {
        let data = &self.variable_data()[offset..];
        let windows = data.windows(4);
        let mut length = 0;
        for (index, wind) in windows.enumerate() {
            length = index;
            if wind == &[0, 0, 0, 0] && length % 2 == 0 {
                break;
            }
        }
        let sliceu16 = unsafe { data[..length].align_to::<u16>().1 };
        String::from_utf16_lossy(sliceu16).to_string()
    }
}

#[derive(Debug)]
pub struct DatRow<'a> {
    cursor: Cursor<&'a [u8]>,
}

impl<'a> AsRef<[u8]> for DatRow<'a> {
    fn as_ref(&self) -> &[u8] {
        self.cursor.get_ref()
    }
}

impl<'a> DatRow<'a> {
    pub fn read_u32(&mut self) -> u32 {
        self.cursor.read_u32::<LittleEndian>().unwrap()
    }
}
