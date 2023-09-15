
AppImage:

  Do not do this (it is broken):
  (cargo-appimage 2.0.1 has a severe bug)
    $ cargo install cargo-appimage

  Instead do this:
    $ git clone https://github.com/mikedilger/cargo-appimage
    $ cd cargo-appimage
    $ cargo install --path .

  Then use it in gossip:
    $ cargo appimage --features="lang-cjk,video-ffmpeg"

