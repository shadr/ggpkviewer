use std::{
    collections::HashMap,
    io::{self, BufRead, Cursor},
};

use anyhow::anyhow;
use byteorder::{LittleEndian, ReadBytesExt};

use crate::{bundle::Bundle, bundle_index::BundleIndex};

pub mod local;
pub mod online;

pub trait FileSource {
    fn get_file(&mut self, path: &str) -> Result<Option<(Bundle, Vec<u8>)>, anyhow::Error>;
}

pub struct PoeFS {
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

        let string = temp
            .get(index as usize)
            .map_or_else(|| string.to_string(), |prefix| prefix.clone() + string);

        if base {
            temp.push(string);
        } else {
            paths.push(string);
        }
    }
    Ok(paths)
}
