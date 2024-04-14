# RELEASE

0. DON'T update dependencies ('cargo update'). Do that kind of stuff right after releasing. Because that stuff presents risk.

1. Stabilize the code. Make all these happy:

   ````bash
      cargo clippy
      cargo fmt
      cargo test
   ````

1. Test it on Windows and MacOS. Then repeat at step 1 if fixes are needed.

1. Update the documentation including:

    - README.md
    - LICENSE.txt may need a copyright range update
    - Help page in the UI

1. Update packaging files:

    - packaging/debian/Dockerfile may need a new rust version
    - packaging/windows/gossip.VERSION.wxs needs creating and a UUID update

1. Edit Cargo.toml and change the version (remove the -unstable).
    Compile so you get a new Cargo.lock

    - Commit these 2 new Cargo files as a commit named after the version.

1. Tag this as vVERSION, and push the tag

1. Tag again with -unstable

    - build to get Cargo.lock,
    - commit both as next commit,
    - push,
    - checkout the release commit again for the rest.

1. Build the debian:

   ````bash
      cd debian
      ./deb.sh
   ````

1. Build the appimage:

   ````bash
      cd appimage
      ./build-appimage.sh
   ````

1. Build the flatpak:

    - Folllow the [Flatpak README](flatpak/README.md)

1. Build the windows:

    - Follow the [Windows README](windows/README.md)

1. Build the macos:

   ````bash
      cd macos
      ./build_macos.sh
      ./build_macos_intel.sh
   ````

1. Bundle the files, create SHA256 hashes

  files in files/

  create changelog.txt like this:

    git log --oneline v0.8.2..v0.9.0 > changelog.txt

1. Upload release to github

1. Update the AUR packages

1. Announce release on nostr under gossip account

-----------------

This is a draft of the steps taken to make a release.
I intend to flesh this out as I actually make releases.

Update crates from the bottom up:

gossip
├── eframe
│   ├── egui
│   ├── egui-winit
│   └── egui_glow
├── egui-winit
├── egui_extras
├── gossip-relay-picker
│   └── nostr-types
│       └-- speedy
├── nostr-types
└── qrcode

Try to push our dependency changes upstream:
  <https://github.com/mikedilger/qrcode-rust>  (unlikely, stale for >3 years)
  <https://github.com/mikedilger/egui>

nostr-types
   -- cargo update, and check for new versions, maybe update dependencies
   -- cargo test
   -- cargo clippy; cargo fmt
   -- FORK 0.N:
      -- all deps switch to released versions
      -- version 0.N
      -- package/publish
      -- version to 0.N.1-unstable
   -- master:
      -- version to 0.N+1.0-unstable

gossip-relay-picker
   -- cargo update, and check for new versions, maybe update dependencies
   -- cargo test
   -- cargo clippy; cargo fmt
   -- FORK 0.N:
      -- all deps switch to released versions
      -- version 0.N
      -- package/publish
      -- version to 0.N.1-unstable
   -- master:
      -- version to 0.N+1.0-unstable

gossip
   -- cargo update, and check for new versions, maybe update dependencies
   -- cargo test
   -- cargo clippy; cargo fmt
   -- FORK 0.N:
      -- all deps switch to released versions
      -- verison 0.N
      -- package/publish (see below)
      -- version 0.N.1-unstable
   -- master
      -- version 0.N+1.0-unstable

-----------------

Package & Publish of gossip:

Package for windows:

* main version, as .msi
* main version with lang-cjk, as .msi

Package for debian:

* main version, as .msi
* main version with lang-cjk, as .msi

Create github release (it will create source tar files)

* Post the windows .msi files
* Post the debian .deb files

Update aur.archlinux.org PKGBUILD
