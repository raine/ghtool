use eyre::Result;
use http::HeaderMap;
use serde::Deserialize;
use tracing::{error, info};

pub struct GithubAuthClient {
    client: reqwest::Client,
}

const GITHUB_BASE_URI: &str = "https://github.com";
// const CLIENT_ID: &str = "Iv1.1bbd5e03617adebb";
const CLIENT_ID: &str = "32a2525cc736ee9b63ae";
const USER_AGENT: &str = "ghtool";
const GRANT_TYPE: &str = "urn:ietf:params:oauth:grant-type:device_code";

#[derive(Deserialize, Debug)]
pub struct CodeResponse {
    pub device_code: String,
    pub user_code: String,
    pub verification_uri: String,
    pub expires_in: u32,
    pub interval: u32,
}

#[derive(Deserialize, Debug)]
pub struct AccessToken {
    pub access_token: String,
    pub scope: String,
    pub token_type: String,
}

#[derive(Deserialize, Debug)]
pub struct Error {
    pub error: String,
    pub error_description: String,
    pub error_uri: String,
}

pub enum AccessTokenResponse {
    AuthorizationPending(Error),
    AccessToken(AccessToken),
}

impl GithubAuthClient {
    pub fn new() -> Result<Self> {
        let client = reqwest::Client::builder()
            .user_agent(USER_AGENT)
            .default_headers(make_headers())
            .build()
            .map_err(|e| eyre::eyre!("Failed to build client: {}", e))?;

        Ok(Self { client })
    }

    pub async fn get_device_code(&self) -> Result<CodeResponse> {
        let params = [("client_id", CLIENT_ID), ("scope", "repo")];
        let url = format!("{}/login/device/code", GITHUB_BASE_URI);
        info!("Requesting device code from {}", url);
        let res = self.client.post(url).form(&params).send().await?;
        let code_response: CodeResponse = res.json().await?;
        info!("Received device code: {:?}", code_response);
        Ok(code_response)
    }

    pub async fn get_access_token(&self, device_code: &str) -> Result<AccessTokenResponse> {
        let params = [
            ("client_id", CLIENT_ID),
            ("device_code", device_code),
            ("grant_type", GRANT_TYPE),
        ];
        let url = format!("{}/login/oauth/access_token", GITHUB_BASE_URI);
        info!("Requesting access token from {}", url);
        let res = self.client.post(url).form(&params).send().await?;

        if res.status().is_success() {
            let bytes = res.bytes().await?;
            let token_response: Result<AccessToken, _> = serde_json::from_slice(&bytes);
            info!("Received response: {:?}", token_response);
            match token_response {
                Ok(token) => Ok(AccessTokenResponse::AccessToken(token)),
                Err(_) => {
                    let error_response: Error = serde_json::from_slice(&bytes)?;
                    if error_response.error == "authorization_pending" {
                        info!(?error_response, "Authorization pending");
                        Ok(AccessTokenResponse::AuthorizationPending(error_response))
                    } else {
                        error!(?error_response, "Unexpected error");
                        Err(eyre::eyre!(
                            "Unexpected error: {} - {}",
                            error_response.error,
                            error_response.error_description
                        ))
                    }
                }
            }
        } else {
            Err(eyre::eyre!("Failed to get access token"))
        }
    }
}

fn make_headers() -> HeaderMap {
    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert(reqwest::header::ACCEPT, "application/json".parse().unwrap());
    headers
}
