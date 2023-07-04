use std::{
    io::{self, Write},
    time::Duration,
};

use eyre::{eyre, Result};
use indicatif::ProgressBar;
use tracing::info;

use crate::{
    github::{AccessToken, AccessTokenResponse, CodeResponse, GithubAuthClient},
    spinner::make_spinner_style,
    term::bold,
    token_store,
};

pub async fn login(hostname: &str) -> Result<()> {
    let auth_client = GithubAuthClient::new()?;
    let code_response = auth_client.get_device_code().await?;

    println!(
        "First copy your one-time code: {}",
        bold(&code_response.user_code)
    );

    prompt_for_user_to_continue("Press Enter to open github.com in your browser...")?;

    info!("Opening {} in browser", code_response.verification_uri);
    open::that(&code_response.verification_uri)?;

    let pb = create_progress_bar();
    let access_token = await_authorization(&auth_client, &code_response).await?;
    pb.finish_and_clear();

    token_store::set_token(hostname, &access_token.access_token)
        .map_err(|e| eyre!("Failed to store token: {}", e))?;

    println!("Logged in to {} account", bold(hostname));
    Ok(())
}

fn prompt_for_user_to_continue(prompt_message: &str) -> io::Result<()> {
    print!("{}", prompt_message);
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    Ok(())
}

fn create_progress_bar() -> ProgressBar {
    let pb = ProgressBar::new_spinner();
    pb.enable_steady_tick(Duration::from_millis(100));
    pb.set_style(make_spinner_style());
    pb.set_message("Waiting for authorization...");
    pb
}

async fn await_authorization(
    auth_client: &GithubAuthClient,
    code_response: &CodeResponse,
) -> Result<AccessToken> {
    loop {
        let token_response = auth_client
            .get_access_token(&code_response.device_code)
            .await?;

        match token_response {
            AccessTokenResponse::AuthorizationPending(_) => {
                tokio::time::sleep(Duration::from_secs(code_response.interval.into())).await;
            }
            AccessTokenResponse::AccessToken(token) => {
                return Ok(token);
            }
        };
    }
}
