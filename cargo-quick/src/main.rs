fn main() {
    let mut args: Vec<_> = std::env::args().collect();
    if args[0] == "quick" {
        args.remove(1);
    }
    match args[1].as_str() {
        "install" => {
            let exitcode = std::process::Command::new("cargo-quickinstall")
                .args(&args[2..])
                .status()
                .unwrap();
            std::process::exit(exitcode.code().unwrap_or(1));
        }
        other => todo!("TODO: implement {other}"),
    }
}
