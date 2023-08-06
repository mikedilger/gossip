# Performance

It is possible to operate gossip in a way that causes it to perform poorly, to do lots of disk activity and for the UI to be poorly responsive.  This isn't usually due to bugs, but is due to asking too much of gossip.

## Possible Causes

### Following too many people

Think about it. If you followed a million people and demanded that gossip load all their recent events and all the likes and zaps on those events, it is going to take a long time to do that.

This is also true if you follow 1000 people that post a lot at 4x relay redundancy. You may need to load 20,000 events before gossip settles down.

This issue will be ameliorated somewhat in the future when you can have different feeds each with different groups of people.

### Slow Hardware

*CPU* - If when gossip seems busy and poorly responsive your CPU is at 100%, then you are CPU bound.

*Disks* - If you are running on a physical spinning disk, this will be a lot slower than when running on an SSD. I highly recommend using an SSD. (However, I run gossip on physical disks in order to help me discover and fix performance issues).

### Aggressive Settings

*feed chunk* - Your feed chunk may be too long, meaning gossip is seeking to load too many events.

*replies chunk* - Similar to feed chunk when looking at your inbox

*number of relays per person* - This should be two. Three is much more expensive than two.

*max relays* - I would put this down around 25 or lower if you are having performance problems.

*max fps* - The FPS needs to be no higher than 10. FPS of 7 is reasonable. High fps is very expensive for very little benefit (unless you are not having performance problems and want a super smooth experience).

*recompute feed periodically* - I would turn this off and press refresh manually once gossip has settled down.

*reactions* - Reactions are a lot of events with low value. I would turn off reactions if I had performance problems.

*load media* - You could save some processing by not loading media automatically. You'll have the option to click to load.

### Non-Optimized Compile

Gossip should be compiled in release mode. You can also compile it for your individual processor to squeeze out the most performance (the following line leaves out feature flags, you'll wnat to determine which ones are right for you):

````bash
$ RUSTFLAGS="-C target-cpu=native --cfg tokio_unstable" cargo build --release
````

### Dumb Programmers

Yes, I am recursively pulling my head out of my ass and left to wonder, "is it asses all the way down?"
