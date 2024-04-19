# Flatpak

- Install system packages: flatpak, flatpak-builder, python-toml, python-aiohttp
- Install SDKs: `flatpak install flathub org.freedesktop.Sdk//23.08 org.freedesktop.Platform//23.08 org.freedesktop.Sdk.Extension.rust-stable//23.08`
- You may need to uncomment some lines in build_flatpak.sh which modify your global git config.
  Mike Dilger didn't need to, but Solomon Victorino did.

```sh
./build_flatpak.sh # see ./gossip.flatpak
```
