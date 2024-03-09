use crate::{CloneObj, FilesystemOrigin, HeaderObj, InodeId, Inodes, Object, ObjectId, Objects};
use anyhow::{anyhow, Context, Result};
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
            dirty: false,
            new_root: None,
            new_header: objects.get_header()?,
            inodes_to_remap: Default::default(),
            inodes_to_free: Default::default(),
        });

        Ok(())
    }

    pub fn get_mut(&mut self) -> Result<&mut TransactionState> {
        self.state
            .as_mut()
            .context("tried to modify a closed transaction")
    }

    /// Schedules given node to replace the current root node.
    ///
    /// If we're working on a clone, this updates the clone's root - otherwise
    /// this updates the header's root.
    pub fn set_root(&mut self, new_root_oid: ObjectId) -> Result<()> {
        let tx = self.get_mut()?;

        if tx.new_root.is_some() {
            return Err(anyhow!("set_root() called twice in a single transaction"));
        }

        tx.dirty = true;
        tx.new_root = Some(new_root_oid);

        Ok(())
    }

    /// Schedules given inode to point at new object.
    pub fn remap_inode(&mut self, src: InodeId, dst: ObjectId) -> Result<()> {
        let tx = self.get_mut()?;

        tx.dirty = true;
        tx.inodes_to_remap.push((src, dst));

        Ok(())
    }

    /// Schedules given inode to be freed.
    pub fn free_inode(&mut self, iid: InodeId) -> Result<()> {
        let tx = self.get_mut()?;

        tx.dirty = true;
        tx.inodes_to_free.push(iid);

        Ok(())
    }

    /// Applies scheduled changes atomically and returns whether anything got
    /// changed.
    ///
    /// (i.e. false = transaction was a no-op)
    pub fn commit(
        &mut self,
        objects: &mut Objects,
        inodes: Option<&mut Inodes>,
        origin: FilesystemOrigin,
    ) -> Result<bool> {
        let mut tx = self
            .state
            .take()
            .context("tried to commit a closed transaction")?;

        if !tx.dirty {
            return Ok(false);
        }

        if let Some(new_root_oid) = tx.new_root {
            if let FilesystemOrigin::Main { .. } = origin {
                tx.new_header.root = new_root_oid;
            }
        }

        objects.set_header(tx.new_header)?;

        if let Some(new_root_oid) = tx.new_root {
            if let FilesystemOrigin::Clone { oid, .. } = origin {
                let obj = CloneObj {
                    root: new_root_oid,
                    ..objects.get(oid)?.into_clone(oid)?
                };

                objects.set(oid, Object::Clone(obj))?;
            }
        }

        if !tx.inodes_to_remap.is_empty() || !tx.inodes_to_free.is_empty() {
            let inodes = inodes.context("tried to commit changeset without having any inodes")?;

            for (src, dst) in tx.inodes_to_remap {
                inodes.remap(src, dst);
            }

            for iid in tx.inodes_to_free {
                inodes.free(iid);
            }
        }

        Ok(true)
    }
}

#[derive(Debug)]
pub struct TransactionState {
    pub dirty: bool,
    pub new_root: Option<ObjectId>,
    pub new_header: HeaderObj,
    pub inodes_to_remap: Vec<(InodeId, ObjectId)>,
    pub inodes_to_free: Vec<InodeId>,
}
