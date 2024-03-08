use crate::Object;
use anyhow::{anyhow, Context, Result};
use std::fs::File;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::Path;
use tracing::{info, instrument};

#[derive(Debug)]
pub struct Storage {
    file: File,
    can_grow: bool,
}

#[allow(clippy::len_without_is_empty)]
impl Storage {
    #[instrument]
    pub fn create(path: &Path) -> Result<Self> {
        info!("creating store");

        let file = File::create_new(path)
            .with_context(|| format!("couldn't create: {}", path.display()))?;

        Ok(Self {
            file,
            can_grow: true,
        })
    }

    #[instrument]
    pub fn open(path: &Path, can_grow: bool) -> Result<Self> {
        info!("opening store");

        let file = File::options()
            .read(true)
            .write(true)
            .open(path)
            .with_context(|| format!("couldn't open: {}", path.display()))?;

        Ok(Self { file, can_grow })
    }

    pub fn len(&mut self) -> Result<u32> {
        let bytes = self.file.seek(SeekFrom::End(0))?;

        Ok((bytes as u32) / (Object::SIZE as u32))
    }

    pub fn seek(&mut self, object_id: u32) -> Result<()> {
        self.file
            .seek(SeekFrom::Start((object_id as u64) * (Object::SIZE as u64)))
            .with_context(|| format!("seek() failed: oid={}", object_id))?;

        Ok(())
    }

    pub fn read(&mut self, object_id: u32) -> Result<[u8; Object::SIZE]> {
        let mut object = [0; Object::SIZE];

        self.seek(object_id)?;

        self.file
            .read_exact(&mut object)
            .with_context(|| format!("read() failed: oid={}", object_id))?;

        Ok(object)
    }

    pub fn write(&mut self, object_id: u32, object: [u8; Object::SIZE]) -> Result<()> {
        self.seek(object_id)?;

        self.file
            .write_all(&object)
            .with_context(|| format!("write() failed: ois={}", object_id))?;

        Ok(())
    }

    pub fn append(&mut self, object: [u8; Object::SIZE]) -> Result<u32> {
        if !self.can_grow {
            return Err(anyhow!(
                "cannot create a new object, because storage was opened in non-growable mode"
            ));
        }

        let pos = self
            .file
            .seek(SeekFrom::End(0))
            .context("seek() failed")
            .context("append() failed")?;

        self.file
            .write_all(&object)
            .context("write() failed")
            .context("append() failed")?;

        Ok((pos as u32) / (Object::SIZE as u32))
    }
}
