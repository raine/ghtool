use std::time::Duration;

use eyre::{eyre, Context, Result};
use indicatif::ProgressBar;
use tracing::info;

use crate::{
    github::{AccessToken, AccessTokenResponse, CodeResponse, GithubAuthClient},
    spinner::make_spinner_style,
    term::{bold, prompt_for_user_to_continue, read_stdin},
    token_store,
};

pub async fn login(hostname: &str, use_stdin_token: bool) -> Result<()> {
    let access_token = if use_stdin_token {
        read_stdin()?
    } else {
        acquire_token_from_github().await?
    };

    token_store::set_token(hostname, &access_token)
        .map_err(|e| eyre!("Failed to store token: {}", e))?;

    println!("Logged in to {} account", bold(hostname));
    Ok(())
}

async fn acquire_token_from_github() -> Result<String> {
    let auth_client = GithubAuthClient::new()?;
    let code_response = auth_client
        .get_device_code()
        .await
        .wrap_err("Failed to get device code")?;

    println!(
        "First copy your one-time code: {}",
        bold(&code_response.user_code)
    );

    prompt_for_user_to_continue("Press Enter to open github.com in your browser...")?;

    info!("Opening {} in browser", code_response.verification_uri);
    open::that(&code_response.verification_uri)?;

    let pb = create_progress_bar();
    let token = await_authorization(&auth_client, &code_response).await?;
    pb.finish_and_clear();
    Ok(token.access_token)
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
