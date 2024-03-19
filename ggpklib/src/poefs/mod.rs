mod local;
mod online;

use std::{
    collections::HashMap,
    io::{self, BufRead, Cursor},
};

use anyhow::anyhow;
use byteorder::{LittleEndian, ReadBytesExt};

use crate::{bundle::Bundle, bundle_index::BundleIndex, dat::DatFile, it::ITFile};
pub use local::LocalSource;
pub use online::OnlineSource;

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

    pub fn get_paths(&self) -> impl Iterator<Item = &String> {
        self.paths.keys()
    }

    /// Helper function to read a .dat file
    pub fn read_dat(&mut self, path: impl AsRef<str>) -> Result<DatFile, anyhow::Error> {
        let bytes = self
            .get_file(path.as_ref())?
            .ok_or(anyhow!("path not found in index bundle",))?;
        let dat_file = DatFile::new(bytes);
        Ok(dat_file)
    }

    /// Helper function to read a utf-16 with bom text file
    pub fn read_txt(&mut self, path: impl AsRef<str>) -> Result<String, anyhow::Error> {
        let bytes = self
            .get_file(path.as_ref())?
            .ok_or(anyhow!("path not found in index bundle"))?;
        let mut bytes = bytes.as_slice();
        if bytes[0] == 0xff && bytes[1] == 0xfe {
            bytes = &bytes[2..];
        }
        let vecu16: Vec<u16> = bytes
            .chunks_exact(2)
            .map(|a| u16::from_le_bytes([a[0], a[1]]))
            .collect();
        let string = String::from_utf16_lossy(&vecu16);
        Ok(string)
    }

    /// Helper function to read a .it file
    pub fn read_it(&mut self, path: impl AsRef<str>) -> Result<ITFile, anyhow::Error> {
        let txt_file = self.read_txt(path)?;
        let it_file = ITFile::parse(txt_file);
        Ok(it_file)
    }

    /// Helper function to read a .it file and recursively extend it from parent .it file
    pub fn read_it_recursive(&mut self, path: impl AsRef<str>) -> Result<ITFile, anyhow::Error> {
        dbg!(path.as_ref());
        let it_file = self.read_it(path)?;
        let extends = &it_file.extends;

        if extends == "nothing" {
            return Ok(it_file);
        }

        let parent_path = format!("{}.it", extends.to_lowercase());
        let parent_it = self.read_it_recursive(&parent_path)?;
        let it_file = it_file.merge(parent_it);
        Ok(it_file)
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
