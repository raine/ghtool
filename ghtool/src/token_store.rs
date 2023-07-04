use eyre::Result;
use keyring::{error::Error, Entry};
use tracing::info;

pub fn set_token(hostname: &str, token: &str) -> Result<(), Error> {
    let entry = Entry::new("ghtool", hostname)?;
    info!("Setting token for {}", hostname);
    entry.set_password(token)
}

pub fn get_token(hostname: &str) -> Result<String, Error> {
    let entry = Entry::new("ghtool", hostname)?;
    let token = entry.get_password()?;
    info!("Got token for {}: {}", hostname, token);
    Ok(token)
}

pub fn delete_token(hostname: &str) -> Result<(), Error> {
    let entry = Entry::new("ghtool", hostname)?;
    info!("Deleting token for {}", hostname);
    entry.delete_password()
}
