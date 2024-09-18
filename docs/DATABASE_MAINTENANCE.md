# Database Maintenance

Gossip does not automatically keep things tidy. Now and then it requires the user to clean up.


## Rebuild Indices

Theoretically you should never have to run this. But due to bugs and changes, now and then it might
do you some good.  When I say "now and then" I mean maybe once every 6 months.

```rust
```

## Reprocess Relay Lists

## Deleting spam

## Pruning useless records

## Pruning old records

## LMDB compression

```
mkdir lmdb2
mdb_copy -c lmdb lmdb2
mv lmdb lmdb.old
mv lmdb2 lmdb
rm -r lmdb.old
```
