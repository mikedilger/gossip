
# Developing

This app is developed using the following tools:

* [Vite](https://vitejs.dev/) - Dev server, compiling and reloading
* [Yarn](https://yarnpkg.com/) - JavaScript package manager
* [VueJS](https://vuejs.org/) 3.x - JavasScript reactive component framework
* [Typescript](https//typescriptlang.org/) - Javascript language with strong typing
* [Rust](https://rust-lang.org/) - Safe system-level language
* [Cargo](https://crates.io/) - Rust package manager
* [Tauri](https://tauri.app/) - Desktop webview based app development and runtime system

## Setup your Build Environment

### ArchLinux

````sh
$ sudo pacman -Syu
$ sudo pacman -S --needed \
       webkit2gtk \
       base-devel \
       curl \
       wget \
       openssl \
       appmenu-gtk-module \
       gtk3 \
       libappindicator-gtk3 \
       librsvg \
       libvips \
       yarn
````

If you do not have rust installed yet:

````sh
$ curl --proto '=https' --tlsv1.2 https://sh.rustup.rs -sSf | sh
````

In any case

````sh
$ rustup update
$ cargo install tauri-cli
````
