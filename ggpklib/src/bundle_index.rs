use std::io::{self};

use byteorder::{LittleEndian, ReadBytesExt};

use crate::bundle::Bundle;

#[derive(Debug)]
pub struct BundleIndex {
    pub bundle_count: u32,
    pub bundles: Vec<BundleRecord>,
    pub files_count: u32,
    pub files: Vec<FileRecord>,
    pub path_rep_count: u32,
    pub path_rep: Vec<PathRep>,
    pub path_rep_bundle: Bundle,
    pub path_rep_data: Vec<u8>,
}

impl BundleIndex {
    pub fn parse(reader: &mut impl io::Read) -> Result<Self, io::Error> {
        let bundle_count = reader.read_u32::<LittleEndian>()?;
        let mut bundles = Vec::with_capacity(bundle_count as usize);
        for _ in 0..bundle_count {
            bundles.push(BundleRecord::parse(reader)?);
        }

        let files_count = reader.read_u32::<LittleEndian>()?;
        let mut files = Vec::with_capacity(files_count as usize);
        for _ in 0..files_count {
            files.push(FileRecord::parse(reader)?);
        }

        let path_rep_count = reader.read_u32::<LittleEndian>()?;
        let mut path_rep = Vec::with_capacity(path_rep_count as usize);
        for _ in 0..path_rep_count {
            path_rep.push(PathRep::parse(reader)?);
        }

        let path_rep_bundle = Bundle::parse(reader)?;
        let path_rep_data = path_rep_bundle.data(reader)?;

        Ok(Self {
            bundle_count,
            bundles,
            files_count,
            files,
            path_rep_count,
            path_rep,
            path_rep_bundle,
            path_rep_data,
        })
    }
}

#[derive(Debug)]
pub struct BundleRecord {
    pub name_length: u32,
    pub name: String,
    pub bundle_uncompressed_size: u32,
}

impl BundleRecord {
    pub fn parse(reader: &mut impl io::Read) -> Result<Self, io::Error> {
        let name_length = reader.read_u32::<LittleEndian>()?;

        let mut name_buf = vec![0u8; name_length as usize];
        reader.read_exact(&mut name_buf)?;
        let name = String::from_utf8_lossy(&name_buf).to_string();
        let bundle_uncompressed_size = reader.read_u32::<LittleEndian>()?;
        Ok(Self {
            name_length,
            name,
            bundle_uncompressed_size,
        })
    }
}

#[derive(Debug)]
pub struct FileRecord {
    pub hash: u64,
    pub bundle_index: u32,
    pub file_offset: u32,
    pub file_size: u32,
}

impl FileRecord {
    pub fn parse(reader: &mut impl io::Read) -> Result<Self, io::Error> {
        let hash = reader.read_u64::<LittleEndian>()?;
        let bundle_index = reader.read_u32::<LittleEndian>()?;
        let file_offset = reader.read_u32::<LittleEndian>()?;
        let file_size = reader.read_u32::<LittleEndian>()?;
        Ok(Self {
            hash,
            bundle_index,
            file_offset,
            file_size,
        })
    }
}

#[derive(Debug)]
pub struct PathRep {
    pub hash: u64,
    pub payload_offset: u32,
    pub payload_size: u32,
    pub payload_recursive_size: u32,
}

impl PathRep {
    pub fn parse(reader: &mut impl io::Read) -> Result<Self, io::Error> {
        let hash = reader.read_u64::<LittleEndian>()?;
        let payload_offset = reader.read_u32::<LittleEndian>()?;
        let payload_size = reader.read_u32::<LittleEndian>()?;
        let payload_recursive_size = reader.read_u32::<LittleEndian>()?;
        Ok(Self {
            hash,
            payload_offset,
            payload_size,
            payload_recursive_size,
        })
    }
}
