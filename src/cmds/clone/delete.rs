use crate::{CloneController, Objects, Storage};
use anyhow::Result;
use std::path::PathBuf;
use structopt::StructOpt;

#[derive(Debug, StructOpt)]
pub struct DeleteCloneCmd {
    /// Path to the *.ofs file
    src: PathBuf,

    /// Name of the clone
    name: String,
}

impl DeleteCloneCmd {
    pub fn run(self) -> Result<()> {
        let storage = Storage::open(&self.src, true)?;
        let mut objects = Objects::new(storage);

        CloneController::new(&mut objects).delete(&self.name)?;

        println!("ok");

        Ok(())
    }
}
