use std::env;
use std::fs::File;
use std::io::copy;
use std::path::PathBuf;
use std::time::Duration;

use chrono::Local;
use regex::Regex;
use scraper::{Html, Selector};
use serenity::async_trait;
use serenity::model::prelude::*;
use serenity::prelude::*;
use tempfile::Builder;

mod outsaver_exception;
use outsaver_exception::OutsaverException;

mod mega;
use mega::Mega;

struct Handler {
    mega: Mega,
}

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
            "Wow, I found {} outplayed video. Do you want to save it?",
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
                let will_upload = msg
                    .reply(ctx, "Let's go! It will be uploaded to our MEGA!")
                    .await;
                upload_medias(&full_links, &msg.author.name, &self.mega).await;
                will_upload
            } else {
                msg.reply(ctx, "Hmm, that's ok. I won't upload it").await
            };
        }
    }

    async fn ready(&self, _: Context, ready: Ready) {
        println!("{} is connected and waiting for messages!", ready.user.name);
    }
}

async fn upload_medias(urls: &Vec<String>, author_nick: &str, mega: &Mega) {
    let tmp_dir = Builder::new().prefix("outplayed").tempdir().unwrap();
    let mega_node = env::var("MEGA_NODE").expect("Expected a MEGA_NODE in the environment");
    for url in urls {
        let video_information = create_video_information_from_url(url).await.unwrap();
        let video_path = download_and_save_temporary_video(&video_information, &tmp_dir).await;
        let full_title = format!(
            "{} | {} | {}.{}",
            author_nick,
            video_information.title,
            Local::now().format("%Y%m%d%H%M%S"),
            video_information.extension
        );

        mega.upload_video(&video_path, &full_title, &mega_node)
            .await;
    }
}

async fn create_video_information_from_url(
    url: &str,
) -> Result<VideoInformation, OutsaverException> {
    let response = reqwest::get(url).await.unwrap().text().await.unwrap();
    let document = Html::parse_document(&response);

    let title_selector = Selector::parse("title").unwrap();
    let title = document
        .select(&title_selector)
        .next()
        .unwrap()
        .inner_html();

    let video_selector = Selector::parse("video").unwrap();
    let mut videos = document.select(&video_selector);
    let video_count = videos.clone().count();

    match video_count {
        0 => return Err(OutsaverException::new("No video found")),
        1 => {}
        _ => return Err(OutsaverException::new("More than one video found")),
    }

    let video = videos.next().unwrap();
    let video_url = video.value().attr("src").expect("Expected a video url");

    return Ok(VideoInformation {
        title,
        url: video_url.to_string(),
        extension: video_url.split('.').last().unwrap().to_string(),
    });
}

async fn download_and_save_temporary_video(
    video_information: &VideoInformation,
    tmp_dir: &tempfile::TempDir,
) -> PathBuf {
    let video_response = reqwest::get(&video_information.url).await.unwrap();
    let content = video_response.bytes().await.unwrap();

    let temporary_file_name = format!(
        "{}.{}",
        Local::now().format("%Y%m%d%H%M%S"),
        video_information.extension
    );
    let destination = tmp_dir.path().join(temporary_file_name);
    let mut file = File::create(&destination).unwrap();

    copy(&mut &content.to_vec()[..], &mut file).unwrap();

    destination
}

#[tokio::main]
async fn main() {
    let token = env::var("DISCORD_TOKEN").expect("Expected a DISCORD_TOKEN in the environment");
    let intents = GatewayIntents::GUILD_MESSAGES
        | GatewayIntents::GUILD_MESSAGE_REACTIONS
        | GatewayIntents::DIRECT_MESSAGES
        | GatewayIntents::MESSAGE_CONTENT;

    let mut mega = Mega::new();
    mega.login().await;

    let mut client = Client::builder(&token, intents)
        .event_handler(Handler { mega })
        .await
        .expect("Error creating client");

    if let Err(why) = client.start().await {
        println!("Client error: {why:?}");
    }
}

struct VideoInformation {
    title: String,
    url: String,
    extension: String,
}
