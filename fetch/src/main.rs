use futures::stream::StreamExt as _;
use std::fs::{create_dir_all, rename, File};
use std::io::copy;
use std::{error::Error, time::Duration};
use tempfile::Builder;

use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct RepoRecord {
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

    while response.status() == 429 {
        println!("hit a rate limit");
        tokio::time::sleep(Duration::from_secs(5 * 60)).await;
        response = reqwest::get(&target).await?;
    }

    if !response.status().is_success() {
        branch = "main";
        let target = format!(
            "https://raw.githubusercontent.com/{}/{}/Cargo.lock",
            repo, branch
        );
        response = reqwest::get(&target).await?;

        while response.status() == 429 {
            println!("hit a rate limit");
            tokio::time::sleep(Duration::from_secs(5 * 60)).await;
            response = reqwest::get(&target).await?;
        }
    }

    if response.status() == 404 {
        return Ok(());
    } else if !response.status().is_success() {
        dbg!(response);
        panic!("unexpected status")
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

async fn fetch_single(rust_repos_dir: &str, repo_record: &RepoRecord) {
    let path = format!(
        "{}/data/locks/{}/Cargo.lock",
        &rust_repos_dir, &repo_record.name
    );
    if std::path::Path::new(&path).exists() {
        return;
    }

    fetch_and_write_file(rust_repos_dir, &repo_record.name)
        .await
        .unwrap();
}

#[allow(dead_code)]
async fn fetch_batch(rust_repos_dir: &str, valid_repo_records: &[RepoRecord]) {
    for repo_record in valid_repo_records {
        fetch_single(rust_repos_dir, repo_record).await
    }
}

#[tokio::main]
pub async fn main() -> Result<(), Box<dyn Error>> {
    let rust_repos_dir = get_first_arg()?;
    let rust_repos_dir = rust_repos_dir.to_str().unwrap();
    let repo_list_csv_path =
        std::path::Path::new(&rust_repos_dir).join("data/github.csv".to_string());

    let file = File::open(repo_list_csv_path)?;
    let mut reader = csv::Reader::from_reader(file);
    let repo_records = reader.deserialize();

    let valid_repo_records = repo_records
        .map(|repo_record: Result<RepoRecord, csv::Error>| repo_record.unwrap())
        .filter(|repo_record| repo_record.has_cargo_lock)
        .collect::<Vec<RepoRecord>>();

    println!("Valid repo records count: {}", valid_repo_records.len());

    let mut futures = vec![];
    for repo_record in valid_repo_records.iter() {
        futures.push(fetch_single(rust_repos_dir, repo_record));
    }
    futures::stream::iter(futures)
        .buffer_unordered(100)
        .collect::<Vec<()>>()
        .await;

    Ok(())
}
