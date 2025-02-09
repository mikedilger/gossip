# Installation

## Release Builds

- **Linux (Debian)**: See the [releases](https://github.com/mikedilger/gossip/releases) area for a file named something like `gossip-VERSION-ARCH.deb`
- **Linux (AppImage)**: See the [releases](https://github.com/mikedilger/gossip/releases) area for a file named something like `gossip-VERSION.AppImage`
- **Linux (Flatpak)**: See the [releases](https://github.com/mikedilger/gossip/releases) area for a file named something like `gossip-VERSION.flatpak`
  - See [README.flatpak.txt](README.flatpak.txt)
- **Microsoft Windows**: See the [releases](https://github.com/mikedilger/gossip/releases) area for a file named something like `gossip.VERSION.msi`
- **MacOS**: See the [releases](https://github.com/mikedilger/gossip/releases) area for a file named something like `gossip-VERSION-Darwin-arm64.dmg` or `gossip-VERSION-Darwin-x86_64.dmg`
  - See [README.macos.txt](README.macos.txt)

## Package Managers

[![Packaging status](https://repology.org/badge/vertical-allrepos/gossip-nostr.svg)](https://repology.org/project/gossip-nostr/versions)

With `pacman` on **Arch Linux**: [`gossip`](https://aur.archlinux.org/packages/gossip) or [`gossip-git`](https://aur.archlinux.org/packages/gossip-git) or [`gossip-bin`](https://aur.archlinux.org/packages/gossip-bin) on the AUR

With [homebrew](https://brew.sh/) on **MacOS** or **Linux**: `brew install gossip` from [`homebrew-core`](https://github.com/Homebrew/homebrew-core), or for more options `brew install nostorg/nostr/gossip` from [`homebrew-nostr`](https://github.com/nostorg/homebrew-nostr)

With [scoop](https://scoop.sh/) on **Microsoft Windows**: `scoop install extras/gossip` from [scoop extras bucket](https://github.com/ScoopInstaller/Extras).

## Common issues
- **Linux (AppImage)**: commonly used AppImageLauncher doesn't work with this image so far, manual run can't be used while AppImageLauncher installed on machine, proceed with the .deb
- **Linux (Flatpak)**: if used altogether with Flatseal, user must gain privilege for at least a single folder location in order to be able adding an attachment for the note later on. 

## Building from Source

### Step 0 - Possible Reset of Master Branch

If when you pull gossip it doesn't pull cleanly, I may have done a rare force-push. Run these commands to reset your master branch:

````bash
git fetch
git reset --hard origin/master
````

### Step 1 - Install Rust

If you don't already have rust installed, follow the guidance at [rust-lang.org](https://www.rust-lang.org/).

### Step 2 - Install some dependencies

Most dependencies are probably already installed in your base operating system. Here are a few that sometimes aren't:

- build essentials like gcc and make (debian: "build-essential")
- cmake (debian: "cmake")
- pkg-config (debian: "pkg-config")
- openssl (debian: "libssl-dev") (this is only needed if not compiling with feature "rustls-tls")
- fontconfig (debian: "libfontconfig1-dev")
- ffmpeg support (debian: libavutil-dev libavformat-dev libavfilter-dev libavdevice-dev libxext-dev libclang-dev)  (this is only needed if compiling with feature "video-ffmpeg")

#### macOS

a. Install rust with rust-up: <https://rustup.rs/>
b. Install homebrew if you don't have it yet <https://brew.sh/>
c. Install these dependencies:

```bash
brew install cmake sdl2 pkg-config ffmpeg
```

### Step 3 - Clone this Repository

````bash
git clone https://github.com/mikedilger/gossip
````

### Step 4 - Compile

````bash
cd gossip
cargo build --release
````

The output will be a binary executable in `target/release/gossip`

This binary should be portable to similar systems with similar hardware and operating system.

If you want a binary optimized for your exact processor with the newest CPU features enabled, and all gossip features enabled, do something more like this (for exact features to use, see the next section):

````bash
RUSTFLAGS="-C target-cpu=native --cfg tokio_unstable" cargo build --features=lang-cjk,video-ffmpeg --release
````

Everything gossip needs (fonts, icons) is baked into this executable. It doesn't need to find assets. So you can move the "gossip" binary and run it from anywhere.

To make the binary smaller,

````bash
strip gossip
````

### Step 5 - Do it all again

The `master` branch changes quickly.  When you want to update, do it all again, something like this:

````bash
git pull
cargo build --release
strip ./target/release/gossip
./target/release/gossip
````

## Compile Features

### TLS

Gossip has three options for TLS support:

1. Use rust-code and compiled in root certificates from webpki  (feature 'rustls-tls')
2. Use rust-code, but use your system's root certificates (feature 'rustls-tls-native', this is the default)
3. Use your system's code and your system's root certificates (feature 'native-tls')

Rust's TLS code is thought to be more secure than your systems TLS code (e.g. OpenSSL). But it is very finnicky. In particular:

- It will not accept self-signed CA certificates. If you have these on your system, it won't run at all.
- Gossip will fail to negotiate SSL with servers that don't have any strong ciphersuites. This is a feature, but not one that everybody wants.
- Gossip may not compile on hardware that the `ring` crypto library does not yet support.

### Language Support

#### Chinese, Japanese and Korean character sets

Gossip by default does not include the CJK font because it is larger than all other languages put together, and most gossip users don't recognize those characters. If you do recognize such characters, you can compile in that font with:

````
  --features=lang-cjk
````

#### Other Non-Latin languages

There are so many of these (172) that it becomes a real pain to add them all. But if you need one, please ask (open an issue) and I'll add it for you.

### Video Playback

You will need to install sdl2 (follow the instructions in the [readme](https://github.com/Rust-SDL2/rust-sdl2/)) and ffmpeg on your system.

Compile with

````
  --features=video-ffmpeg
````

## Configuration

Now that you have it installed, see [Configuration](CONFIGURATION.md)
