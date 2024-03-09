use super::{FsError, FsResult};
use crate::{EntryObj, Filesystem, InodeId, Object, ObjectId};
use anyhow::Result;
use std::ffi::OsStr;
use tracing::instrument;

impl Filesystem {
    /// Appends object to parent (i.e. adds a file/directory into a directory).
    ///
    /// Note that this function works in-place, i.e. it assumes that the parent
    /// has been already cloned.
    #[instrument(skip(self))]
    pub fn append(&mut self, parent_oid: ObjectId, child: Object) -> Result<ObjectId> {
        let parent = self.objects.get(parent_oid)?.into_entry(parent_oid)?;
        let child_oid = self.objects.alloc(Some(&mut self.tx), child)?;

        if let Some(mut oid) = parent.body {
            // If the parent already has a child, find the linked list's tail
            // and append our object there

            loop {
                let obj = self.objects.get(oid)?.into_entry(oid)?;

                if let Some(next) = obj.next {
                    oid = next;
                } else {
                    self.objects.set(
                        oid,
                        Object::Entry(EntryObj {
                            next: Some(child_oid),
                            ..obj
                        }),
                    )?;

                    break;
                }
            }
        } else {
            // If our parent has no children (it's an empty directory), then
            // easy peasy - just modify the parent

            self.objects.set(
                parent_oid,
                Object::Entry(EntryObj {
                    body: Some(child_oid),
                    ..parent
                }),
            )?;
        }

        Ok(child_oid)
    }

    /// Goes through inode's children and looks for the one with given name.
    ///
    /// If no such child exists, bails out with [`FsError::NotFound`].
    #[instrument(skip(self))]
    pub fn find(&mut self, parent_iid: InodeId, name: &OsStr) -> FsResult<(InodeId, EntryObj)> {
        let children = self
            .inodes
            .resolve_children(&mut self.objects, parent_iid)
            .ok()
            .ok_or(FsError::NotFound)?;

        for iid in children {
            let oid = self.inodes.resolve_object(iid)?;
            let obj = self.objects.get(oid)?.into_entry(oid)?;

            if self.objects.get_os_string(obj.name)? == name {
                return Ok((iid, obj));
            }
        }

        Err(FsError::NotFound)
    }

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
        let head_oid = parent.body;

        let mut children: Vec<_> = self
            .inodes
            .resolve_children(&mut self.objects, parent_iid)?
            .into_iter()
            .filter(|iid| op.skipping.map_or(true, |skipping| skipping != *iid))
            .map(|iid| {
                let old_oid = self.inodes.resolve_object(iid)?;
                let new_oid = self.objects.alloc(Some(&mut self.tx), Object::Empty)?;
                let obj = self.objects.get(old_oid)?.into_entry(old_oid)?;

                Ok(AlteredChild {
                    iid,
                    oid: new_oid,
                    obj,
                })
            })
            .collect::<FsResult<_>>()?;

        if let Some((src, dst)) = op.replacing {
            for child in &mut children {
                if child.obj.body == Some(src) {
                    child.obj.body = dst;
                    break;
                }
            }
        }

        // Children form a linked list - since we've got brand new objects, we
        // must establish connections between them
        for i in 0..children.len() {
            let next = children.get(i + 1).map(|n| n.oid);
            let curr = &mut children[i];

            curr.obj.next = next;
        }

        for child in &children {
            self.objects.set(child.oid, Object::Entry(child.obj))?;
            self.tx.remap_inode(child.iid, child.oid)?;
        }

        if let Some(iid) = op.skipping {
            self.tx.free_inode(iid)?;
        }

        let new_oid = children.iter().find_map(|child| {
            if child.iid == op.iid {
                Some(child.oid)
            } else {
                None
            }
        });

        if parent_iid.is_root() {
            let new_root_oid = self.objects.alloc(
                Some(&mut self.tx),
                Object::Entry(EntryObj {
                    body: children.first().map(|child| child.oid),
                    ..parent
                }),
            )?;

            self.tx.remap_inode(parent_iid, new_root_oid)?;
            self.tx.set_root(new_root_oid)?;

            Ok(Some(new_oid.unwrap_or(new_root_oid)))
        } else {
            let old_head_oid = head_oid.unwrap();
            let new_head_oid = children.first().map(|c| c.oid);

            self.alter(Alter::clone(parent_iid).replacing(old_head_oid, new_head_oid))?;

            Ok(new_oid)
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct Alter {
    iid: InodeId,
    skipping: Option<InodeId>,
    replacing: Option<(ObjectId, Option<ObjectId>)>,
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

    fn replacing(mut self, src: ObjectId, dst: Option<ObjectId>) -> Self {
        self.replacing = Some((src, dst));
        self
    }
}

#[derive(Clone, Copy, Debug)]
struct AlteredChild {
    iid: InodeId,
    oid: ObjectId,
    obj: EntryObj,
}
