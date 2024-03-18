# About
**ggpkviewer** contains various tools for reading ggpk files:
-  [ggpklib](#library) - Rust library that implements logic for parsing files
- [ggpkcli](#cli) - CLI tool for getting files from ggpk or straight from patch servers

# Library
Add library as dependency:
```toml
[dependencies]
ggpklib = { git = "https://github.com/shadr/ggpkviewer.git" }
```

To get files first we need to create instance of our virtual file system
```rust
use ggpklib::poefs::{PoeFS, local:LocalSource, online::OnlineSource};

// Create source for file system from local GGPK file
let source = LocalSource::new(path).unwrap();
// Or from patch server
let source = Online::new().unwrap();

let poe_fs = PoeFS::new(source);
```
For example we want to get Mods.dat64 file, using `PoeFS` function `get_file` we can get uncompressed bytes of wanted file
> note: currently you need to specify file path in lowercase only, with extension and starting with **/**
```rust
let mods_bytes = poe_fs.get_file("/data/mods.dat64").unwrap();
```
Bytes of dat files are not very usefull without knowing how to read them, we need to parse those bytes using `SchemaFile`
> schema is reverse engineered by community members and availabe in [poe-tool-dev/dat-schema](https://github.com/poe-tool-dev/dat-schema) repo

```rust
let file_dat = DatFile::new(mods_bytes);
let schema_content = std::fs::read_to_string("schema.min.json").unwrap();
let schema: SchemaFile = serde_json::from_str(&schema_content).unwrap();
// Find schema for wanted table by its name, case insensitive
let file_schema = schema.find_table("mods").unwrap();
// Get column definitions
let file_columns = &file_schema.columns;

for i in 0..file_dat.row_count as usize {
    let mut row = file_dat.nth_row(i);
    // Values will contain Vec<DatValue> where DatValue is enum of possible types
    // values[nth] corresponds to file_columns[nth] column definition
    let values = row.read_with_schema(file_columns);
    // ... do something with values
}
```
# CLI
You can use supported commands:
```sh
ggpkcli --help
```
Save dat64 file from local GGPK file to csv:
```sh
ggpkcli --ggpk /path/to/ggpk/file get /data/mods.dat64 mods.csv
```

Print list of paths from online file:
```sh
ggpkcli --online list-paths
```
Use tools like `grep` to filter paths:
```sh
ggpkcli --online list-paths | grep mods.dat

```
# Credits
Thanks to these libraries and their contributors because half of this project is based on them:
- [PyPoE](https://github.com/Project-Path-of-Exile-Wiki/PyPoE) - GGPK parsing library written in python, more mature and featureful, used to get lastest data for wiki
- [RePoE](https://github.com/lvlvllvlvllvlvl/RePoE/) - tool for saving game data to json, uses PyPoE, gets latest game data automatically via CI
