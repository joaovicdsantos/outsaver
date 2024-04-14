use std::env;
use std::fs::File;
use std::io::copy;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use async_read_progress::TokioAsyncReadProgressExt;
use indicatif::{ProgressBar, ProgressStyle};
use regex::Regex;
use serenity::async_trait;
use serenity::model::prelude::*;
use serenity::prelude::*;
use tempfile::Builder;
use tokio_util::compat::TokioAsyncReadCompatExt;

struct Handler;

const OUTPLAYED_URL_PREFIX: &str = "https://outplayed.tv/media";

#[async_trait]
impl EventHandler for Handler {
    async fn message(&self, ctx: Context, msg: Message) {
        let content = msg.content.clone();
        let is_a_outplayed_media = content.contains(OUTPLAYED_URL_PREFIX);
        if !is_a_outplayed_media {
            return;
        }

        let link_regex = Regex::new(r"\bhttps?://\S+\b").unwrap();
        let full_links = link_regex
            .find_iter(&content)
            .map(|m| m.as_str().to_string())
            .collect::<Vec<String>>();

        let confirmation_text = format!(
            "Wow, I found {} outplayed video. Do you want to upload it?",
            full_links.len()
        );
        let confirmation_message = msg.reply(&ctx.http, confirmation_text).await.unwrap();
        confirmation_message
            .react(&ctx.http, ReactionType::Unicode("✅".into()))
            .await
            .unwrap();
        confirmation_message
            .react(&ctx.http, ReactionType::Unicode("❌".into()))
            .await
            .unwrap();

        let collector = msg
            .author
            .await_reaction(&ctx.shard)
            .timeout(Duration::from_secs(15))
            .author_id(msg.author.id);
        if let Some(reaction) = collector.await {
            let _ = if reaction.emoji == ReactionType::Unicode("✅".into()) {
                tokio::spawn(async move { upload_medias(&full_links).await });
                msg.reply(ctx, "Let's go! It will be uploaded!").await
            } else {
                msg.reply(ctx, "Hmm, that's ok. I won't upload it").await
            };
        }
    }

    async fn ready(&self, _: Context, ready: Ready) {
        println!("{} is connected!", ready.user.name);
    }
}

async fn upload_medias(urls: &Vec<String>) {
    const VIDEO_TAG: &str = "<video src=\"";
    let tmp_dir = Builder::new().prefix("outplayed").tempdir().unwrap();
    for url in urls {
        let response = reqwest::get(url).await.unwrap().text().await.unwrap();
        let video_tag_src = match response.find(VIDEO_TAG) {
            Some(i) => i,
            None => {
                println!("Not a video: {url}");
                continue;
            }
        };

        let init_src_position = video_tag_src + VIDEO_TAG.len();
        let end_src = response[init_src_position..].find("\"").unwrap();
        let video_url = &response[init_src_position..init_src_position + end_src];

        let video_response = reqwest::get(video_url).await.unwrap();
        let dest = tmp_dir.path().join("video.mp4");
        let mut file = File::create(&dest).unwrap();
        let content = video_response.bytes().await.unwrap();
        let body_vec = content.to_vec();
        copy(&mut &body_vec[..], &mut file).unwrap();
        upload_media_to_mega(&dest).await;
    }
}

async fn upload_media_to_mega(path: &PathBuf) {
    let email = env::var("MEGA_EMAIL").expect("Expected a MEGA_EMAIL in the environment");
    let password = env::var("MEGA_PASSWORD").expect("Expected a MEGA_PASSWORD in the environment");

    let http_client = reqwest::Client::new();
    let mut mega = mega::Client::builder().build(http_client).unwrap();

    mega.login(&email, &password, None).await.unwrap();
    let nodes = mega.fetch_own_nodes().await.unwrap();
    let node = nodes.get_node_by_handle("Z0hnwThT").unwrap();

    let file = tokio::fs::File::open(path).await.unwrap();
    let size = file.metadata().await.unwrap().len();

    let bar = ProgressBar::new(size);
    bar.set_style(ProgressStyle::default_bar());
    let bar = Arc::new(bar);
    let reader = {
        let bar = bar.clone();
        file.report_progress(Duration::from_millis(100), move |bytes_read| {
            bar.set_position(bytes_read as u64)
        })
    };

    mega.upload_node(
        &node,
        "video.mp4",
        size,
        reader.compat(),
        mega::LastModified::Now,
    )
    .await
    .unwrap();

    mega.logout().await.unwrap();
}

#[tokio::main]
async fn main() {
    let token = env::var("DISCORD_TOKEN").expect("Expected a token in the environment");
    let intents = GatewayIntents::GUILD_MESSAGES
        | GatewayIntents::GUILD_MESSAGE_REACTIONS
        | GatewayIntents::DIRECT_MESSAGES
        | GatewayIntents::MESSAGE_CONTENT;

    let mut client = Client::builder(&token, intents)
        .event_handler(Handler)
        .await
        .expect("Error creating client");

    if let Err(why) = client.start().await {
        println!("Client error: {why:?}");
    }
}
