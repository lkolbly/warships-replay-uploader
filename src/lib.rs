use reqwest::blocking::Client;
use sha2::Digest;
use sha2::Sha256;
use std::collections::HashSet;
use std::fs;
use std::fs::File;
use std::io::Read;
use std::path::PathBuf;
use thiserror::Error;
#[macro_use]
extern crate log;

#[derive(Error, Debug)]
pub enum Error {
    #[error("Io Error")]
    IoError(#[from] std::io::Error),

    #[error("HTTP Error")]
    HttpError(#[from] reqwest::Error),

    #[error("Serde Error")]
    SerdeError(#[from] serde_json::Error),

    #[error("System Time Error")]
    SysTimeError(#[from] std::time::SystemTimeError),
}

pub type Result<T> = core::result::Result<T, Error>;

fn read_and_hash(path: &PathBuf) -> Result<(String, Vec<u8>)> {
    let mut f = File::open(path)?;
    let mut contents = Vec::new();
    f.read_to_end(&mut contents)?;

    let mut hasher = Sha256::new();
    hasher.update(&contents);
    let hash = hex::encode(hasher.finalize());
    Ok((hash, contents))
}

pub fn upload_replays(
    replay_path: &str,
    base_url: &str,
    mut already_uploaded: HashSet<String>,
) -> Result<HashSet<String>> {
    trace!(
        "Uploading replays from {} with {} seen replays",
        replay_path,
        already_uploaded.len()
    );

    let paths = fs::read_dir(replay_path)?;

    let mut blobs = vec![];
    for path in paths {
        let path = path?;
        let path_string = path.file_name().into_string().unwrap();

        if already_uploaded.contains(&path_string) {
            trace!("Path {} has already been seen", path_string);
            continue;
        }

        // Only upload replays once they're 30 minutes old (to avoid uploading them mid-game)
        let file_mod_time = path.metadata()?.modified()?;
        let now = std::time::SystemTime::now();
        if now.duration_since(file_mod_time)?.as_secs() < 60 * 30 {
            trace!("Path {} has been modified too recently", path_string);
            continue;
        }

        debug!("Hashing path {}", path_string);
        if path.metadata()?.len() < 50_000_000 {
            let (hash, contents) = match read_and_hash(&path.path()) {
                Ok(x) => x,
                Err(_) => {
                    continue;
                }
            };
            blobs.push((path_string, hash, contents));
        }
    }

    if blobs.len() == 0 {
        trace!("There are no new replays");
        return Ok(already_uploaded);
    }

    let hashes: Vec<_> = blobs.iter().map(|(_, hash, _)| hash).collect();
    let client = Client::new();
    let contained: Vec<bool> = client
        .post(&format!("{}/contains", base_url))
        .body(serde_json::to_string(&hashes)?)
        .send()?
        .json()?;

    if contained.len() == blobs.len() {
        for (path, hash, content) in blobs
            .iter()
            .zip(contained)
            .filter(|(_, x)| !x)
            .map(|(a, _)| a)
        {
            let res = client
                .post(&format!("{}/insert", base_url))
                .body(content.clone())
                .send()?
                .text()?;
            if res == "OK" {
                info!("Uploaded {}-byte replay {}", content.len(), hash);
                already_uploaded.insert(hash.to_string());
                already_uploaded.insert(path.to_string());
            } else {
                error!(
                    "Server error uploading {}-byte replay {}",
                    content.len(),
                    hash
                );
            }
        }
    } else {
        error!(
            "contained.len()={} but blobs.len()={}!",
            contained.len(),
            blobs.len()
        );
    }

    Ok(already_uploaded)
}
