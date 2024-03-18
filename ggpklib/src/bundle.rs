use std::io::{self};

use byteorder::{LittleEndian, ReadBytesExt};

#[derive(Debug, Default)]
pub struct Bundle {
    pub uncompressed_size: u32,
    pub total_payload_size: u32,
    pub head_payload_size: u32,
    pub head_payload: HeadPayload,
}

impl Bundle {
    pub fn parse(reader: &mut impl io::Read) -> Result<Self, io::Error> {
        let uncompressed_size = reader.read_u32::<LittleEndian>()?;
        let total_payload_size = reader.read_u32::<LittleEndian>()?;
        let head_payload_size = reader.read_u32::<LittleEndian>()?;
        let head_payload = HeadPayload::parse(reader)?;
        Ok(Self {
            uncompressed_size,
            total_payload_size,
            head_payload_size,
            head_payload,
        })
    }

    pub fn data(&self, reader: &mut impl io::Read) -> Result<Vec<u8>, io::Error> {
        let mut data_input = vec![0u8; self.head_payload.total_payload_size as usize];
        reader.read_exact(&mut data_input)?;
        let mut data = Vec::new();
        let mut offset = 0;
        for block_size in &self.head_payload.block_sizes {
            data.push(&data_input[offset..offset + *block_size as usize]);
            offset += *block_size as usize;
        }
        let mut uncompressed = Vec::with_capacity(self.uncompressed_size as usize);
        for (index, block) in data.iter().enumerate() {
            let size = if index != data.len() - 1 {
                self.head_payload.uncompressed_block_granularity as usize
            } else {
                (self.head_payload.uncompressed_size
                    % self.head_payload.uncompressed_block_granularity as u64)
                    as usize
            };
            let mut data_output = vec![0u8; size];
            unsafe { oozle::decompress(block, &mut data_output) }.unwrap();
            uncompressed.extend_from_slice(&data_output)
        }
        Ok(uncompressed)
    }
}

#[derive(Debug, Default)]
pub struct HeadPayload {
    pub first_file_encode: u32,
    pub unk10: u32,
    pub uncompressed_size: u64,
    pub total_payload_size: u64,
    pub block_count: u32,
    pub uncompressed_block_granularity: u32,
    pub unk28: [u32; 4],
    pub block_sizes: Vec<u32>,
}

impl HeadPayload {
    pub fn parse(reader: &mut impl io::Read) -> Result<Self, io::Error> {
        let first_file_encode = reader.read_u32::<LittleEndian>()?;
        let unk10 = reader.read_u32::<LittleEndian>()?;
        let uncompressed_size = reader.read_u64::<LittleEndian>()?;
        let total_payload_size = reader.read_u64::<LittleEndian>()?;
        let block_count = reader.read_u32::<LittleEndian>()?;
        let uncompressed_block_granularity = reader.read_u32::<LittleEndian>()?;
        let unk28 = [
            reader.read_u32::<LittleEndian>()?,
            reader.read_u32::<LittleEndian>()?,
            reader.read_u32::<LittleEndian>()?,
            reader.read_u32::<LittleEndian>()?,
        ];
        let mut block_sizes = Vec::with_capacity(block_count as usize);
        for _ in 0..block_count {
            block_sizes.push(reader.read_u32::<LittleEndian>()?);
        }
        Ok(Self {
            first_file_encode,
            unk10,
            uncompressed_size,
            total_payload_size,
            block_count,
            uncompressed_block_granularity,
            unk28,
            block_sizes,
        })
    }
}
