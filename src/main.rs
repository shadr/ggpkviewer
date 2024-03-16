pub mod bundle;
pub mod bundle_index;
pub mod ggpk;
pub mod utils;

use byteorder::{LittleEndian, ReadBytesExt};
use ggpk::{Entry, EntryData};
use std::{
    collections::HashMap,
    fs::File,
    io::{self, BufRead, Cursor, Read, SeekFrom},
    path::{Path, PathBuf},
};

use clap::Parser;

use crate::{bundle::Bundle, bundle_index::BundleIndex};

#[derive(Debug, Parser)]
#[clap(group(clap::ArgGroup::new("source").required(true)))]
struct Args {
    #[arg(short, long, group = "source")]
    ggpk: Option<PathBuf>,
    #[arg(short, long, group = "source")]
    online: bool,
}

pub trait FileSource {
    fn get_file(&mut self, path: &str) -> Result<Option<(Bundle, Vec<u8>)>, anyhow::Error>;
}

struct LocalSource {
    file: File,
    ggpk_entry: Entry,
}

impl LocalSource {
    pub fn new(path: impl AsRef<Path>) -> Result<Self, io::Error> {
        let mut file = File::open(path)?;
        let entry = Entry::parse(&mut file)?;
        Ok(Self {
            file,
            ggpk_entry: entry,
        })
    }

    fn find_file_helper(
        entry: &Entry,
        reader: &mut (impl io::Read + io::Seek),
        mut path: &[&str],
    ) -> Option<Entry> {
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
                    reader.seek(SeekFrom::Start(entry.offset)).unwrap();
                    let entry = Entry::parse(reader).unwrap();
                    let found_file = Self::find_file_helper(&entry, reader, path);
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
            EntryData::Ggpk { entries, .. } => {
                reader.seek(SeekFrom::Start(entries[0].offset)).unwrap();
                let entry = Entry::parse(reader).unwrap();
                let found_file = Self::find_file_helper(&entry, reader, path);
                if found_file.is_some() {
                    return found_file;
                }

                reader.seek(SeekFrom::Start(entries[1].offset)).unwrap();
                let entry = Entry::parse(reader).unwrap();
                Self::find_file_helper(&entry, reader, path)
            }
        }
    }
}

impl FileSource for LocalSource {
    fn get_file(&mut self, path: &str) -> Result<Option<(Bundle, Vec<u8>)>, anyhow::Error> {
        let vec = path.split('/').collect::<Vec<_>>();
        let _file_entry = Self::find_file_helper(&self.ggpk_entry, &mut self.file, &vec).unwrap();
        let bundle = Bundle::parse(&mut self.file)?;
        let size = bundle.total_payload_size;
        let mut buf = vec![0u8; size as usize];
        self.file.read_exact(&mut buf)?;
        Ok(Some((bundle, buf)))
    }
}

struct OnlineSource;

impl FileSource for OnlineSource {
    fn get_file(&mut self, path: &str) -> Result<Option<(Bundle, Vec<u8>)>, anyhow::Error> {
        todo!()
    }
}

struct PoeFS {
    source: Box<dyn FileSource>,
    bundle_index: BundleIndex,
}

impl PoeFS {
    pub fn new<S: FileSource + 'static>(mut source: S) -> Self {
        let (bundle, file) = source.get_file("/Bundles2/_.index.bin").unwrap().unwrap();
        let mut c = Cursor::new(file);
        let uncompressed = bundle.data(&mut c).unwrap();
        let mut data = Cursor::new(uncompressed);
        let bundle_index = BundleIndex::parse(&mut data).unwrap();
        Self {
            source: Box::new(source),
            bundle_index,
        }
    }
}

fn main() -> Result<(), anyhow::Error> {
    let args = Args::parse();
    let fs = if let Some(path) = args.ggpk {
        PoeFS::new(LocalSource::new(path)?)
    } else if args.online {
        PoeFS::new(OnlineSource)
    } else {
        unreachable!()
    };
    let mut file_map = HashMap::new();
    for file in &fs.bundle_index.files {
        file_map.insert(file.hash, file);
    }
    for path_rep in fs.bundle_index.path_rep {
        let start = path_rep.payload_offset as usize;
        let end = start + path_rep.payload_size as usize;
        let payload = &fs.bundle_index.path_rep_data[start..end];
        let mut c = Cursor::new(payload);
        for path in make_paths(&mut c).unwrap() {
            let hash = murmur2::murmur64a(path.as_bytes(), 0x1337b33f);
            let file_record = file_map[&hash];
            let bundle_record = &fs.bundle_index.bundles[file_record.bundle_index as usize];
            println!(
                "{} {}: {}",
                bundle_record.name, bundle_record.bundle_uncompressed_size, path
            );
        }
    }
    Ok(())
}

fn make_paths(reader: &mut Cursor<&[u8]>) -> Result<Vec<String>, io::Error> {
    let mut temp: Vec<String> = Vec::new();
    let mut paths = Vec::new();
    let mut base = false;
    let mut buf = Vec::new();
    while (reader.position() as usize) < reader.get_ref().len() - 4 {
        let mut index = reader.read_u32::<LittleEndian>()?;
        if index == 0 {
            base = !base;
            if base {
                temp.clear();
            }
            continue;
        } else {
            index -= 1;
        }

        buf.clear();
        reader.read_until(0, &mut buf)?;
        let raw = String::from_utf8(buf.clone()).unwrap();

        let string = raw.trim_end_matches('\0');

        let string = if let Some(prefix) = temp.get(index as usize) {
            prefix.clone() + &string
        } else {
            string.to_string()
        };

        if base {
            temp.push(string);
        } else {
            paths.push(string);
        }
    }
    Ok(paths)
}
