use crate::{CloneController, Objects, Storage};
use anyhow::Result;
use std::path::PathBuf;
use structopt::StructOpt;

#[derive(Debug, StructOpt)]
pub struct ListCloneCmd {
    /// Path to the *.ofs file
    src: PathBuf,
}

impl ListCloneCmd {
    pub fn run(self) -> Result<()> {
        let storage = Storage::open(&self.src, false)?;
        let mut objects = Objects::new(storage);
        let clones = CloneController::new(&mut objects).all()?;

        match clones.len() {
            0 => println!("found no clones"),
            1 => println!("found 1 clone:"),
            n => println!("found {} clones:", n),
        }

        for (clone_id, clone) in clones.into_iter().enumerate() {
            println!("- #{}: {}", clone_id, clone.name);
        }

        Ok(())
    }
}
