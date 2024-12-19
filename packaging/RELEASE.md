# RELEASE

## Phase 1 - Test and stabilize the codebase

### Don't update dependencies

Don't do a `cargo update`. Too risky right before a release.
Do that kind of stuff right *after* releasing.

### Test it on Windows and MacOS.

Yes, actually do this. We often find problems.

### Test it with a new user on Linux, Windows and MacOS.

Many problems come up for new users that we normally never see.

### fmt, clippy, test

```bash
cargo clippy
cargo test
cargo fmt
```

### Update recommended relays

This can be a long external process.

### Update documentation

Update the following

- README.md
- LICENSE.txt may need a copyright range update
- Help page in the UI

### Pre-update the packaging files

- packaging/debian/Dockerfile may need a new rust version
- packaging/windows/gossip.VERSION.wxs will need 3 edits
    - Update the Package.Version near the top
    - Update the SummaryInformation.Description near the bottom
    - Update the Package.ProductCode GUID near the top to a new random one

## Phase 2 - Release

- Create a new release branch (if not a point release)
- Update */Cargo.toml to change the version number
- COMPILE! at least once to get a new Cargo.lock
- Commit the three changed Cargo files with the commit description as the release number, e.g. "0.11.1"
- Tag the relase with a 'v' prefix, e.g. 'v0.11.1'
- Push the branch and the tag to github:

```bash
git push github
git push --tags github
```

## Phase 3 - Build the packages

### Debian

```bash
cd debian
./deb.sh
```

### AppImage

```bash
cd appimage
./build-appimage.sh
```

### Flatpak

- Follow the [Flatpak README](flatpak/README.md)

### Windows

- Follow the [Windows README](windows/README.md)

### MacOS

```bash
cd macos
./build_macos.sh
./build_macos_intel.sh
```

## Phase 4 - Describe the release

Review the changes from last time, and create a summary description in a README.txt file
and put that into a packaging directory.

## Phase 5 - Bundle

### Copy binaries into the release directory:

You should have six binary artifacts from the build phase, like this:

- gossip-$VERSION-1_amd64.deb
- gossip-$VERSION-Darwin-arm64.dmg
- gossip-$VERSION-Darwin-x86_64.dmg
- gossip-$VERSION.flatpak
- gossip-$VERSION.msi
- gossip-$VERSION-x86_64.AppImage.tar.gz

Copy these into the release directory.

### Copy these into the release directory:

- gossip/filter.rhai.example
- gossip/LICENSE.txt
- gossip/packaging/
- gossip/docs/README.flatpak.txt
- gossip/docs/README.macos.txt

### Create a changelog and put into the release directory

Substituting for $PREV, $CURRENT and $PACKAGINGDIR:

```bash
    git log --reverse --oneline v$PREV..v$CURRENT > $PACKAGINGDIR/changelog-$CURRENT.txt
```

### Create a file with the SHA hashes

```bash
SHA256sum * > ./SHA256sums.txt
```

Create an announcement nostr event from the gossip account.

Store it in the release files.

Refer to https://github.com/vicariousdrama/nostrcheck as a tool people could use
to verify it.

## Phase 6 - Publish

On GitHub, make a new release.

Use the git tag of the release

Drag all the files into the release

In the release description, copy the contents of README.txt

Publish as the latest release

## Phase 7 - Update Archlinux User Repository

- Update the AUR packages

## Phase 8 - Announce on NOSTR

- Announce release on nostr under gossip account (using the event created earlier)
- Repost as Mike Dilger
