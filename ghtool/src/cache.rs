use std::time::SystemTime;

use eyre::Result;
use futures::Future;
use lazy_static::lazy_static;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use tracing::{debug, info};

lazy_static! {
    pub static ref CACHE_DIR: String = {
        let mut path = dirs::cache_dir().expect("failed to get cache dir");
        path.push("ghtool");
        let cache_path = path.to_str().unwrap().to_string();
        info!(?path, "using cache path");
        cache_path
    };
}

#[derive(Serialize, Deserialize)]
struct CacheValue<V> {
    value: V,
    timestamp: SystemTime,
}

// The db needs to be opened in call to allow multiple processes
pub fn put<K, V>(key: K, value: V) -> Result<()>
where
    K: AsRef<[u8]> + std::fmt::Debug,
    V: Serialize,
{
    let db = open_db()?;
    let value = CacheValue {
        value,
        timestamp: SystemTime::now(),
    };
    let bytes = serde_json::to_vec(&value)?;
    db.insert(&key, bytes)?;
    debug!(?key, "cache key set");
    db.flush()?;
    Ok(())
}

pub fn get<K, V>(key: K) -> Result<Option<V>>
where
    K: AsRef<[u8]> + std::fmt::Debug,
    V: DeserializeOwned,
{
    let db = open_db()?;
    let bytes = db.get(&key)?;
    let value = match bytes {
        Some(bytes) => {
            debug!(?key, "found cached key");
            let value: CacheValue<V> = serde_json::from_slice(&bytes)?;
            Some(value.value)
        }
        None => None,
    };
    Ok(value)
}

pub async fn memoize<F, Fut, K, V>(key: K, f: F) -> Result<V>
where
    F: FnOnce() -> Fut,
    Fut: Future<Output = Result<V>>,
    K: AsRef<[u8]> + std::fmt::Debug,
    V: Serialize + DeserializeOwned + Clone,
{
    let cached = get(key.as_ref())?;
    match cached {
        Some(cached) => Ok(cached),
        None => {
            debug!(?key, "key not found in cache");
            let value = f().await?;
            put(key, value.clone())?;
            Ok(value)
        }
    }
}

fn open_db() -> Result<sled::Db> {
    let db = sled::Config::new()
        .path(CACHE_DIR.as_str())
        .use_compression(true)
        .open()?;
    Ok(db)
}
