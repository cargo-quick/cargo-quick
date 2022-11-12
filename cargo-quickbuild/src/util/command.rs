use std::ffi::OsStr;
use std::io::{Error, ErrorKind, Read, Write};
use std::process::{Command, Stdio};
use std::thread;

pub fn command(args: impl IntoIterator<Item = impl AsRef<OsStr>>) -> Command {
    let mut args = args.into_iter();
    let mut command = Command::new(
        args.next()
            .expect("command() takes command and args (at least one item)"),
    );
    command.args(args);
    command
}

pub trait CommandExt {
    /// Execute Command and return a useful error if something went wrong.
    fn try_execute(&mut self) -> Result<(), Error>;
    /// Execute Command and tee stdout and stderr into files.
    fn try_execute_tee(
        &mut self,
        stdout_file: impl Write + Send,
        stderr_file: impl Write + Send,
    ) -> Result<(), Error>;
}

impl CommandExt for Command {
    fn try_execute(&mut self) -> Result<(), Error> {
        let mut child = self.spawn().expect("failed to execute child");
        let ecode = child.wait().expect("failed to wait on child");

        if ecode.success() {
            Ok(())
        } else {
            Err(Error::new(
                ErrorKind::Other,
                "command {self:?} failed: {ecode:?}",
            ))
        }
    }

    fn try_execute_tee(
        &mut self,
        stdout_file: impl Write + Send,
        stderr_file: impl Write + Send,
    ) -> Result<(), Error> {
        let mut child = self
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("failed to execute child");

        let child_out = std::mem::take(&mut child.stdout).expect("cannot attach to child stdout");
        let child_err = std::mem::take(&mut child.stderr).expect("cannot attach to child stderr");

        thread::scope(move |s| {
            s.spawn(|| {
                communicate(child_out, stdout_file, std::io::stdout())
                    .expect("error communicating with child stdout")
            });
            s.spawn(|| {
                communicate(child_err, stderr_file, std::io::stderr())
                    .expect("error communicating with child stderr")
            });
        });

        let ecode = child.wait().expect("failed to wait on child");

        if ecode.success() {
            Ok(())
        } else {
            Err(Error::new(
                ErrorKind::Other,
                "command {self:?} failed: {ecode:?}",
            ))
        }
    }
}

/// adapted from https://stackoverflow.com/questions/66060139/how-to-tee-stdout-stderr-from-a-subprocess-in-rust
fn communicate(
    mut stream: impl Read,
    mut file: impl Write,
    mut output: impl Write,
) -> std::io::Result<()> {
    let mut buf = [0u8; 1024];
    loop {
        let num_read = stream.read(&mut buf)?;
        if num_read == 0 {
            break;
        }

        let buf = &buf[..num_read];
        file.write_all(buf)?;
        output.write_all(buf)?;
    }

    Ok(())
}
