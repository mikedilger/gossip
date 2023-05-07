use std::process::Command;
fn main() {
    let output = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .output()
        .unwrap();
    let git_hash = String::from_utf8(output.stdout).unwrap();
    println!("cargo:rustc-env=GIT_HASH={git_hash}");

    // link to bundled libraries
    #[cfg(target_os="macos")]
    println!("cargo:rustc-link-arg=-Wl,-rpath,@loader_path");

    #[cfg(target_os="linux")]
    println!("cargo:rustc-link-arg=-Wl,-rpath,$ORIGIN");
}
