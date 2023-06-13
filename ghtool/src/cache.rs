use std::time::SystemTime;

use eyre::Result;
use futures::Future;
use lazy_static::lazy_static;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use sled::Db;
use tracing::{debug, info};

lazy_static! {
    pub static ref CACHE_DIR: String = {
        let mut path = dirs::cache_dir().expect("failed to get cache dir");
        path.push("ghtool");
        let cache_path = path.to_str().unwrap().to_string();
        info!(?path, "using cache path");
        cache_path
    };
    pub static ref DB: Db = sled::Config::new()
        .path(CACHE_DIR.as_str())
        .use_compression(true)
        .open()
        .expect("failed to open database");
}

#[derive(Serialize, Deserialize)]
struct CacheValue<V> {
    value: V,
    timestamp: SystemTime,
}

pub fn put<K, V>(key: K, value: V) -> Result<()>
where
    K: AsRef<[u8]> + std::fmt::Debug,
    V: Serialize,
{
    let value = CacheValue {
        value,
        timestamp: SystemTime::now(),
    };
    let bytes = serde_json::to_vec(&value)?;
    DB.insert(&key, bytes)?;
    debug!(?key, "cache key set");
    Ok(())
}

pub fn get<K, V>(key: K) -> Result<Option<V>>
where
    K: AsRef<[u8]> + std::fmt::Debug,
    V: DeserializeOwned,
{
    let bytes = DB.get(&key)?;
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
