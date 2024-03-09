use super::FsResult;
use crate::{EntryObj, Filesystem, InodeId, Object, ObjectId};
use tracing::instrument;

impl Filesystem {
    /// Resolves given inode to an object and then clones it recursively,
    /// yielding a new object id.
    ///
    /// This is called e.g. when an entry gets renamed - we clone the entry's
    /// object and then modify the object returned by this function.
    ///
    /// This function can be called at most once per transaction (since it
    /// affects inodes and modifies the root).
    pub fn clone_inode(&mut self, iid: InodeId) -> FsResult<ObjectId> {
        let new_oid = self.alter(Alter::clone(iid))?;

        // Unwrap-safety: Cloning doesn't remove any objects, so `new_oid` is
        // supposed to be `Some` here
        Ok(new_oid.unwrap())
    }

    /// Resolves given inode to an object and then clones its siblings, parent
    /// etc., but without the object itself.
    ///
    /// This is called e.g. when an entry gets deleted - we clone entry's tree,
    /// but without given entry itself, effectively removing it.
    ///
    /// This function can be called at most once per transaction (since it
    /// affects inodes and modifies the root).
    pub fn delete_inode(&mut self, iid: InodeId) -> FsResult<()> {
        self.alter(Alter::clone(iid).skipping(iid))?;

        Ok(())
    }

    #[instrument(skip(self))]
    fn alter(&mut self, op: Alter) -> FsResult<Option<ObjectId>> {
        let parent_iid = self.inodes.resolve_parent(op.iid)?;
        let parent_oid = self.inodes.resolve_object(parent_iid)?;
        let parent = self.objects.get(parent_oid)?.into_entry(parent_oid)?;

        let mut children: Vec<_> = self
            .inodes
            .resolve_children(&mut self.objects, parent_iid)?
            .into_iter()
            .filter(|child_iid| op.skipping.map_or(true, |skipping| skipping != *child_iid))
            .map(|child_iid| {
                let child_old_oid = self.inodes.resolve_object(child_iid)?;
                let child_new_oid = self.objects.alloc(Some(&mut self.tx), Object::Empty)?;
                let child_obj = self.objects.get(child_old_oid)?.into_entry(child_old_oid)?;

                Ok(AlteredChild {
                    iid: child_iid,
                    old_oid: child_old_oid,
                    new_oid: child_new_oid,
                    obj: child_obj,
                })
            })
            .collect::<FsResult<_>>()?;

        if let Some((src_oid, dst_oid)) = op.replacing {
            for child in &mut children {
                if child.obj.body == Some(src_oid) {
                    child.obj.body = Some(dst_oid);
                    break;
                }
            }
        }

        // Children form a linked list - since we've got brand new objects, we
        // must establish connections between them
        for i in 0..children.len() {
            let next = children.get(i + 1).map(|n| n.new_oid);
            let curr = &mut children[i];

            curr.obj.next = next;
        }

        for child in &children {
            self.objects.set(child.new_oid, Object::Entry(child.obj))?;
            self.tx.remap_inode(child.iid, child.new_oid)?;
        }

        if let Some(iid) = op.skipping {
            self.tx.free_inode(iid)?;
        }

        let new_oid = children.iter().find_map(|child| {
            if child.iid == op.iid {
                Some(child.new_oid)
            } else {
                None
            }
        });

        if parent_iid.is_root() {
            let new_root_oid = self.objects.alloc(
                Some(&mut self.tx),
                Object::Entry(EntryObj {
                    body: children.first().map(|child| child.new_oid),
                    ..parent
                }),
            )?;

            self.tx.remap_inode(parent_iid, new_root_oid)?;
            self.tx.set_root(new_root_oid)?;

            Ok(Some(new_oid.unwrap_or(new_root_oid)))
        } else {
            let op = if let Some(child) = children.first() {
                Alter::clone(parent_iid).replacing(child.old_oid, child.new_oid)
            } else {
                Alter::clone(parent_iid)
            };

            self.alter(op)?;

            Ok(new_oid)
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct Alter {
    iid: InodeId,
    skipping: Option<InodeId>,
    replacing: Option<(ObjectId, ObjectId)>,
}

impl Alter {
    fn clone(iid: InodeId) -> Self {
        Self {
            iid,
            skipping: None,
            replacing: None,
        }
    }

    fn skipping(mut self, iid: InodeId) -> Self {
        self.skipping = Some(iid);
        self
    }

    fn replacing(mut self, src: ObjectId, dst: ObjectId) -> Self {
        self.replacing = Some((src, dst));
        self
    }
}

#[derive(Clone, Copy, Debug)]
struct AlteredChild {
    iid: InodeId,
    old_oid: ObjectId,
    new_oid: ObjectId,
    obj: EntryObj,
}
