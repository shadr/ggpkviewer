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
