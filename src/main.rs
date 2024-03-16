pub mod bundle;
pub mod bundle_index;
pub mod dat;
pub mod ggpk;
pub mod utils;

use anyhow::anyhow;
use byteorder::{LittleEndian, ReadBytesExt};
use dat::DatFile;
use ggpk::{Entry, EntryData};
use std::{
    collections::HashMap,
    fs::File,
    io::{self, BufRead, Cursor, Read, Seek, SeekFrom},
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
    paths: HashMap<String, u64>,
    file_map: HashMap<u64, usize>,
}

impl PoeFS {
    pub fn new<S: FileSource + 'static>(mut source: S) -> Self {
        let (bundle, file) = source.get_file("/Bundles2/_.index.bin").unwrap().unwrap();
        let mut c = Cursor::new(file);
        let uncompressed = bundle.data(&mut c).unwrap();
        let mut data = Cursor::new(uncompressed);
        let bundle_index = BundleIndex::parse(&mut data).unwrap();

        let mut paths = HashMap::new();
        for path_rep in &bundle_index.path_rep {
            let start = path_rep.payload_offset as usize;
            let end = start + path_rep.payload_size as usize;
            let payload = &bundle_index.path_rep_data[start..end];
            let mut c = Cursor::new(payload);
            for path in make_paths(&mut c).unwrap() {
                let hash = murmur2::murmur64a(path.as_bytes(), 0x1337b33f);
                paths.insert(path, hash);
            }
        }

        let mut file_map = HashMap::new();
        for (index, file) in bundle_index.files.iter().enumerate() {
            file_map.insert(file.hash, index);
        }

        Self {
            source: Box::new(source),
            bundle_index,
            paths,
            file_map,
        }
    }

    pub fn get_file(&mut self, path: &str) -> Result<Option<Vec<u8>>, anyhow::Error> {
        let Some(hash) = self.paths.get(path) else {
            return Err(anyhow!(io::Error::new(
                io::ErrorKind::NotFound,
                "path not found in index bundle",
            )));
        };
        let Some(index) = self.file_map.get(hash) else {
            return Err(anyhow!(io::Error::new(
                io::ErrorKind::NotFound,
                "path hash not found in file map",
            )));
        };
        let file_record = &self.bundle_index.files[*index];
        let bundle_record = &self.bundle_index.bundles[file_record.bundle_index as usize];
        let Some((bundle, bundle_data)) = self
            .source
            .get_file(&format!("/Bundles2/{}.bundle.bin", bundle_record.name))?
        else {
            return Err(anyhow!(io::Error::new(
                io::ErrorKind::NotFound,
                "bundle file not found",
            )));
        };
        let mut c = Cursor::new(bundle_data);
        let bundle_uncompressed = bundle.data(&mut c)?;
        let start = file_record.file_offset as usize;
        let end = start + file_record.file_size as usize;
        let file_data = &bundle_uncompressed[start..end];
        Ok(Some(file_data.to_vec()))
    }
}

fn main() -> Result<(), anyhow::Error> {
    let args = Args::parse();
    let mut fs = if let Some(path) = args.ggpk {
        PoeFS::new(LocalSource::new(path)?)
    } else if args.online {
        PoeFS::new(OnlineSource)
    } else {
        unreachable!()
    };
    let mods = fs.get_file("data/mods.dat64")?.unwrap();
    let mods_dat = DatFile::new(mods);

    let mut variable_cursor = Cursor::new(mods_dat.variable_data());
    for i in 0..20 {
        let row = mods_dat.nth_row(i);

        let mut c = Cursor::new(row);
        let string_offset = c.read_u32::<LittleEndian>()?;
        variable_cursor.seek(SeekFrom::Start(string_offset as u64))?;
        let mut buf = Vec::new();
        for wind in mods_dat.variable_data()[string_offset as usize..].windows(4) {
            if wind == &[0, 0, 0, 0] && buf.len() % 2 == 0 {
                break;
            }
            buf.push(wind[0]);
        }
        let vecu16: Vec<u16> = buf
            .chunks_exact(2)
            .map(|a| u16::from_ne_bytes([a[0], a[1]]))
            .collect();
        let sliceu16 = vecu16.as_slice();
        let string = String::from_utf16_lossy(sliceu16)
            .trim_end_matches("\0")
            .to_string();
        dbg!(string);
    }
    Ok(())
}

fn find_sequence(bytes: &[u8], find: &[u8], offset: usize) -> Option<usize> {
    bytes[offset..]
        .windows(find.len())
        .position(|wind| wind == find)
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
