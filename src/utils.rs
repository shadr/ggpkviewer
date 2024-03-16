use std::{
    io::{self, SeekFrom},
    path::{Component, Path},
};

use crate::ggpk::{Entry, EntryData};

pub fn print_tree(
    entry: &Entry,
    reader: &mut (impl io::Read + io::Seek),
    indentation: u32,
) -> Result<(), io::Error> {
    const INDENT_STR: &str = "│ ";
    let indent = indentation.saturating_sub(1);
    let indent_string = INDENT_STR.repeat(indent as usize);
    print!("{}├─", indent_string);
    match &entry.data {
        EntryData::Free => println!("Free"),
        EntryData::Pdir { name, entries, .. } => {
            println!("{}", name);
            for entry in entries {
                reader.seek(SeekFrom::Start(entry.offset))?;
                let entry = Entry::parse(reader)?;
                print_tree(&entry, reader, indentation + 1)?;
            }
        }
        EntryData::File { name, .. } => {
            println!("{} size: {}", name, entry.data_length_left());
        }
        EntryData::Ggpk { version, entries } => {
            println!("Ggpk version={}", version);

            reader.seek(SeekFrom::Start(entries[0].offset))?;
            let entry = Entry::parse(reader)?;
            print_tree(&entry, reader, indentation + 1)?;

            reader.seek(SeekFrom::Start(entries[1].offset))?;
            let entry = Entry::parse(reader)?;
            print_tree(&entry, reader, indentation + 1)?;
        }
    }
    Ok(())
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
                let found_file = find_file_helper(&entry, reader, path);
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
            let found_file = find_file_helper(&entry, reader, path);
            if found_file.is_some() {
                return found_file;
            }

            reader.seek(SeekFrom::Start(entries[1].offset)).unwrap();
            let entry = Entry::parse(reader).unwrap();
            find_file_helper(&entry, reader, path)
        }
    }
}

/// Find file in ggpk entry, file cursor will be set at the start of file data if file is found
pub fn find_file(
    entry: &Entry,
    reader: &mut (impl io::Read + io::Seek),
    path: &Path,
) -> Option<Entry> {
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
    find_file_helper(entry, reader, &vec)
}
