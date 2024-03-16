use std::ops::Range;

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

    pub fn nth_row(&self, n: usize) -> &[u8] {
        let start = n * self.row_length;
        let end = start + self.row_length;
        &self.fixed_data()[start..end]
    }
}
