//! Minimal OLE2/CFB access: open the container and read named streams.

use std::io::{Cursor, Read};

use crate::error::{Error, Result};

/// `.doc` magic — OLE2/CFB compound file header.
pub(crate) fn is_ole2(bytes: &[u8]) -> bool {
    bytes.len() >= 8 && bytes[0..8] == [0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1]
}

/// An opened compound file backed by an owned byte buffer.
pub(crate) struct Container {
    inner: cfb::CompoundFile<Cursor<Vec<u8>>>,
}

impl Container {
    pub(crate) fn open(bytes: &[u8]) -> Result<Self> {
        if !is_ole2(bytes) {
            return Err(Error::NotOle2);
        }
        let inner = cfb::CompoundFile::open(Cursor::new(bytes.to_vec()))?;
        Ok(Self { inner })
    }

    /// Read a root-level stream by name, e.g. `"WordDocument"`. Returns `None`
    /// when the stream does not exist (so callers can try alternates).
    pub(crate) fn stream(&mut self, name: &str) -> Result<Option<Vec<u8>>> {
        let path = format!("/{name}");
        if !self.inner.exists(&path) || !self.inner.is_stream(&path) {
            return Ok(None);
        }
        let mut s = self.inner.open_stream(&path)?;
        let mut buf = Vec::new();
        s.read_to_end(&mut buf)?;
        Ok(Some(buf))
    }

    /// Read a required stream, erroring if absent.
    pub(crate) fn required(&mut self, name: &'static str) -> Result<Vec<u8>> {
        self.stream(name)?.ok_or(Error::MissingStream(name))
    }
}
