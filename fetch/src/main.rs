use std::error::Error;
use std::fs::{create_dir_all, rename, File};
use std::io::copy;
use tempfile::Builder;

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

async fn fetch_and_write_file(
    repo_root: &str,
    repo: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut branch = "master";
    let target = format!(
        "https://raw.githubusercontent.com/{}/{}/Cargo.lock",
        repo, branch
    );

    let mut response = reqwest::get(&target).await?;

    if !response.status().is_success() {
        branch = "main";
        let target = format!(
            "https://raw.githubusercontent.com/{}/{}/Cargo.lock",
            repo, branch
        );
        response = reqwest::get(&target).await?;
    }

    if !response.status().is_success() {
        return Ok(());
    }

    let tmp_dir = Builder::new().tempdir()?;
    let fname = response
        .url()
        .path_segments()
        .and_then(|segments| segments.last())
        .and_then(|name| if name.is_empty() { None } else { Some(name) })
        .unwrap_or("tmp.bin");

    let fname = tmp_dir.path().join(fname);
    let mut temp_destination = { File::create(&fname)? };

    let content = response.text().await?;
    copy(&mut content.as_bytes(), &mut temp_destination)?;

    let perm_directory = format!("{}/data/locks/{}", repo_root, repo);
    create_dir_all(&perm_directory)?;

    let perm_dest_path = format!("{}/Cargo.lock", &perm_directory);
    rename(fname, &perm_dest_path)?;

    println!("Cargo.lock retrieved: {}", &perm_dest_path);

    Ok(())
}

async fn fetch_batch(repo_root_str: &str, valid_records: &[Record]) {
    for record in valid_records {
        let path = format!("{}/data/locks/{}/Cargo.lock", &repo_root_str, &record.name);
        if std::path::Path::new(&path).exists() {
            continue;
        }

        fetch_and_write_file(repo_root_str, &record.name)
            .await
            .unwrap();
    }
}

#[tokio::main]
pub async fn main() -> Result<(), Box<dyn Error>> {
    let repo_root = get_first_arg()?;
    let file_path = std::path::Path::new(&repo_root).join("data/github.csv".to_string());
    let file = File::open(file_path)?;
    let mut rdr = csv::Reader::from_reader(file);

    let records = rdr.deserialize();

    let valid_records = records
        .map(|record: Result<Record, csv::Error>| record.unwrap())
        .filter(|record| record.has_cargo_lock)
        .collect::<Vec<Record>>();

    println!("Valid records count: {}", valid_records.len());

    let repo_root_str = repo_root.to_str().unwrap();

    let mut futures = vec![];
    for chunk in valid_records.chunks(1_000) {
        futures.push(fetch_batch(repo_root_str, chunk));
    }

    futures::future::join_all(futures).await;

    Ok(())
}
