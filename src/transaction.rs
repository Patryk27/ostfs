use crate::{HeaderObj, InodeChangeset, Inodes, ObjectId, Objects};
use anyhow::{Context, Result};
use tracing::warn;

#[derive(Debug, Default)]
pub struct Transaction {
    state: Option<TransactionState>,
}

impl Transaction {
    pub fn begin(&mut self, objects: &mut Objects) -> Result<()> {
        if let Some(state) = self.state.take() {
            if state.dirty {
                warn!("previous transaction got aborted");
            }
        }

        self.state = Some(TransactionState {
            header: objects.get_header()?,
            changeset: Default::default(),
            dirty: false,
        });

        Ok(())
    }

    pub fn get_mut(&mut self) -> Result<&mut TransactionState> {
        self.state
            .as_mut()
            .context("tried to modify a closed transaction")
    }

    pub fn update_root(&mut self, new_root_oid: ObjectId, changeset: InodeChangeset) -> Result<()> {
        let state = self.get_mut()?;

        state.header.root = new_root_oid;
        state.changeset = changeset;
        state.dirty = true;

        Ok(())
    }

    pub fn commit(&mut self, objects: &mut Objects, inodes: Option<&mut Inodes>) -> Result<()> {
        let state = self
            .state
            .take()
            .context("tried to commit a closed transaction")?;

        if state.dirty {
            objects.set_header(state.header)?;

            if !state.changeset.is_empty() {
                state.changeset.apply_to(
                    inodes.context("tried to commit changeset without having inodes alive")?,
                );
            }
        }

        Ok(())
    }
}

#[derive(Debug)]
pub struct TransactionState {
    pub header: HeaderObj,
    pub changeset: InodeChangeset,
    pub dirty: bool,
}
