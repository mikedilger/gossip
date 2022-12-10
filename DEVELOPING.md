
# Developing

This app is developed using the following tools:

* [Vite](https://vitejs.dev/) - Dev server, compiling and reloading
* [Yarn](https://yarnpkg.com/) - JavaScript package manager
* [VueJS](https://vuejs.org/) 3.x - JavasScript reactive component framework
* [Typescript](https//typescriptlang.org/) - Javascript language with strong typing
* [Rust](https://rust-lang.org/) - Safe system-level language
* [Cargo](https://crates.io/) - Rust package manager
* [Tauri](https://tauri.app/) - Desktop webview based app development and runtime system

---

## Setup your Build Environment

### System Packages

Everything in this section could also be installed under your user account in your home directory. But typically the things in this section are installed globally on your computer, so that is how our instructions read.

#### ArchLinux

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
        libvips
````

(We may have left off npm and nodejs, I can't check right now my archlinux system
has a hardware failure.)

#### Debian

````sh
$ sudo apt install \
        libwebkit2gtk-4.0-dev \
        build-essential \
        curl \
        wget \
        libssl-dev \
        libgtk-3-dev \
        libayatana-appindicator3-dev \
        librsvg2-dev
````

### Local Installations

#### Node

Outside of ArchLinux and other bleeding-edge linuxes, you'll probably need to manage nodejs and related packages under your user account inside of your home directory. We presume this in this section.

Get node version manager https://github.com/nvm-sh/nvm.  The following may be out of date:

````
$ curl -o- https://raw.githubusercontent.com/nvm-sh/nvm/v0.39.2/install.sh | bash
````

After restarting your shell,

```
$ nvm install 18
$ nvm use 18
```

#### Rust

If you do not have rust installed yet:

````sh
$ curl --proto '=https' --tlsv1.2 https://sh.rustup.rs -sSf | sh
````

In any casev

````sh
$ rustup update
$ cargo install tauri-cli
````

Minimum rust version is 1.64 (for the process_set_process_group feature).

#### Yarn

Install yarn (1.x)

````
$ npm install yarn
````

## Clone and Prepare

You are reading this so you may have already cloned.
Otherwise:

````
$ git clone https://github.com/mikedilger/gossip
$ cd gossip
````

### Project package installation

````
$ yarn install
````

---

## Develop

Start up vue-devtools before the app

````sh
$ yarn run vue-devtools
````

Run dev environment

````sh
$ cargo tauri dev
````

Build just the rust part without running

````sh
$ (cd src-tauri && cargo build)
````

Building and packaging full output

````sh
cargo tauri build
````

Building and packaging full output with debugging

````sh
cargo tauri build --debug
````

Output is in `src-tauri/target/debug/gossip`

---

## Upgrading dependencies

### Yarn

````sh
$ yarn upgrade @tauri-apps/cli @tauri-apps/api --latest
````

### cargo

````sh
$ cargo update
````

---

## How the app was initialized (no need to do this again)

Approximately like this (with config file edits in-between)

````sh
$ yarn create vite
    Project Name: gossip
    Select a framework: Vue
    Select a variant: TypeScript
$ cd gossip
$ yarn add -D @tauri-apps/cli
$ yarn add @tauri-apps/api
$ cargo tauri init
    App name: gossip
    Window title: Gossip
    Assets loc: ../dist
    Dev server: http://localhost:5173/
    Frontend dev command: yarn run dev
    Fronend build command: yarn run build
$ cargo tauri dev
````
