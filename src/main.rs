mod clone;
mod cmds;
mod filesystem;
mod inode;
mod inode_changeset;
mod inode_id;
mod inodes;
mod object;
mod object_id;
mod object_rw;
mod objects;
mod storage;
mod transaction;

pub use self::clone::*;
pub use self::cmds::*;
pub use self::filesystem::*;
pub use self::inode::*;
pub use self::inode_changeset::*;
pub use self::inode_id::*;
pub use self::inodes::*;
pub use self::object::*;
pub use self::object_id::*;
pub use self::object_rw::*;
pub use self::objects::*;
pub use self::storage::*;
pub use self::transaction::*;
use anyhow::Result;
use structopt::StructOpt;

/// OstFS, a made-for-fun FUSE filesystem with support for zero-cost snapshots
/// and clones
#[derive(Debug, StructOpt)]
enum Cmd {
    Clone(CloneCmd),
    Collect(CollectCmd),
    Create(CreateCmd),
    Inspect(InspectCmd),
    Mount(MountCmd),
}

fn main() -> Result<()> {
    match Cmd::from_args() {
        Cmd::Clone(cmd) => cmd.run(),
        Cmd::Create(cmd) => cmd.run(),
        Cmd::Collect(cmd) => cmd.run(),
        Cmd::Inspect(cmd) => cmd.run(),
        Cmd::Mount(cmd) => cmd.run(),
    }
}
