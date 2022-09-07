use anyhow::bail;

// At some point I will pick a command-line parsing crate, but for now this will do.
pub fn exec(args: &[String]) -> anyhow::Result<()> {
    assert_eq!(args[0], "repo");
    assert_eq!(args[1], "search");
    if args.len() != 3 {
        bail!("USAGE: cargo quickbuild repo search $filename");
    }

    let filename = args[1].as_str();
    assert_eq!(args, &["repo", "search", filename]);

    Ok(())
}
