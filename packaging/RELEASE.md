# RELEASE

0. DON'T update dependencies. DON'T 'cargo update'.  Do that kind of stuff right after
   releasing. Because that stuff presents risk.

1. Update the documentation including:

   README.md
   help page in the UI

2. Stabilize the code. Make all these happy:

   ````bash
      cargo clippy
      cargo fmt
      cargo test
   ````

3. Edit Cargo.toml and change the version (remove the -unstable).
   Compile so you get a new Cargo.lock

   Commit these 2 new Cargo files as a commit named after the version.

4. Tag this as vVERSION, and push the tag

4b. Tag again with -unstable,
    build to get Cargo.lock,
    commit both as next commit,
    push,
    checkout the release commit again for the rest.

5. Build the debian:

   ````bash
      cd debian
      ./deb.sh
   ````

6. Build the appimage:

   ````bash
      cd appimage
      cargo appimage --features="lang-cjk,video-ffmpeg"
   ````

7. Build the windows:

   ````bash
      cd windows
   ````

   and follow the [Windows README](windows/README.md)

8. Build the macos:

   ````bash
      cd macos
      ./build_macos.sh
      ./build_macos_intel.sh
   ````

9. Bundle the files, create SHA256 hashes

  files in files/

  create changelog.txt like this:

    git log --oneline v0.8.2..v0.9.0 > changelog.txt

10. Upload release to github

11. Update the AUR packages

12. Announce release on nostr under gossip account

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
