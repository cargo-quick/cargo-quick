use std::error::Error;
use std::fs::File;

use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct Record {
    id: String,
    name: String,
    has_cargo_toml: bool,
    has_cargo_lock: bool,
}

fn get_first_arg() -> Result<std::ffi::OsString, Box<dyn std::error::Error>> {
    match std::env::args_os().nth(1) {
        None => Err(From::from("expected 1 argument, but got none")),
        Some(file_path) => Ok(file_path),
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    let repo_root = get_first_arg()?;
    let file_path = std::path::Path::new(&repo_root).join("data/github.csv".to_string());
    let file = File::open(file_path)?;
    let mut rdr = csv::Reader::from_reader(file);

    let records = rdr.deserialize();

    let valid_records = records
        .map(|record: Result<Record, csv::Error>| record.unwrap())
        .filter(|record| record.has_cargo_lock)
        .collect::<Vec<Record>>();

    let repo_root_str = repo_root.to_str().unwrap();

    for record in valid_records {
        let path = format!("{}/data/{}/Cargo.toml", repo_root_str, record.name);

        println!("The path is {}", path);

        if std::path::Path::new(&path).exists() {
            continue;
        }
    }

    Ok(())
}
