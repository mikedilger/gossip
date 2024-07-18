# Gossip Troubleshooting

If your issue is not covered here, please [open an issue on github](https://github.com/mikedilger/gossip/issues).

## DOES NOT RUN

### Startup error about glutin/eframe and window does not show, gossip closes

You may need to swtich backend renderers. Run gossip from the command line like this:

```
$ gossip wgpu_renderer true
```

Then try running gossip again normally.

### On MacOS is says Gossip is damaged and can't be opened

Because I am not an Apple Developer (I didn't like their contract), Apple's protection
mechanisms (Gatekeeper and/or XProtect) may give you errors like this:

    "Gossip" is damaged and can't be opened. You should move it to the Bin.

It is unlikely to be actually damaged. It is just that your computer cannot verify that
it is safe to run. If you trust me then you should do this:

1. Run 'sha256sum' on the .dmg file

   (If that command is not available run "brew install coreutils" first)

2. Compare the output to the published sum.  The sum will be published by the
   official gossip nostr account, as well as in a file on the github release.

   Official gossip nostr account: nprofile1qqsrjerj9rhamu30sjnuudk3zxeh3njl852mssqng7z4up9jfj8yupqprdmhxue69uhkummnw3ezumtfddjkg6tvvajhytnrdakj7ugumjz

   Github releases: https://github.com/mikedilger/gossip/releases

3. If they match, then the file is good. Tell your Mac that it is okay with this command:

   % sudo xattr -rd com.apple.quarantine /Applications/Gossip.app

### Upgrading from very old versions

Some code managing very old data has been removed. This happened twice now. If you are running a very old version (see the table, column 1) you need to upgrade to an old version of gossip (column 2) and run it just once in order to migrate the data because the very newest version of gossip doesn't have the migration code anymore.

| If you are running | You must first install and run once |
|--------------------|-------------------------------------|
|  < 0.8.x           |  0.8.x, then see next line          |
|  < 0.11.x          |  0.9.x -or- 0.10.x                  |

Alternatively, just delete your old gossip directory in your [config dir](https://docs.rs/dirs/latest/dirs/fn.config_dir.html) and start fresh.


## PERFORMANCE

### Gossip runs very slow, and my disk drive is making a lot of noise

Gossip doesn't work well on physical disk drives. It was designed to work well on NVME and SSD
drives.

We may add a feature switch to make gossip run faster on disk drives, at the expense of possible
data corruption. But there is presently no good solution to this.

### Gossip still runs slow on an NVME/SSD drive

### Following too many people

Think about it. If you followed a million people and demanded that gossip load all their recent events and all the likes and zaps on those events, it is going to take a long time to do that.

This is also true if you follow 1000 people that post a lot at 4x relay redundancy. You may need to load 20,000 events before gossip settles down.

This issue will be ameliorated somewhat in the future when you can have different feeds each with different groups of people.

### CPU bound

If when gossip seems busy and poorly responsive, and your CPU is at 100%, then you are CPU bound. Usually it is best to just give it time to settle down. But if it doesn't, please
[open an issue](https://github.com/mikedilger/gossip/issues).

*Disks* - If you are running on a physical spinning disk, this will be a lot slower than when running on an SSD. I highly recommend using an SSD. (However, I run gossip on physical disks in order to help me discover and fix performance issues).

### Avoid Aggressive Settings

*feed chunk* - Your feed chunk may be too long, meaning gossip is seeking to load too many events.

*replies chunk* - Similar to feed chunk when looking at your inbox

*number of relays per person* - This should be two. Three is much more expensive than two.

*max relays* - I would put this down around 25 or lower if you are having performance problems.

*max fps* - The FPS needs to be no higher than 10. FPS of 7 is reasonable. High fps is very expensive for very little benefit (unless you are not having performance problems and want a super smooth experience).

*recompute feed periodically* - I would turn this off and press refresh manually once gossip has settled down.

*reactions* - Reactions are a lot of events with low value. I would turn off reactions if I had performance problems.

*load media* - You could save some processing by not loading media automatically. You'll have the option to click to load.

### Non-Optimized Compile

Gossip should be compiled in release mode. You can also compile it for your individual processor to squeeze out the most performance (the following line leaves out feature flags, you'll want to determine which ones are right for you):

````bash
RUSTFLAGS="-C target-cpu=native --cfg tokio_unstable" cargo build --release
````
