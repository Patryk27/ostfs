mod create;
mod delete;
mod list;

pub use self::create::*;
pub use self::delete::*;
pub use self::list::*;
use anyhow::Result;
use structopt::StructOpt;

#[derive(Debug, StructOpt)]
pub enum CloneCmd {
    Create(CreateCloneCmd),
    Delete(DeleteCloneCmd),
    List(ListCloneCmd),
}

impl CloneCmd {
    pub fn run(self) -> Result<()> {
        match self {
            CloneCmd::Create(cmd) => cmd.run(),
            CloneCmd::Delete(cmd) => cmd.run(),
            CloneCmd::List(cmd) => cmd.run(),
        }
    }
}
