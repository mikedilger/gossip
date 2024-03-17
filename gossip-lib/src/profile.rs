use crate::error::Error;
use std::env;
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::RwLock;

#[cfg(windows)]
use normpath::PathExt;

lazy_static! {
    static ref CURRENT: RwLock<Option<Profile>> = RwLock::new(None);
}

/// Storage paths
#[derive(Clone, Debug, PartialEq)]
pub struct Profile {
    /// The base directory for all gossip data
    pub base_dir: PathBuf,

    /// The directory for cache
    pub cache_dir: PathBuf,

    /// The profile directory (could be the same as the base_dir if default)
    pub profile_dir: PathBuf,

    /// The LMDB directory (within the profile directory)
    pub lmdb_dir: PathBuf,
}

impl Profile {
    fn new() -> Result<Profile, Error> {
        if cfg!(feature = "appimage") {
            // Because AppImage only changes $HOME (and not $XDG_DATA_HOME), we unset
            // $XDG_DATA_HOME and let it use the changed $HOME on linux to find the
            // data directory
            std::env::remove_var("XDG_DATA_HOME");
        }

        // Get system standard directory for user data
        let data_dir = dirs::data_dir()
            .ok_or::<Error>("Cannot find a directory to store application data.".into())?;

        // Canonicalize (follow symlinks, resolve ".." paths)
        let data_dir = normalize(data_dir)?;

        // Push "gossip" to data_dir, or override with GOSSIP_DIR
        let base_dir = match env::var("GOSSIP_DIR") {
            Ok(dir) => {
                tracing::info!("Using GOSSIP_DIR: {}", dir);
                // Note, this must pre-exist
                normalize(dir)?
            }
            Err(_) => {
                let mut base_dir = data_dir;
                base_dir.push("gossip");
                // We canonicalize here because gossip might be a link, but if it
                // doesn't exist yet we have to just go with basedir
                normalize(base_dir.as_path()).unwrap_or(base_dir)
            }
        };

        let cache_dir = {
            let mut cache_dir = base_dir.clone();
            cache_dir.push("cache");
            cache_dir
        };

        // optional profile name, if specified the the user data is stored in a subdirectory
        let profile_dir = match env::var("GOSSIP_PROFILE") {
            Ok(profile) => {
                if "cache".eq_ignore_ascii_case(profile.as_str()) {
                    return Err(Error::from("Profile name 'cache' is reserved."));
                }

                // Check that it doesn't corrupt the expected path
                let mut dir = base_dir.clone();
                dir.push(&profile);
                match dir.file_name() {
                    Some(filename) => {
                        if filename != OsStr::new(&profile) {
                            return Err(Error::from(format!(
                                "Profile is not a simple filename: {}",
                                profile
                            )));
                        }
                    }
                    None => {
                        return Err(Error::from(format!("Profile is invalid: {}", profile)));
                    }
                };

                dir
            }
            Err(_) => base_dir.clone(),
        };

        let lmdb_dir = {
            let mut lmdb_dir = profile_dir.clone();
            lmdb_dir.push("lmdb");

            // Windows syntax not compatible with lmdb:
            if lmdb_dir.starts_with(r"\\?\") {
                lmdb_dir = lmdb_dir.strip_prefix(r"\\?\").unwrap().to_path_buf();
            }

            lmdb_dir
        };

        // Create all these directories if missing
        fs::create_dir_all(&base_dir)?;
        fs::create_dir_all(&cache_dir)?;
        fs::create_dir_all(&profile_dir)?;
        fs::create_dir_all(&lmdb_dir)?;

        Ok(Profile {
            base_dir,
            profile_dir,
            cache_dir,
            lmdb_dir,
        })
    }

    pub fn current() -> Result<Profile, Error> {
        {
            // create a new scope to drop the read lock before we try to create a new profile if it doesn't exist
            let current = CURRENT.read().unwrap();
            if current.is_some() {
                return Ok(current.as_ref().unwrap().clone());
            }
        }
        let created = Profile::new()?;
        let mut w = CURRENT.write().unwrap();
        *w = Some(created.clone());
        Ok(created)
    }
}

#[cfg(not(windows))]
fn normalize<P: AsRef<Path>>(path: P) -> Result<PathBuf, Error> {
    Ok(fs::canonicalize(path)?)
}

#[cfg(windows)]
fn normalize<P: AsRef<Path>>(path: P) -> Result<PathBuf, Error> {
    Ok(path.as_ref().normalize()?.into_path_buf())
}
