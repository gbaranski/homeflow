use fulfillment_types::{SyncRequest, SyncResponse};
use reqwest::Client;
use token::Token;
use url::Url;

#[derive(Clone)]
pub struct Fulfillment {
    pub url: Url,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Sending request failed: `{0}`")]
    ReqwestError(#[from] reqwest::Error),
}

impl Fulfillment {
    pub fn new(url: Url) -> Self {
        Self { url }
    }

    pub async fn sync(&self, access_token: &Token) -> Result<SyncResponse, Error> {
        let client = Client::new();
        let url = self.url.join("internal/sync").unwrap();
        let response = client
            .get(url)
            .json(&SyncRequest {})
            .bearer_auth(access_token.to_string())
            .send()
            .await?
            .json::<SyncResponse>()
            .await?;

        Ok(response)
    }
}
