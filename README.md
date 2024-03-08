# OstFS 🧀

[OstFS](https://sv.wiktionary.org/wiki/ost#Svenska) is a toy FUSE filesystem
with support for zero-cost¹ snapshots and clones (think ZFS / Btrfs):

``` shell
# (example requires `rustup`, since OstFS is a Rust application)

# Create a file for storing the filesystem:
$ ./ost create tank.ofs

# Mount it:
$ mkdir mnt
$ ./ost mount tank.ofs mnt &

# (`&` causes the process to run in background - press
#  the enter key a few times to bring back your shell
#  prompt)

# Create a file *inside* the filesystem:
$ echo 'Hello, World!' > mnt/hello.txt

# Make a snapshot (aka a read-only clone):
$ ./ost clone create tank.ofs init --mode ro

# </>
# What's cool is that creating a clone doesn't actually
# copy the underlying data - rather, OstFS tracks which
# files have changed *since* the clone and stores only
# the diff (with a pinch of salt).
#
# That is to say, creating a clone is aaalmost¹ a no-op.
# </>

# Modify the file:
$ echo 'nobody expects the spanish inquisition' > mnt/hello.txt

# ---
# Alright, now we can utilize the snapshot to time-travel!
# ---

# Kill the mount:
# (this might be a bash/zsh specific thing - if it doesn't work
#  in your shell, you can run `fg` and press Ctrl+C)
$ kill %1
$ umount mnt

# Invoke `./ofs mount` again, but instructing it to mount
# the snapshot instead of the top filesystem:
$ ./ost mount tank.ofs mnt --clone init &

# Ha, ha!
$ cat mnt/hello.txt
Hello, World!

# Note that mounting a clone doesn't rollback the changes -
# if you remounted without `--clone init`, `cat mnt/hello.txt`
# would say `nobody expects ...` again.
```

¹ terms and conditions apply, see below

## Usage

See the command line:

``` shell
$ ./ost
```

... but it mostly boils down to `./ost create` & `./ost mount`.

### Quirks

An important OstFS-thingie is that it uses garbage collector to remove stale
data, but it doesn't _start_ the collector automatically - every now and then
it's a good idea to unmount the system and run:

``` shell
$ ./ost collect tank.ofs
```

This is required because all actions made on the filesystem (not only removals,
but also changes, renames etc.) _append_ data into the backing file - imagine it 
containing a journal like `file xyz got moved`, `file abc got changed`, where
without running the garbage collector, those events would never get pruned and
even if the only thing you do is `mv`, the file would continue to grow.

(in reality OstFS utilizes a graph instead of being event based, but events are
a good enough approximation to the problem here)

## Architecture

OstFS stores everything in a tree structure which starts from the header and
then dispatches into clones, the root directory and its children.

Something that `ls` displays as:

```
drwxrwxrwx   1 PWY  staff    0 Jan  1  1970 .
drwxr-xr-x  15 PWY  staff  480 Mar  8 20:49 ..
-rw-r--r--   1 PWY  staff   12 Jan  1  1970 one.txt
-rw-r--r--   1 PWY  staff   12 Jan  1  1970 two.txt
```

... would internally be stored as a couple of objects forming a tree:

```
       / ------ \
       | header |
       \ ------ /
            |
            |
        has child
            |
            |
            v
        / ----- \                / ---------- \
        | entry | -- has name -> | payload(/) |  # root note starts at `/`
        \ ----- /                \ ---------- /  # (just a convention)
            |
            |
        has child
            |
            |
            v
        / ----- \                / ---------------- \
        |       | -- has name -> | payload(one.txt) |
        |       |                \ ---------------- /
        | entry |
        |       |                / -------------------- \
        |       | -- has body -> | payload(Hello, One!) |
        \ ----- /                \ -------------------- /
            |
            |
       has sibling
            |
            |
            v
{ similar stuff for two.txt }
```

There are three basic types of objects:

- header (represents the entrypoint - links to the root entry, aka root
  directory),
- entry (represents either a file or a directory),
- payload (represents a string (e.g. entry name) or binary data (e.g. file
  content)).

Each object is assigned a unique identifier (starting from zero and going up)
and each object _links_ to other objects using their identifiers as well - this
can be observed through the `./ofs inspect` command:

```
[0] = Header(HeaderObj { root: ObjectId(29), clone: None, dead: None })
[1] = Entry(EntryObj { name: ObjectId(2), body: None, next: None, kind: Directory, size: 0, mode: 511, uid: 502, gid: 20 })
[2] = Payload(Payload { size: 1, next: None, data: "/" })
[3] = Payload(Payload { size: 7, next: None, data: "one.txt" })
/* ... */
```

Now, all this keeping track of the objects, their edges and so on is a huge
amount of work, so it's reasonable to ask: what do we gain?

Well, we can become **copy on write**!

That is, when you modify a file (change its attributes, content, rename it
etc.), its original object remains intact - rather, OstFS duplicates that object
(with the duplicate having a new identifier), then duplicates its parent,
grandparent etc., up until and including the root.

But we don't duplicate everything - if only the file name has changed, there's
no point in duplicating payload containing the file's body, so if we did:

``` shell
$ mv one.txt one.md
```

... the graph would say:

```
 /* ... */

        / ----- \                / ---------------- \
        |       | -- has name -> | payload(one.txt) |
        |       |                \ ---------------- /
        | entry |
        |       |                / -------------------- \
        |       | -- has body -> | payload(Hello, One!) |
        \ ----- /                \ -------------------- /
                                                      |
        / ----- \                / --------------- \  |
        |       | -- has name -> | payload(one.md) |  |
        |       |                \ --------------- /  |
        | entry |                                     |
        |       |                                     |
        |       | -- has body ----------------------- /
        \ ----- /

 /* ... */

- payload(one.txt) has one parent
- payload(one.md) has one parent
- payload(Hello, One!) has two parents
```

Similarly, given a structure like:

``` shell
     a
   /   \
  b     c
 / \   / \
d   e f   g
```

... if we modified `e`, we'd only need to duplicate `b` and `a` - the rest could
be linked as-is.

This is what makes (almost) zero-cost snapshots (almost) zero-cost - because we
don't modify objects in-place, we can reuse this fact to time-travel back to the
past, if only we can get our hands on the past object ids (and we don't remove
those past objects, of course).

What's more, this also allows for perfectly safe **atomic updates**! -- remember
the header object?

Header is always located at the object slot 0 (i.e. the beginning of the file)
and the most important information it contains is the reference (object id) of
the root directory. Initially the root directory starts at slot 1 (right after
the header), but as soon as the filesystem gets modified, we generate a _new_
root directory (with a brand new object id), which requires updating the header.

Here's the second greatest part of using copy on writes:

If the power goes down when we're building the new tree, nothing gets
accidentally removed/updated! (that is, it's not possible to observe a partial
update)

See, since we update the header at the very end of the process (after we've
built the entire tree), the only two possible options are:

- power went down _before_ we've manged to write the header, in which case upon
  the next mount the filesystem will effectively rollback to its previous state
  (because we didn't overwrite the older objects),

- power went down after we've managed to write the header, in which case we're
  100% sure the _entire_ new tree is in place, because we update the header
  last.

(well, it's technically also possible for the power to go down during the header
update and that's why ZFS uses the concept of uberblock, but let's not go _that_
deep!)

If you're into that, you might find this interesting:

<https://www.youtube.com/watch?v=NRoUC9P1PmA>

## Limitations

- OstFS uses 32-byte objects, which makes it easy to understand, but also 
  impractical (doesn't match any typical sector size)
- OstFS uses linked lists instead of b-treemaps & hashmaps, so a directory with
  10k entries will open noticably slower than a directory with 10 entries
- OstFS keeps inodes entirely in memory
- OstFS doesn't store checksums
- OstFS doesn't store atime, ctime and a couple of other properties
- OstFS requires running garbage collector by hand
- OstFS has no caching layer
- **OstFS is a toy** - I made it solely to learn a few cool concepts; if you're
  looking for actual filesystem, ZFS is your friend!

(most - if not all - of this things could get improved -- it's just that I just
wanted to hack something over three days, not thirty years)

## Hacking

Code is a bit crude at the moment, with no tests and no comments - if you'd like
to take a look anyway, here's a couple of entrypoints:

- `object.rs` 
- `objects.rs`
- `filesystem.rs` (in particular the modules inside of it)

## License

MIT License

Copyright (c) 2024 Patryk Wychowaniec
