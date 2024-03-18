use std::path::{Path, PathBuf};

use ggpklib::dat::{DatFile, DatValue};
use ggpklib::dat_schema::SchemaFile;
use ggpklib::poefs::{LocalSource, OnlineSource, PoeFS};

use clap::Parser;

#[derive(Debug, Parser)]
#[clap(group(clap::ArgGroup::new("source").required(true)))]
struct Args {
    #[arg(
        short,
        long,
        group = "source",
        requires = "schema_path",
        help = "Get files from local GGPK file"
    )]
    ggpk: Option<PathBuf>,
    #[arg(
        short,
        long,
        group = "source",
        help = "Get requested file from patch server"
    )]
    online: bool,
    #[arg(
        short,
        long,
        help = "Path to schema.json file, only needed if '--ggpk' argument is used"
    )]
    schema_path: Option<PathBuf>,
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

fn datvalue_to_csv_cell(value: DatValue) -> String {
    match value {
        DatValue::Bool(b) => b.to_string(),
        DatValue::String(s) => s,
        DatValue::I32(i) => i.to_string(),
        DatValue::F32(f) => f.to_string(),
        DatValue::Array(a) => {
            let a = a.into_iter().map(datvalue_to_csv_cell).collect::<Vec<_>>();
            let joined = a.join(";");
            format!("[{joined}]")
        }
        DatValue::Row(r) => format!("{r:?}"),
        DatValue::ForeignRow { rid, .. } => {
            format!("{rid:?}")
        }
        DatValue::EnumRow(r) => r.to_string(),
    }
}

fn save_dat_file(
    bytes: Vec<u8>,
    schema: &SchemaFile,
    path: impl AsRef<Path>,
    output: impl AsRef<Path>,
) -> Result<(), anyhow::Error> {
    let table_name = path.as_ref().file_stem().unwrap().to_str().unwrap();
    let file_dat = DatFile::new(bytes);

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
    for i in 0..file_dat.row_count() as usize {
        let mut row = file_dat.nth_row(i);
        let values = row.read_with_schema(file_columns);
        let values = values.into_iter().map(datvalue_to_csv_cell);
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

fn get_file(
    fs: &mut PoeFS,
    path: PathBuf,
    output: PathBuf,
    schema: &SchemaFile,
) -> Result<(), anyhow::Error> {
    let extension = path.extension().unwrap().to_str().unwrap();
    let file_bytes = fs.get_file(path.to_str().unwrap())?.unwrap();

    match extension {
        "dat64" => {
            save_dat_file(file_bytes, schema, path, output)?;
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
    let schema;
    let mut fs = if let Some(path) = args.ggpk {
        schema = SchemaFile::read_from_file(args.schema_path.unwrap())?;
        PoeFS::new(LocalSource::new(path)?)
    } else if args.online {
        todo!();
        PoeFS::new(OnlineSource::new(None))
    } else {
        unreachable!()
    };
    match args.command {
        Command::Get { file, output } => get_file(&mut fs, file, output, &schema)?,
        Command::ListPaths => {
            for path in fs.get_paths() {
                println!("{path}");
            }
        }
    }
    Ok(())
}
