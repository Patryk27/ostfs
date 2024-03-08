use crate::{DeadObj, HeaderObj, Object, ObjectId, Objects, Storage};
use anyhow::{anyhow, Result};
use std::collections::{BTreeSet, VecDeque};
use std::iter;
use std::path::PathBuf;
use structopt::StructOpt;

#[derive(Debug, StructOpt)]
pub struct CollectCmd {
    /// Path to the *.ofs file
    src: PathBuf,
}

impl CollectCmd {
    pub fn run(self) -> Result<()> {
        let mut storage = Storage::open(&self.src, true)?;
        let total_objects = storage.len()? as usize;
        let mut objects = Objects::new(storage);
        let header = objects.get_header()?;

        println!("found {} objects", total_objects);
        println!();

        let known_dead_objects = find_known_dead_objects(&mut objects, header)?;
        let alive_objects = find_alive_objects(&mut objects, header)?;

        collect_objects(
            &mut objects,
            header,
            total_objects,
            known_dead_objects,
            alive_objects,
        )?;

        println!("ok");

        Ok(())
    }
}

fn find_known_dead_objects(objects: &mut Objects, header: HeaderObj) -> Result<BTreeSet<ObjectId>> {
    println!("looking for known-dead objects");

    let mut result = BTreeSet::new();
    let mut cursor = header.dead;

    while let Some(oid) = cursor {
        result.insert(oid);

        cursor = objects.get(oid)?.into_dead(oid)?.next;
    }

    println!("... found {}", result.len());
    println!();

    Ok(result)
}

fn find_alive_objects(objects: &mut Objects, header: HeaderObj) -> Result<BTreeSet<ObjectId>> {
    println!("looking for alive objects");

    let mut result = BTreeSet::new();
    let mut pending: VecDeque<_> = iter::once(header.root).chain(header.clone).collect();

    while let Some(oid) = pending.pop_front() {
        result.insert(oid);

        match objects.get(oid)? {
            Object::Empty => {
                return Err(anyhow!("{:?} is reachable, but it's empty", oid));
            }

            Object::Dead(_) => {
                return Err(anyhow!("{:?} is reachable, but it's dead", oid));
            }

            Object::Header(_) => {
                return Err(anyhow!(
                    "filesystem contains more than one header (another one found at {:?})",
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

    println!("... found {}", result.len());
    println!();

    Ok(result)
}

fn collect_objects(
    objects: &mut Objects,
    header: HeaderObj,
    total_objects: usize,
    known_dead_objects: BTreeSet<ObjectId>,
    alive_objects: BTreeSet<ObjectId>,
) -> Result<()> {
    let collectible = (1..total_objects)
        .map(|oid| ObjectId::new(oid as u32))
        .collect::<BTreeSet<_>>();

    let collectible = collectible
        .difference(&known_dead_objects)
        .copied()
        .collect::<BTreeSet<_>>();

    let collectible = collectible
        .difference(&alive_objects)
        .copied()
        .collect::<BTreeSet<_>>();

    match collectible.len() {
        1 => println!("collecting 1 object"),
        n => println!("collecting {} objects", n),
    }

    if !collectible.is_empty() {
        let mut head = header.dead;

        for &oid in collectible.iter() {
            objects.set(oid, Object::Dead(DeadObj { next: head }))?;

            head = Some(oid);
        }

        objects.set_header(HeaderObj {
            dead: head,
            ..header
        })?;
    }

    println!();

    Ok(())
}
