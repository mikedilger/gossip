# Pruning

Your database will continue to grow.

## Cleaning LMDB cruft

Periodically you can compact the LMDB waste using `mdb_copy -c`.  This is done automatically
on startup from time to time, but you can do it manually.

1. First get into your gossip directory, e.g. on linux `cd ~/.local/share/gossip`
2. Make a lmdb2 directory: `mkdir lmdb2`
3. Copy: `mdb_copy -c lmdb lmdb2`
4. Switch to the copy: `mv lmdb lmdb.old; mv lmdb2 lmdb; rm -rf ./lmdb.old`

## Removing unused person records

This takes a very long time and only shrinks the database by a small amount.

`gossip prune_unused_people`

You can break out and start again at any time. It will quickly get back to where
it left off.

You can clean LMDB cruft afterwards.

## Removing old events

Old events can be pruned. We don't delete your events no matter how old they are, and
we also preserve entire threads that you participated in.

1. Fire up gossip and adjust the pruning time in Settings > Storage
2. Exit gossip
3. Run `gossip prune_old_events`

You can clean LMDB cruft afterwards.

## Reindexing

After doing prunes, you should rebuild indexes because the prunes do not clean out
the indexes.

`gossip rebuild_indices`

You can clean LMDB cruft afterwards.
