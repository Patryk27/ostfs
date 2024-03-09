use crate::{CloneController, Objects, Storage};
use anyhow::Result;
use std::path::PathBuf;
use structopt::StructOpt;

#[derive(Debug, StructOpt)]
pub struct CreateCloneCmd {
    /// Path to the *.ofs file
    src: PathBuf,

    /// Name of the clone; must be unique across other clone names
    name: String,

    /// When specified, clone is read-only
    #[structopt(short, long)]
    read_only: bool,
}

impl CreateCloneCmd {
    pub fn run(self) -> Result<()> {
        let storage = Storage::open(&self.src, true)?;
        let mut objects = Objects::new(storage);

        CloneController::new(&mut objects).create(&self.name, !self.read_only)?;

        println!("ok");

        Ok(())
    }
}
