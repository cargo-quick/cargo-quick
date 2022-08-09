// At some point I will pick a command-line parsing crate, but for now this will do.
pub fn exec(args: &[String]) -> anyhow::Result<()> {
    assert_eq!(args[0], "install");
    Ok(())
}
