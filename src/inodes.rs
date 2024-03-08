use crate::{Inode, InodeId, ObjectId, Objects};
use anyhow::{Context, Result};
use std::collections::HashMap;
use tracing::{instrument, trace};

#[derive(Debug)]
pub struct Inodes {
    nodes: HashMap<InodeId, Inode>,
    next_iid: InodeId,
}

impl Inodes {
    pub fn new(root_oid: ObjectId) -> Result<Self> {
        let nodes = HashMap::from_iter([(
            InodeId::ROOT,
            Inode {
                oid: root_oid,
                parent: InodeId::ROOT,
                children: Default::default(),
            },
        )]);

        Ok(Self {
            nodes,
            next_iid: InodeId::new(2),
        })
    }

    #[instrument(skip(self))]
    pub fn alloc(&mut self, parent_iid: InodeId, oid: ObjectId) -> Result<InodeId> {
        let parent = self
            .nodes
            .get(&parent_iid)
            .with_context(|| format!("{:?} is dead", parent_iid))?;

        if let Some(children) = &parent.children {
            for iid in children {
                if self.nodes[iid].oid == oid {
                    return Ok(*iid);
                }
            }
        }

        let iid = self.next_iid.fetch_add()?;

        self.nodes.insert(
            iid,
            Inode {
                oid,
                parent: parent_iid,
                children: Default::default(),
            },
        );

        self.nodes
            .get_mut(&parent_iid)
            .unwrap()
            .children
            .get_or_insert_with(Default::default)
            .push(iid);

        trace!("allocated inode {:?}", iid);

        Ok(iid)
    }

    #[instrument(skip(self))]
    pub fn remap(&mut self, iid: InodeId, oid: ObjectId) {
        trace!("remapping inode");

        if let Some(inode) = self.nodes.get_mut(&iid) {
            inode.oid = oid;
        }
    }

    #[instrument(skip(self))]
    pub fn free(&mut self, iid: InodeId) {
        trace!("freeing inode");

        let Some(inode) = self.nodes.remove(&iid) else {
            return;
        };

        if let Some(parent) = self.nodes.get_mut(&inode.parent) {
            if let Some(children) = &mut parent.children {
                if let Some(chd_idx) = children.iter().position(|chd| *chd == iid) {
                    children.swap_remove(chd_idx);
                }
            }
        }

        if let Some(children) = inode.children {
            for child_iid in children {
                self.free(child_iid);
            }
        }
    }

    pub fn mark_as_empty(&mut self, iid: InodeId) {
        if let Some(inode) = self.nodes.get_mut(&iid) {
            inode.children = Some(Default::default());
        }
    }

    pub fn resolve_object(&self, iid: InodeId) -> Result<ObjectId> {
        self.nodes
            .get(&iid)
            .map(|inode| inode.oid)
            .with_context(|| format!("{:?} is dead", iid))
    }

    pub fn resolve_parent(&self, iid: InodeId) -> Result<InodeId> {
        self.nodes
            .get(&iid)
            .map(|inode| inode.parent)
            .with_context(|| format!("{:?} is dead", iid))
    }

    pub fn resolve_children(
        &mut self,
        objects: &mut Objects,
        iid: InodeId,
    ) -> Result<Vec<InodeId>> {
        let inode = self
            .nodes
            .get(&iid)
            .with_context(|| format!("{:?} is dead", iid))?;

        if inode.children.is_none() {
            let obj = objects.get(inode.oid)?.into_entry(inode.oid)?;

            let children = if obj.body.is_none() {
                Some(Default::default())
            } else {
                let mut children = Vec::new();
                let mut cursor = obj.body;

                while let Some(oid) = cursor {
                    children.push(self.alloc(iid, oid)?);

                    cursor = objects.get(oid)?.into_entry(oid)?.next;
                }

                Some(children)
            };

            self.nodes.get_mut(&iid).unwrap().children = children;
        }

        // TODO get rid of extra allocation
        Ok(self.nodes[&iid].children.clone().unwrap())
    }
}
