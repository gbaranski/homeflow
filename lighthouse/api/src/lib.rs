use async_trait::async_trait;
use lighthouse_proto::{execute, execute_response};
use lighthouse_types::{DeviceError, ExecuteRequest, ExecuteResponse};
use thiserror::Error;
use tokio::sync::Mutex;
use types::DeviceID;
use url::Url;

pub mod prelude;

#[derive(Debug, Error)]
pub enum Error {
    #[error("Error with device: {0}")]
    DeviceError(#[from] DeviceError),

    #[error("Error when sending request: {0}")]
    ReqwestError(#[from] reqwest::Error),
}

#[derive(Clone)]
pub struct Lighthouse {
    url: Url,
}

impl Lighthouse {
    pub fn new(server_address: std::net::SocketAddr) -> Self {
        Self {
            url: Url::parse(&format!("http://{}/lighthouse/", server_address)).unwrap(),
        }
    }
}

#[async_trait]
impl prelude::Lighthouse for Lighthouse {
    async fn execute(
        &self,
        frame: &execute::Frame,
        device_id: &DeviceID,
    ) -> Result<execute_response::Frame, Error> {
        let request = ExecuteRequest {
            device_id: device_id.clone(),
            frame: frame.clone(),
        };
        let url = self.url.join("execute").unwrap();

        let client = reqwest::Client::new();
        let response = client
            .post(url)
            .json(&request)
            .send()
            .await?
            .json::<ExecuteResponse>()
            .await?
            .into_result()?;

        Ok(response.frame)
    }
}

mod mocks {
    use super::*;
    use tokio::sync::mpsc;

    pub struct LighthouseMock {
        pub request_sender: mpsc::Sender<execute::Frame>,
        pub response_receiver: Mutex<mpsc::Receiver<execute_response::Frame>>,
    }

    #[async_trait]
    impl prelude::Lighthouse for LighthouseMock {
        async fn execute(
            &self,
            frame: &execute::Frame,
            _device_id: &DeviceID,
        ) -> Result<execute_response::Frame, Error> {
            self.request_sender.send(frame.clone()).await.unwrap();
            let response_frame = self.response_receiver.lock().await.recv().await.unwrap();
            Ok(response_frame)
        }
    }
}

pub use mocks::LighthouseMock;
