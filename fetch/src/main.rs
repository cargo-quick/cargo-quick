use std::fs::File;

use csv::Position;

fn get_first_arg() -> Result<std::ffi::OsString, Box<dyn std::error::Error>> {
    match std::env::args_os().nth(1) {
        None => Err(From::from("expected 1 argument, but got none")),
        Some(file_path) => Ok(file_path),
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let file_path = get_first_arg()?;
    let file = File::open(file_path)?;
    let mut rdr = csv::Reader::from_reader(file);

    println!(
        "The number of ALL records in CSV is {}",
        rdr.records().count()
    );

    let pos = Position::new();
    rdr.seek(pos)?;
    let recs_with_cargo_lock = rdr
        .records()
        .filter(|record| &record.as_ref().unwrap()[3] == "true")
        .collect::<Vec<_>>();

    println!(
        "The number of true records in CSV is {}",
        recs_with_cargo_lock.iter().count()
    );
    for result in rdr.records() {
        let record = result?;
        let has_lockfile = &record[3];
        println!("{:?}", has_lockfile);
    }

    Ok(())
}

// fetch function
// Check that file hasn't been downloaded
// Grab file and stick in directory
// main function tokio
// async reqwest fetch
// for loop to call fetch function
// sequential

// other
// futures unordered (refactored)
// parrallel
