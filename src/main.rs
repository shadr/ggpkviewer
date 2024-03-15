pub mod bundle;
pub mod bundle_index;

use byteorder::{LittleEndian, ReadBytesExt};
use std::{
    fs::File,
    io::{self, Cursor, Read, Seek, SeekFrom},
    path::{Component, Path, PathBuf},
};

use clap::Parser;

use crate::{bundle::Bundle, bundle_index::BundleIndex};

#[derive(Debug, Parser)]
struct Args {
    #[arg(short, long)]
    ggpk: PathBuf,
}

#[derive(Debug, Clone)]
pub struct GgpkEntry {
    pub offset: u64,
}

impl GgpkEntry {
    pub fn parse(file: &mut File) -> Result<Self, io::Error> {
        let offset = file.read_u64::<LittleEndian>()?;
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
    pub fn parse(file: &mut File) -> Result<Self, io::Error> {
        let length = file.read_u32::<LittleEndian>()?;
        let tag = EntryTag::parse(file)?;
        let data = EntryData::parse(file, tag)?;
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
    pub fn parse(file: &mut File) -> Result<Self, io::Error> {
        let mut tag = [0; 4];
        file.read_exact(&mut tag)?;
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
    pub fn parse(file: &mut File, tag: EntryTag) -> Result<Self, io::Error> {
        Ok(match tag {
            EntryTag::Free => Self::Free,
            EntryTag::Pdir => {
                let name_length = file.read_u32::<LittleEndian>()?;
                let total_entries = file.read_u32::<LittleEndian>()?;
                let mut sha256hash = [0; 32];
                file.read_exact(&mut sha256hash)?;

                let mut name_buf = vec![0u8; (name_length * 2) as usize];
                file.read_exact(&mut name_buf)?;
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
                    entries.push(DirectoryEntry::parse(file)?);
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
                let name_length = file.read_u32::<LittleEndian>()?;
                let mut sha256hash = [0; 32];
                file.read_exact(&mut sha256hash)?;

                let mut name_buf = vec![0u8; (name_length * 2) as usize];
                file.read_exact(&mut name_buf)?;
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
                let version = file.read_u32::<LittleEndian>()?;
                let entries = [GgpkEntry::parse(file)?, GgpkEntry::parse(file)?];
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
    pub fn parse(file: &mut File) -> Result<Self, io::Error> {
        let entry_name_hash = file.read_i32::<LittleEndian>()?;
        let offset = file.read_u64::<LittleEndian>()?;
        Ok(Self {
            entry_name_hash,
            offset,
        })
    }
}

pub fn print_tree(entry: &Entry, file: &mut File, indentation: u32) -> Result<(), io::Error> {
    const INDENT_STR: &str = "│ ";
    let indent = indentation.saturating_sub(1);
    let indent_string = INDENT_STR.repeat(indent as usize);
    print!("{}├─", indent_string);
    match &entry.data {
        EntryData::Free => println!("Free"),
        EntryData::Pdir { name, entries, .. } => {
            println!("{}", name);
            for entry in entries {
                file.seek(SeekFrom::Start(entry.offset))?;
                let entry = Entry::parse(file)?;
                print_tree(&entry, file, indentation + 1)?;
            }
        }
        EntryData::File { name, .. } => {
            println!("{} size: {}", name, entry.data_length_left());
        }
        EntryData::Ggpk { version, entries } => {
            println!("Ggpk version={}", version);

            file.seek(SeekFrom::Start(entries[0].offset))?;
            let entry = Entry::parse(file)?;
            print_tree(&entry, file, indentation + 1)?;

            file.seek(SeekFrom::Start(entries[1].offset))?;
            let entry = Entry::parse(file)?;
            print_tree(&entry, file, indentation + 1)?;
        }
    }
    Ok(())
}

fn find_file_helper(entry: &Entry, file: &mut File, mut path: &[&str]) -> Option<Entry> {
    if path.is_empty() {
        return None;
    }

    match &entry.data {
        EntryData::Free => None,
        EntryData::Pdir { name, entries, .. } => {
            if name != path[0] {
                return None;
            }
            path = &path[1..];
            for entry in entries {
                file.seek(SeekFrom::Start(entry.offset)).unwrap();
                let entry = Entry::parse(file).unwrap();
                let found_file = find_file_helper(&entry, file, path);
                if found_file.is_some() {
                    return found_file;
                }
            }
            None
        }
        EntryData::File { name, .. } => {
            if name == path[0] {
                Some(entry.clone())
            } else {
                None
            }
        }
        EntryData::Ggpk { version, entries } => {
            file.seek(SeekFrom::Start(entries[0].offset)).unwrap();
            let entry = Entry::parse(file).unwrap();
            let found_file = find_file_helper(&entry, file, path);
            if found_file.is_some() {
                return found_file;
            }

            file.seek(SeekFrom::Start(entries[1].offset)).unwrap();
            let entry = Entry::parse(file).unwrap();
            find_file_helper(&entry, file, path)
        }
    }
}

/// Find file in ggpk entry, file cursor will be set at the start of file data if file is found
pub fn find_file(entry: &Entry, file: &mut File, path: &Path) -> Option<Entry> {
    let vec = path
        .components()
        .map(|c| match c {
            Component::Prefix(_) => todo!(),
            Component::RootDir => "",
            Component::CurDir => todo!(),
            Component::ParentDir => todo!(),
            Component::Normal(s) => s.to_str().unwrap(),
        })
        .collect::<Vec<_>>();
    find_file_helper(entry, file, &vec)
}

fn main() -> Result<(), anyhow::Error> {
    let args = Args::parse();
    let mut file = File::open(&args.ggpk)?;
    let entry = Entry::parse(&mut file)?;
    let _file_entry = find_file(&entry, &mut file, Path::new("/Bundles2/_.index.bin")).unwrap();
    let bundle = Bundle::parse(&mut file).unwrap();
    dbg!(&bundle);
    let mut data_input = vec![0u8; bundle.head_payload.total_payload_size as usize];
    file.read_exact(&mut data_input)?;
    let mut data = Vec::new();
    let mut offset = 0;
    for block_size in &bundle.head_payload.block_sizes {
        data.push(&data_input[offset..offset + *block_size as usize]);
        offset += *block_size as usize;
    }
    let mut uncompressed = Vec::with_capacity(bundle.uncompressed_size as usize);
    for (index, block) in data.iter().enumerate() {
        let size = if index != data.len() - 1 {
            bundle.head_payload.uncompressed_block_granularity as usize
        } else {
            (bundle.head_payload.uncompressed_size
                % bundle.head_payload.uncompressed_block_granularity as u64) as usize
        };
        let mut data_output = vec![0u8; size];
        unsafe { oozle::decompress(block, &mut data_output) }.unwrap();
        uncompressed.extend_from_slice(&data_output)
    }
    let length = uncompressed.len();
    let mut data = Cursor::new(uncompressed);
    let bundle_index = BundleIndex::parse(&mut data)?;
    dbg!(bundle_index);
    dbg!(length - data.position() as usize);
    Ok(())
}
