pub mod bundle;
pub mod bundle_index;
pub mod dat;
pub mod dat_schema;
pub mod ggpk;
pub mod poefs;
pub mod translation;
pub mod utils;

use dat::DatFile;
use dat_schema::SchemaFile;
use poefs::{local::LocalSource, online::OnlineSource, PoeFS};
use std::path::{Path, PathBuf};
use translation::TranslationFile;

use clap::Parser;

#[derive(Debug, Parser)]
#[clap(group(clap::ArgGroup::new("source").required(true)))]
struct Args {
    #[arg(short, long, group = "source")]
    ggpk: Option<PathBuf>,
    #[arg(short, long, group = "source")]
    online: bool,
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, clap::Subcommand)]
pub enum Command {
    Get {
        file: PathBuf,
        #[arg(default_value = "output.csv")]
        output: PathBuf,
    },
    ListPaths,
}

fn save_dat_file(
    bytes: Vec<u8>,
    path: impl AsRef<Path>,
    output: impl AsRef<Path>,
) -> Result<(), anyhow::Error> {
    let table_name = path.as_ref().file_stem().unwrap().to_str().unwrap();
    let file_dat = DatFile::new(bytes);

    let schema_content = std::fs::read_to_string("schema.min.json")?;
    let schema: SchemaFile = serde_json::from_str(&schema_content)?;
    let file_schema = schema.find_table(table_name).unwrap();
    let file_columns = &file_schema.columns;

    let mut wtr = csv::Writer::from_path(output)?;
    let mut unknown_count = 0;
    let headers = file_columns.iter().map(|c| {
        c.name.clone().unwrap_or_else(|| {
            let s = format!("Unknown{unknown_count}");
            unknown_count += 1;
            s
        })
    });

    wtr.write_record(headers)?;
    for i in 0..file_dat.row_count as usize {
        let mut row = file_dat.nth_row(i);
        let values = row.read_with_schema(file_columns);
        let values = values.into_iter().map(|v| v.to_csv());
        wtr.write_record(values)?;
    }
    wtr.flush()?;
    Ok(())
}

fn save_txt_file(
    bytes: Vec<u8>,
    _path: impl AsRef<Path>,
    output: impl AsRef<Path>,
) -> Result<(), anyhow::Error> {
    let vecu16: Vec<u16> = bytes
        .chunks_exact(2)
        .map(|a| u16::from_ne_bytes([a[0], a[1]]))
        .collect();
    let text = String::from_utf16_lossy(&vecu16);
    std::fs::write(output, text)?;
    Ok(())
}

fn get_file(fs: &mut PoeFS, path: PathBuf, output: PathBuf) -> Result<(), anyhow::Error> {
    let extension = path.extension().unwrap().to_str().unwrap();
    let file_bytes = fs.get_file(path.to_str().unwrap())?.unwrap();

    match extension {
        "dat64" => {
            save_dat_file(file_bytes, path, output)?;
        }
        "txt" => {
            save_txt_file(file_bytes, path, output)?;
        }
        _ => unimplemented!(
            "Reading files with extension: '{}' not supported yet",
            extension
        ),
    }

    Ok(())
}

fn main() -> Result<(), anyhow::Error> {
    let args = Args::parse();
    let mut fs = if let Some(path) = args.ggpk {
        PoeFS::new(LocalSource::new(path)?)
    } else if args.online {
        PoeFS::new(OnlineSource::new(None))
    } else {
        unreachable!()
    };
    match args.command {
        Command::Get { file, output } => get_file(&mut fs, file, output)?,
        Command::ListPaths => {
            for path in fs.get_paths() {
                println!("{path}");
            }
        }
    }
    Ok(())
}
