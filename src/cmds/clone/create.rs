use crate::{CloneController, Objects, Storage};
use anyhow::{anyhow, Result};
use std::path::PathBuf;
use structopt::StructOpt;

#[derive(Debug, StructOpt)]
pub struct CreateCloneCmd {
    /// Path to the *.ofs file
    src: PathBuf,

    /// Name of the clone; must be unique across other clone names
    name: String,

    /// `rw` or `ro`, specifying whether the clone is read-write or read-only
    #[structopt(short, long)]
    mode: String,
}

impl CreateCloneCmd {
    pub fn run(self) -> Result<()> {
        let storage = Storage::open(&self.src, true)?;
        let mut objects = Objects::new(storage);

        let is_writable = match self.mode.as_str() {
            "rw" => true,
            "ro" => false,
            mode => return Err(anyhow!("unknown mode: {}", mode)),
        };

        CloneController::new(&mut objects).create(&self.name, is_writable)?;

        println!("ok");

        Ok(())
    }
}
