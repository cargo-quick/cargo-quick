// from https://gist.github.com/erikh/c0a5aa9fde317ec9589271e78c78783c via https://github.com/alexcrichton/tar-rs/issues/102
use std::io::{self, prelude::*};

pub struct PaxBuilder {
    data: Vec<u8>,
}

impl PaxBuilder {
    pub fn new() -> Self {
        Self { data: Vec::new() }
    }

    pub fn add(&mut self, key: &str, value: &str) {
        let mut len_len = 1;
        let mut max_len = 10;
        let rest_len = 3 + key.len() + value.len();
        while rest_len + len_len >= max_len {
            len_len += 1;
            max_len *= 10;
        }
        let len = rest_len + len_len;
        writeln!(&mut self.data, "{} {}={}", len, key, value).unwrap();
    }

    fn as_bytes(&self) -> &[u8] {
        &self.data
    }
}

pub trait BuilderExt {
    /// adds a pax 'x' header, to the tarball, which describes extra details about the file that comes after it.
    /// See  https://man.archlinux.org/man/tar.5.en#Pax_Interchange_Format for details.
    fn append_pax_extensions(&mut self, headers: &PaxBuilder) -> Result<(), io::Error>;
}

impl<T: Write> BuilderExt for tar::Builder<T> {
    fn append_pax_extensions(&mut self, headers: &PaxBuilder) -> Result<(), io::Error> {
        let mut header = tar::Header::new_ustar();
        header.set_size(headers.as_bytes().len() as u64);
        header.set_entry_type(tar::EntryType::XHeader);
        header.set_cksum();
        self.append(&header, headers.as_bytes())
    }
}
