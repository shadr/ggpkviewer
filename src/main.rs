pub mod bundle;
pub mod bundle_index;
pub mod ggpk;
pub mod utils;

use byteorder::{LittleEndian, ReadBytesExt};
use ggpk::Entry;
use std::{
    collections::HashMap,
    fs::File,
    io::{self, BufRead, Cursor},
    path::{Path, PathBuf},
};
use utils::find_file;

use clap::Parser;

use crate::{bundle::Bundle, bundle_index::BundleIndex};

#[derive(Debug, Parser)]
struct Args {
    #[arg(short, long, group = "source")]
    ggpk: PathBuf,
    #[arg(short, long, group = "source")]
    online: bool,
}

fn main() -> Result<(), anyhow::Error> {
    let args = Args::parse();
    let mut file = File::open(&args.ggpk)?;
    let entry = Entry::parse(&mut file)?;
    let _file_entry = find_file(&entry, &mut file, Path::new("/Bundles2/_.index.bin")).unwrap();
    let bundle = Bundle::parse(&mut file).unwrap();
    let uncompressed = bundle.data(&mut file)?;
    let mut data = Cursor::new(uncompressed);
    let bundle_index = BundleIndex::parse(&mut data)?;
    let path_rep_bundle = Bundle::parse(&mut data)?;
    let path_rep_data = path_rep_bundle.data(&mut data)?;
    let mut file_map = HashMap::new();
    for file in &bundle_index.files {
        file_map.insert(file.hash, file);
    }
    for path_rep in bundle_index.path_rep {
        let start = path_rep.payload_offset as usize;
        let end = start + path_rep.payload_size as usize;
        let payload = &path_rep_data[start..end];
        let mut c = Cursor::new(payload);
        for path in make_paths(&mut c).unwrap() {
            let hash = murmur2::murmur64a(path.as_bytes(), 0x1337b33f);
            let file_record = file_map[&hash];
            let bundle_record = &bundle_index.bundles[file_record.bundle_index as usize];
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
