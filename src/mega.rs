use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use async_read_progress::TokioAsyncReadProgressExt;
use indicatif::{ProgressBar, ProgressStyle};
use tokio_util::compat::TokioAsyncReadCompatExt;

use crate::exception::mega_exception::MegaException;

pub struct Mega {
    pub client: mega::Client,
}

impl Mega {
    pub fn new() -> Result<Self, MegaException> {
        let http_client = reqwest::Client::new();
        match mega::Client::builder().build(http_client) {
            Ok(client) => Ok(Mega { client }),
            Err(e) => {
                let message = format!("Failed to create MEGA client. Error: {e}");
                Err(MegaException::new(&message))
            }
        }
    }

    pub fn remove_invalid_characters(name: String) -> String {
        let invalid_characters = vec!['"', '*', '/', ':', '<', '>', '?', '\\', '|'];
        let mut name = name.to_string();
        for c in invalid_characters {
            name = name.replace(&c.to_string(), "");
        }
        name
    }

    pub async fn login(&mut self, email: &str, password: &str) -> Result<(), MegaException> {
        match &self.client.login(&email, &password, None).await {
            Ok(_) => Ok(()),
            Err(e) => {
                let message = format!("Failed to login to MEGA. Error: {e}");
                Err(MegaException::new(&message))
            }
        }
    }

    pub async fn upload_video(
        &self,
        path: &PathBuf,
        filename: &str,
        node_handle: &str,
    ) -> Result<(), MegaException> {
        let file = match tokio::fs::File::open(path).await {
            Ok(file) => file,
            Err(e) => {
                let message = format!("Failed to open file. Error: {e}");
                return Err(MegaException::new(&message));
            }
        };
        let size = match file.metadata().await {
            Ok(metadata) => metadata.len(),
            Err(e) => {
                let message = format!("Failed to get file metadata. Error: {e}");
                return Err(MegaException::new(&message));
            }
        };

        let nodes = match self.client.fetch_own_nodes().await {
            Ok(nodes) => nodes,
            Err(e) => {
                let message = format!("Failed to fetch own nodes. Error: {e}");
                return Err(MegaException::new(&message));
            }
        };
        let node = match nodes.get_node_by_handle(node_handle) {
            Some(node) => node,
            None => {
                let message = format!("Failed to find node with handle {node_handle}");
                return Err(MegaException::new(&message));
            }
        };

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

        Ok(())
    }
}
