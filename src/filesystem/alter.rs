use super::FsResult;
use crate::{EntryObj, Filesystem, InodeChangeset, InodeId, Object, ObjectId};
use tracing::instrument;

#[derive(Clone, Copy, Debug)]
pub struct Alter {
    iid: InodeId,
    skipping: Option<InodeId>,
    replacing: Option<(ObjectId, ObjectId)>,
}

impl Alter {
    pub fn clone(iid: InodeId) -> Self {
        Self {
            iid,
            skipping: None,
            replacing: None,
        }
    }

    pub fn delete(iid: InodeId) -> Self {
        Self {
            iid,
            skipping: Some(iid),
            replacing: None,
        }
    }

    fn replacing(mut self, src: ObjectId, dst: ObjectId) -> Self {
        self.replacing = Some((src, dst));
        self
    }
}

#[derive(Debug)]
pub struct AlterResult {
    pub new_root_oid: ObjectId,
    pub new_oid: Option<ObjectId>,
    pub changeset: InodeChangeset,
}

#[derive(Clone, Copy, Debug)]
struct AlteredChild {
    iid: InodeId,
    old_oid: ObjectId,
    new_oid: ObjectId,
    obj: EntryObj,
}

impl Filesystem {
    #[instrument(skip(self))]
    pub fn alter(&mut self, op: Alter) -> FsResult<AlterResult> {
        let mut changeset = InodeChangeset::default();
        let (new_root_oid, new_oid) = self.alter_ex(&mut changeset, op)?;

        Ok(AlterResult {
            new_root_oid,
            new_oid,
            changeset,
        })
    }

    #[instrument(skip(self, changeset))]
    fn alter_ex(
        &mut self,
        changeset: &mut InodeChangeset,
        op: Alter,
    ) -> FsResult<(ObjectId, Option<ObjectId>)> {
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

        for i in 0..children.len() {
            let next = children.get(i + 1).map(|n| n.new_oid);
            let curr = &mut children[i];

            curr.obj.next = next;
        }

        for child in &children {
            self.objects.set(child.new_oid, Object::Entry(child.obj))?;
        }

        for child in &children {
            changeset.remap(child.iid, child.new_oid);
        }

        if let Some(iid) = op.skipping {
            changeset.free(iid);
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

            changeset.remap(parent_iid, new_root_oid);

            let new_oid = Some(new_oid.unwrap_or(new_root_oid));

            Ok((new_root_oid, new_oid))
        } else {
            let op = if let Some(child) = children.first() {
                Alter::clone(parent_iid).replacing(child.old_oid, child.new_oid)
            } else {
                Alter::clone(parent_iid)
            };

            let (new_root_oid, _) = self.alter_ex(changeset, op)?;

            Ok((new_root_oid, new_oid))
        }
    }
}
