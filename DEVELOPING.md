# Developing

Gossip is architected with the following components:

- A User Interface thread, synchronous
- Tokio asynchronous runtime running
    - An overlord (handles most jobs)
    - A set of minions (each one handles one relay)

## Keeping the UI responsive

The most important thing to be aware of is that the User Interface thread repeatedly calculates what to draw and potentially redraws up to 60 frames per second, therefore it **must** not run any slow code.

To that end, the following are allowed from the UI thread:
- Locking global variables (since nearly all locks in gossip are intended to be rapidly released)
- Sending messages to the overlord.

The following is NOT appreciated when done from the UI thread:
- Database calls, or calls to functions that do database calls
- Internet queries, or calls to functions that query over the Internet

Generally when you need to do something that takes a while, ask the overlord to do it for you.

You also must make sure that if you acquire a lock on a global in any code (UI or not), you **must release the lock very rapidly**. Do not hold such locks while doing database calls, and definitely do not hold them while waiting for the network. You may need to copy data to achieve this.

## Communication

Anyone can send a message to the Overlord using the GLOBALS.to_overlord channel.

The overlord generally is the one to send messages to minions using the GLOBALS.to_minions channel, but there may be cases for other components to talk directly to minions as well.

## Flow

The flow generally happens like this
- The user interacts with the UI
- The UI requests something of the Overlord
- The overlord either does it, or spawns a task to do it if it takes too long (the overlord should also remain somewhat responsive).
- Sometimes the overlord has to start minions to handle it
- Sometimes the overlord contacts one or more minions
- The minions then updates filters on relays
- When events come in fulfilling those filters, they are sent to crate::process::process_new_event()
- crate::process handles the new event regardless of how it got there - generally it is unaware of the sequence of events that happened in the previous steps of this list
- the result of such processing updates global data
- the UI on the next frame reads the global data (which is now different) and renders accordingly.
