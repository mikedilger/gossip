use crate::error::Error;
use std::env;
use std::ffi::OsStr;
use std::fs;
use std::path::PathBuf;
use std::sync::RwLock;

lazy_static! {
    static ref CURRENT: RwLock<Option<Profile>> = RwLock::new(None);
}

///
/// Access to current directories
///
#[derive(Clone, Debug, PartialEq)]
pub struct Profile {
    /// The base directory for all gossip data, may be the same as profile_dir if it's a default profile
    pub base_dir: PathBuf,
    /// The directory for current profile
    pub profile_dir: PathBuf,
    /// The directory for cache
    pub cache_dir: PathBuf,
}

impl Profile {
    fn new() -> Result<Profile, Error> {
        // By default it's what `dirs::data_dir()` gives, but we allow overriding the base directory via env vars
        let base_dir = match env::var("GOSSIP_DIR") {
            Ok(dir) => {
                tracing::info!("Using GOSSIP_DIR: {}", dir);
                PathBuf::from(dir)
            }
            Err(_) => {
                let mut base_dir = dirs::data_dir()
                    .ok_or::<Error>("Cannot find a directory to store application data.".into())?;
                base_dir.push("gossip");
                base_dir
            }
        };

        // optional profile name, if specified the the user data is stored in a subdirectory
        let profile_dir = match env::var("GOSSIP_PROFILE") {
            Ok(profile) => {
                if "cache".eq_ignore_ascii_case(profile.as_str()) {
                    return Err(Error::from("Profile name 'cache' is reserved."));
                }

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

        let cache_dir = {
            let mut base_dir = base_dir.clone();
            base_dir.push("cache");
            base_dir
        };

        fs::create_dir_all(&base_dir)?;
        fs::create_dir_all(&profile_dir)?;
        fs::create_dir_all(&cache_dir)?;

        Ok(Profile {
            base_dir,
            profile_dir,
            cache_dir,
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
