use crate::error::Error;
use std::env;
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
                dir.push(profile);

                {
                    // make sure the profile dir is inside the base dir, i.e. protect from directory traversal
                    let base_canonical = base_dir
                        .canonicalize()
                        .map_err(|_| Error::from("Base dir is invalid."))?;
                    let profile_canonical = dir
                        .canonicalize()
                        .map_err(|_| Error::from("Profile dir is invalid."))?;
                    if !profile_canonical.starts_with(&base_canonical) {
                        return Err(Error::from(format!(
                            "Profile dir is outside of the base dir ({:?} not in {:?})",
                            profile_canonical, base_canonical
                        )));
                    }
                }

                dir
            }
            Err(_) => base_dir.clone(),
        };

        let cache_dir = {
            let mut base_dir = base_dir.clone();
            base_dir.push("cache");
            base_dir
        };

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
