use crate::{term::bold, token_store};
use eyre::Result;

pub fn logout(hostname: &str) -> Result<()> {
    token_store::delete_token(hostname)?;
    println!("Logged out of {} account", bold(hostname));
    Ok(())
}
