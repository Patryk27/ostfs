use crate::{DeadObj, HeaderObj, Object, ObjectId, Objects};
use anyhow::{anyhow, Result};
use std::collections::{BTreeSet, VecDeque};
use std::iter;
use tracing::{debug, instrument};

#[derive(Debug)]
pub struct Collector<'a> {
    objects: &'a mut Objects,
}

impl<'a> Collector<'a> {
    pub fn new(objects: &'a mut Objects) -> Self {
        Self { objects }
    }

    #[instrument(skip(self))]
    pub fn run(mut self) -> Result<()> {
        debug!("starting garbage collector");
        debug!("looking for all objects");

        let all_objects = self.objects.len()?;

        debug!("... found {}", all_objects);

        let header = self.objects.get_header()?;
        let known_dead_objects = self.find_known_dead_objects(header)?;
        let alive_objects = self.find_alive_objects(header)?;

        self.collect_objects(header, all_objects, known_dead_objects, alive_objects)?;

        debug!("garbage collection completed");

        Ok(())
    }

    fn find_known_dead_objects(&mut self, header: HeaderObj) -> Result<BTreeSet<ObjectId>> {
        debug!("looking for known-dead objects");

        let mut result = BTreeSet::new();
        let mut cursor = header.dead;

        while let Some(oid) = cursor {
            result.insert(oid);

            cursor = self.objects.get(oid)?.into_dead(oid)?.next;
        }

        debug!("... found {}", result.len());

        Ok(result)
    }

    fn find_alive_objects(&mut self, header: HeaderObj) -> Result<BTreeSet<ObjectId>> {
        debug!("looking for alive objects");

        let mut result = BTreeSet::new();
        let mut pending: VecDeque<_> = iter::once(header.root).chain(header.clone).collect();

        while let Some(oid) = pending.pop_front() {
            result.insert(oid);

            match self.objects.get(oid)? {
                Object::Empty => {
                    return Err(anyhow!(
                        "filesystem seems damaged: {:?} is reachable, but it's empty",
                        oid
                    ));
                }

                Object::Dead(_) => {
                    return Err(anyhow!(
                        "filesystem seems damaged: {:?} is reachable, but it's dead",
                        oid
                    ));
                }

                Object::Header(_) => {
                    return Err(anyhow!(
                        "filesystem seems damaged: found second header object (at {:?})",
                        oid
                    ));
                }

                Object::Clone(obj) => {
                    pending.push_back(obj.name);
                    pending.push_back(obj.root);

                    if let Some(next) = obj.next {
                        pending.push_back(next);
                    }
                }

                Object::Entry(obj) => {
                    pending.push_back(obj.name);

                    if let Some(body) = obj.body {
                        pending.push_back(body);
                    }

                    if let Some(next) = obj.next {
                        pending.push_back(next);
                    }
                }

                Object::Payload(obj) => {
                    if let Some(next) = obj.next {
                        pending.push_back(next);
                    }
                }
            }
        }

        debug!("... found {}", result.len());

        Ok(result)
    }

    fn collect_objects(
        &mut self,
        header: HeaderObj,
        all_objects: u32,
        known_dead_objects: BTreeSet<ObjectId>,
        alive_objects: BTreeSet<ObjectId>,
    ) -> Result<()> {
        let collectible = (1..all_objects).map(ObjectId::new).collect::<BTreeSet<_>>();

        let collectible = collectible
            .difference(&known_dead_objects)
            .copied()
            .collect::<BTreeSet<_>>();

        let collectible = collectible
            .difference(&alive_objects)
            .copied()
            .collect::<BTreeSet<_>>();

        match collectible.len() {
            1 => debug!("got 1 object to collect"),
            n => debug!("got {} objects to collect", n),
        }

        if !collectible.is_empty() {
            let mut head = header.dead;

            for &oid in collectible.iter() {
                self.objects
                    .set(oid, Object::Dead(DeadObj { next: head }))?;

                head = Some(oid);
            }

            self.objects.set_header(HeaderObj {
                dead: head,
                ..header
            })?;
        }

        Ok(())
    }
}
