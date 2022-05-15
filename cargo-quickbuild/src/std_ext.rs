// half-baked polyfill for ExitStatus:: Exit_ok()
// FIXME: move this up to Command, and include the failing
// command as part of the error message.
pub trait ExitStatusExt {
    fn exit_ok_ext(&self) -> Result<(), &'static str>;
}
impl ExitStatusExt for std::process::ExitStatus {
    fn exit_ok_ext(&self) -> Result<(), &'static str> {
        if self.success() {
            Ok(())
        } else {
            Err("command failed")
        }
    }
}
