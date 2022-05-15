use std::io::{Error, ErrorKind};

// half-baked polyfill for ExitStatus:: Exit_ok()
// FIXME: move this up to Command, and include the failing
// command as part of the error message.
pub trait ExitStatusExt {
    fn exit_ok_ext(&self) -> Result<(), Error>;
}
impl ExitStatusExt for std::process::ExitStatus {
    fn exit_ok_ext(&self) -> Result<(), Error> {
        if self.success() {
            Ok(())
        } else {
            Err(Error::new(ErrorKind::Other, "command failed"))
        }
    }
}
