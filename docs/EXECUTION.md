# Execution

## Logs

Many people run programs by double-clicking an icon and interacting only with the GUI window.

But while gossip is a GUI desktop program, it also outputs a lot of messages to the console as it runs. I recommend you keep the console open and pay some attention to errors or other odd behavior that may become apparent from the console messages.

Gossip logs a lot, and important messages can get missed. In order to only see the important messages, you can change the log level. Every log message is tagged by a log level, one of `error`, `warn`, `info`, `debug` or `trace`. You can specify a cut-off to see fewer messages by setting the environment variable `RUST_LOG` to the level you desire, such as `RUST_LOG=warn`.  If not specified otherwise, gossip defaults to the `info` log level.

There are other things you can specify in `RUST_LOG`, such as which level to log on a crate-by-crate basis. For example: `RUST_LOG="warn,gossip=info,gossip_lib=info,nostr_types=info"`

Gossip logs to stderr by default now (this used to be stdout). This helps differentiate log messages from command line outputs.

## Rapid

For some systems with slow storage devices, you can run gossip faster using the `--rapid` command line parameter.  However, a crash can corrupt local data under this mode if your filesystem does not preserve write ordering.

## Backtraces

This is usually not necessary. But if gossip is panicking you can get backtrace information by setting the environment variable `RUST_BACKTRACE=1` when executing gossip.

## Command Line Usage

Gossip has a lot of command line commands for tweaking things or extracting bits of information from its database.  These commands to execute are specified on the command line.

Try `gossip help` and see [Gossip Commands](COMMANDS.md)
