pub mod bundle;
pub mod bundle_index;
pub mod dat;
pub mod dat_schema;
pub mod ggpk;
pub mod poefs;
pub mod utils;

use dat::DatFile;
use dat_schema::SchemaFile;
use poefs::{local::LocalSource, online::OnlineSource, PoeFS};
use std::path::PathBuf;

use clap::Parser;

#[derive(Debug, Parser)]
#[clap(group(clap::ArgGroup::new("source").required(true)))]
struct Args {
    #[arg(short, long, group = "source")]
    ggpk: Option<PathBuf>,
    #[arg(short, long, group = "source")]
    online: bool,
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
    let mods = fs.get_file("data/mods.dat64")?.unwrap();
    let mods_dat = DatFile::new(mods);

    let schema_content = std::fs::read_to_string("schema.min.json")?;
    let schema: SchemaFile = serde_json::from_str(&schema_content)?;
    let mods_schema = schema.tables.iter().find(|t| t.name == "Mods").unwrap();
    let mods_columns = &mods_schema.columns;

    let mut wtr = csv::Writer::from_path("output.csv")?;
    let mut unknown_count = 0;
    let headers = mods_columns.iter().map(|c| {
        c.name.clone().unwrap_or_else(|| {
            let s = format!("Unknown{unknown_count}");
            unknown_count += 1;
            s
        })
    });
    wtr.write_record(headers)?;
    for i in 0..mods_dat.row_count as usize {
        let mut row = mods_dat.nth_row(i);
        let values = row.read_with_schema(mods_columns);
        let values = values.into_iter().map(|v| v.to_csv());
        wtr.write_record(values)?;
    }
    wtr.flush()?;
    Ok(())
}
