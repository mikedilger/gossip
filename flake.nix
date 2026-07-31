# nix develop
# cargo build/run
{
  description = "Rust Development Shell";

  inputs = {
    nixpkgs.url      = "github:NixOS/nixpkgs/nixos-unstable";
    rust-overlay.url = "github:oxalica/rust-overlay";
    flake-utils.url  = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, rust-overlay, flake-utils, ... }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs {
          inherit system overlays;
        };
      in
      with pkgs;
      {
        devShells.default = mkShell rec {
          buildInputs = [
            openssl
            pkg-config
	    libxkbcommon
	    libGL
	    wayland
            (
              rust-bin.selectLatestNightlyWith (toolchain: toolchain.default.override {
                extensions = [
                  "rust-src"
                  "rust-analyzer"
                ];
              })
            )
          ];
	  LD_LIBRARY_PATH = "${lib.makeLibraryPath buildInputs}";
          RUST_BACKTRACE="full";
        };
      }
    );
}
