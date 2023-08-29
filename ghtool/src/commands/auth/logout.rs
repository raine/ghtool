use crate::{term::bold, token_store};
use eyre::Result;

pub fn logout() -> Result<()> {
    // Assume hostname github.com for now
    let hostname = "github.com";
    token_store::delete_token(hostname)?;
    println!("Logged out of {} account", bold(hostname));
    Ok(())
}
