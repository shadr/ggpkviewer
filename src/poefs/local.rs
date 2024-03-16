use std::{
    fs::File,
    io::{self, Read, SeekFrom},
    path::Path,
};

use crate::{
    bundle::Bundle,
    ggpk::{Entry, EntryData},
};

use super::FileSource;

pub struct LocalSource {
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
