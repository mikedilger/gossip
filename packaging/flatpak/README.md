# Flatpak

- Install flatpak-builder with your package manager (or from flathub and declare `alias flatpak-builder="flatpak run org.flatpak.Builder"`)
- Install SDKs: `flatpak install flathub org.freedesktop.Sdk//23.08 org.freedesktop.Platform//23.08 org.freedesktop.Sdk.Extension.rust-stable//23.08`
- Make sure to initialize the flatpak-builder-tools git submodule: `git submodule update --init`

```sh
./build_flatpak.sh # see ./gossip.flatpak
```
