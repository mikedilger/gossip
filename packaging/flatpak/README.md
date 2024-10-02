# Flatpak

- Install system packages: flatpak, flatpak-builder, python-toml, python-aiohttp
- Install SDKs: `flatpak install flathub org.freedesktop.Sdk//24.08 org.freedesktop.Platform//24.08 org.freedesktop.Sdk.Extension.rust-stable//24.08`
- You may need to uncomment some lines in build_flatpak.sh which modify your global git config.
  Mike Dilger didn't need to, but Solomon Victorino did.
- You may need to clear stale data from ~/.cache/flatpak and ~/.cache/flatpak-cargo
- You may need to clear stale data from everything in the .gitignore file
- You may need to update rustup with a version-specific toolchain (not just "stable")

```sh
./build_flatpak.sh # see ./gossip.flatpak
```
