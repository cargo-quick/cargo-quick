use std::io::{Error, ErrorKind, Read};

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

pub trait ReadExt: Read {
    fn read_as_string(&mut self) -> Result<String, Error> {
        let mut buf = String::new();
        self.read_to_string(&mut buf)?;
        Ok(buf)
    }
}

impl<T> ReadExt for T where T: Read {}
