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
