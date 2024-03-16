use std::io;

use byteorder::{LittleEndian, ReadBytesExt};

#[derive(Debug, Clone)]
pub struct GgpkEntry {
    pub offset: u64,
}

impl GgpkEntry {
    pub fn parse(reader: &mut impl io::Read) -> Result<Self, io::Error> {
        let offset = reader.read_u64::<LittleEndian>()?;
        Ok(Self { offset })
    }
}

#[derive(Debug, Clone)]
pub struct Entry {
    pub length: u32,
    pub tag: EntryTag,
    pub data: EntryData,
}

impl Entry {
    pub fn parse(reader: &mut impl io::Read) -> Result<Self, io::Error> {
        let length = reader.read_u32::<LittleEndian>()?;
        let tag = EntryTag::parse(reader)?;
        let data = EntryData::parse(reader, tag)?;
        Ok(Self { length, tag, data })
    }

    pub fn data_length_left(&self) -> u32 {
        match &self.data {
            EntryData::File { name_length, .. } => {
                let mut left = self.length;
                left -= 4; // length field itself
                left -= 4; // tag field
                left -= 4; // name_length field
                left -= 32; // sha256hash
                left -= name_length * 2; // name
                left
            }
            _ => 0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryTag {
    Ggpk,
    Free,
    Pdir,
    File,
}

impl EntryTag {
    pub fn parse(reader: &mut impl io::Read) -> Result<Self, io::Error> {
        let mut tag = [0; 4];
        reader.read_exact(&mut tag)?;
        if tag == "GGPK".as_bytes() {
            return Ok(Self::Ggpk);
        }
        if tag == "FREE".as_bytes() {
            return Ok(Self::Free);
        }
        if tag == "PDIR".as_bytes() {
            return Ok(Self::Pdir);
        }
        if tag == "FILE".as_bytes() {
            return Ok(Self::File);
        }
        unimplemented!("Unknown entry tag: {:?}", String::from_utf8(tag.to_vec()));
    }
}

#[derive(Debug, Clone)]
pub enum EntryData {
    Free,
    Pdir {
        name_length: u32,
        total_entries: u32,
        sha256hash: [u8; 32],
        name: String,
        entries: Vec<DirectoryEntry>,
    },
    File {
        name_length: u32,
        sha256hash: [u8; 32],
        name: String,
    },
    Ggpk {
        version: u32,
        entries: [GgpkEntry; 2],
    },
}

impl EntryData {
    pub fn parse(reader: &mut impl io::Read, tag: EntryTag) -> Result<Self, io::Error> {
        Ok(match tag {
            EntryTag::Free => Self::Free,
            EntryTag::Pdir => {
                let name_length = reader.read_u32::<LittleEndian>()?;
                let total_entries = reader.read_u32::<LittleEndian>()?;
                let mut sha256hash = [0; 32];
                reader.read_exact(&mut sha256hash)?;

                let mut name_buf = vec![0u8; (name_length * 2) as usize];
                reader.read_exact(&mut name_buf)?;
                let vecu16: Vec<u16> = name_buf
                    .chunks_exact(2)
                    .map(|a| u16::from_ne_bytes([a[0], a[1]]))
                    .collect();
                let sliceu16 = vecu16.as_slice();
                let name = String::from_utf16_lossy(sliceu16)
                    .trim_end_matches("\0")
                    .to_string();

                let mut entries = Vec::with_capacity(total_entries as usize);
                for _ in 0..total_entries {
                    entries.push(DirectoryEntry::parse(reader)?);
                }
                Self::Pdir {
                    name_length,
                    total_entries,
                    sha256hash,
                    name,
                    entries,
                }
            }
            EntryTag::File => {
                let name_length = reader.read_u32::<LittleEndian>()?;
                let mut sha256hash = [0; 32];
                reader.read_exact(&mut sha256hash)?;

                let mut name_buf = vec![0u8; (name_length * 2) as usize];
                reader.read_exact(&mut name_buf)?;
                let vecu16: Vec<u16> = name_buf
                    .chunks_exact(2)
                    .map(|a| u16::from_le_bytes([a[0], a[1]]))
                    .collect();
                let sliceu16 = vecu16.as_slice();
                let name = String::from_utf16_lossy(sliceu16)
                    .trim_end_matches("\0")
                    .to_string();
                Self::File {
                    name_length,
                    sha256hash,
                    name,
                }
            }
            EntryTag::Ggpk => {
                let version = reader.read_u32::<LittleEndian>()?;
                let entries = [GgpkEntry::parse(reader)?, GgpkEntry::parse(reader)?];
                Self::Ggpk { version, entries }
            }
        })
    }
}

#[derive(Debug, Clone)]
pub struct DirectoryEntry {
    pub entry_name_hash: i32,
    pub offset: u64,
}

impl DirectoryEntry {
    pub fn parse(reader: &mut impl io::Read) -> Result<Self, io::Error> {
        let entry_name_hash = reader.read_i32::<LittleEndian>()?;
        let offset = reader.read_u64::<LittleEndian>()?;
        Ok(Self {
            entry_name_hash,
            offset,
        })
    }
}
