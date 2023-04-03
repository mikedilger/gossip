#[derive(Debug)]
pub struct About {
    pub name: String,
    pub version: String,
    pub description: String,
    pub authors: String,
    pub repository: String,
    pub homepage: String,
    pub license: String,
    pub storage_path: String,
}

pub fn about() -> About {
    let data_dir = match dirs::data_dir() {
        Some(mut d) => {
            d.push("gossip");
            format!("{}/", d.display())
        }
        None => "Cannot find a directory to store application data.".to_owned(),
    };

    let mut version = env!("CARGO_PKG_VERSION").to_string();
    if version.contains("unstable") {
        let git_hash_prefix: String = env!("GIT_HASH").chars().take(8).collect();
        version = format!("{}-{}", version, git_hash_prefix);
    }

    About {
        name: env!("CARGO_PKG_NAME").to_string(),
        version,
        description: env!("CARGO_PKG_DESCRIPTION").to_string(),
        authors: env!("CARGO_PKG_AUTHORS").to_string(),
        repository: env!("CARGO_PKG_REPOSITORY").to_string(),
        homepage: env!("CARGO_PKG_HOMEPAGE").to_string(),
        license: env!("CARGO_PKG_LICENSE").to_string(),
        storage_path: data_dir,
    }
}
