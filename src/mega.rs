use std::env;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use async_read_progress::TokioAsyncReadProgressExt;
use indicatif::{ProgressBar, ProgressStyle};
use tokio_util::compat::TokioAsyncReadCompatExt;

pub struct Mega {
    pub client: mega::Client,
}

impl Mega {
    pub fn new() -> Self {
        let http_client = reqwest::Client::new();
        let client = mega::Client::builder().build(http_client).unwrap();
        Mega { client }
    }

    pub async fn login(&mut self) {
        let email = env::var("MEGA_EMAIL").expect("Expected a MEGA_EMAIL in the environment");
        let password =
            env::var("MEGA_PASSWORD").expect("Expected a MEGA_PASSWORD in the environment");
        let _ = &self
            .client
            .login(&email, &password, None)
            .await
            .expect("Failed to login");
    }

    pub async fn upload_video(&self, path: &PathBuf, filename: &str, node_handle: &str) {
        let file = tokio::fs::File::open(path).await.unwrap();
        let size = file.metadata().await.unwrap().len();

        let nodes = self.client.fetch_own_nodes().await.unwrap();
        let node = nodes.get_node_by_handle(node_handle).unwrap();

        let bar = ProgressBar::new(size);
        bar.set_style(ProgressStyle::default_bar());
        let bar = Arc::new(bar);
        let reader = {
            let bar = bar.clone();
            file.report_progress(Duration::from_millis(100), move |bytes_read| {
                bar.set_position(bytes_read as u64)
            })
        };

        let _ = self
            .client
            .upload_node(
                &node,
                filename,
                size,
                reader.compat(),
                mega::LastModified::Now,
            )
            .await;
    }
}
