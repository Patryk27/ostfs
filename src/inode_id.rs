use anyhow::{Context, Result};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct InodeId(u64);

impl InodeId {
    pub const ROOT: Self = Self(1);

    pub fn new(iid: u64) -> Self {
        Self(iid)
    }

    pub fn fetch_add(&mut self) -> Result<Self> {
        let this = *self;

        self.0 = self
            .0
            .checked_add(1)
            .context("reached the maximum number of inodes")?;

        Ok(this)
    }

    pub fn is_root(self) -> bool {
        self == Self::ROOT
    }

    pub fn get(self) -> u64 {
        self.0
    }
}
