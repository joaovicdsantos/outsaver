use std::error::Error;
use std::fs::{self, File};
use std::io::copy;
use std::path::PathBuf;
use std::process;
use std::time::Duration;

use chrono::Local;
use exception::outsaver_exception::OutsaverException;
use log::{error, info};
use regex::Regex;
use scraper::{Html, Selector};
use serenity::async_trait;
use serenity::model::prelude::*;
use serenity::prelude::*;
use tempfile::Builder;

mod mega;
use mega::Mega;

mod config;
mod exception;

struct DiscordHandler {
    mega: Mega,
    destination_node: String,
}

const OUTPLAYED_URL_PREFIX: &str = "https://outplayed.tv/media";

#[async_trait]
impl EventHandler for DiscordHandler {
    async fn message(&self, ctx: Context, msg: Message) {
        info!(
            "New message received from {} in {}",
            msg.author, msg.channel_id
        );
        let content = msg.content.clone();
        let is_a_outplayed_media = content.contains(OUTPLAYED_URL_PREFIX);
        if !is_a_outplayed_media {
            info!("Not an outplayed media. Skipping.");
            return;
        }

        let link_regex = match Regex::new(r"\bhttps?://\S+\b") {
            Ok(regex) => regex,
            Err(e) => {
                let message = format!("Failed to create link regex. Error: {e}");
                error!("{message}");
                panic!("{}", message);
            }
        };
        let full_links = link_regex
            .find_iter(&content)
            .map(|link| link.as_str().to_string())
            .collect::<Vec<String>>();

        info!("Found {} links", full_links.len());
        let confirmation_text = format!(
            "Wow, I found {} outplayed video. Do you want to save it?",
            full_links.len()
        );
        match send_confirmation_message(&ctx, &msg, &confirmation_text).await {
            Ok(_) => {}
            Err(e) => error!("Failed to send confirmation message. Error: {e}"),
        };

        let collector = msg
            .author
            .await_reaction(&ctx.shard)
            .timeout(Duration::from_secs(15))
            .author_id(msg.author.id);
        if let Some(reaction) = collector.await {
            let _ = if reaction.emoji == ReactionType::Unicode("✅".into()) {
                info!("User accepted the upload");
                let will_upload = msg
                    .reply(ctx, "Let's go! It will be uploaded to our MEGA!")
                    .await;
                match upload_medias(
                    &full_links,
                    &msg.author.name,
                    &self.mega,
                    &self.destination_node,
                )
                .await
                {
                    Ok(_) => {}
                    Err(e) => error!("Failed to upload medias. Error: {e}"),
                };
                will_upload
            } else {
                info!("User rejected the upload");
                msg.reply(ctx, "Hmm, that's ok. I won't upload it").await
            };
        }
    }

    async fn ready(&self, _: Context, ready: Ready) {
        info!("{} is connected and waiting for messages!", ready.user.name);
    }
}

async fn send_confirmation_message(
    ctx: &Context,
    msg: &Message,
    confirmation_text: &str,
) -> Result<Message, Box<dyn Error>> {
    let confirmation_message = match msg.reply(&ctx.http, confirmation_text).await {
        Ok(msg) => msg,
        Err(e) => {
            let message = format!("Failed to send confirmation message. Error: {e}");
            return Err(OutsaverException::new(&message))?;
        }
    };
    confirmation_message
        .react(&ctx.http, ReactionType::Unicode("✅".into()))
        .await?;
    confirmation_message
        .react(&ctx.http, ReactionType::Unicode("❌".into()))
        .await?;

    Ok(confirmation_message)
}

async fn upload_medias(
    urls: &Vec<String>,
    author_nick: &str,
    mega: &Mega,
    destination_node: &str,
) -> Result<(), Box<dyn Error>> {
    info!("Uploading {} medias", urls.len());
    let tmp_dir = Builder::new().prefix("outplayed").tempdir()?;
    for url in urls {
        let video_information = match create_video_information_from_url(url).await {
            Ok(video_information) => video_information,
            Err(e) => {
                error!("Failed to create video information. Error: {e}. Trying to continue...");
                continue;
            }
        };
        let video_path = match download_and_save_temporary_video(&video_information, &tmp_dir).await
        {
            Ok(video_path) => video_path,
            Err(e) => {
                error!("Failed to download and save temporary video. Error: {e}. Trying to continue...");
                continue;
            }
        };
        let full_title = Mega::remove_invalid_characters(format!(
            "{} - {} - {}.{}",
            author_nick,
            video_information.game,
            Local::now().format("%Y%m%d%H%M%S"),
            video_information.extension
        ));

        match mega
            .upload_video(&video_path, &full_title, &destination_node)
            .await
        {
            Ok(_) => {}
            Err(e) => {
                error!("Failed to upload video. Error: {e}. Trying to continue...");
                continue;
            }
        };
        let _ = fs::remove_file(&video_path);
        info!("Uploaded {}", full_title);
    }
    Ok(())
}

async fn create_video_information_from_url(url: &str) -> Result<VideoInformation, Box<dyn Error>> {
    let response = reqwest::get(url).await?.text().await?;
    let document = Html::parse_document(&response);

    let title_selector = Selector::parse("title")?;
    let page_title = document
        .select(&title_selector)
        .next()
        .ok_or(OutsaverException::new(
            "Error formatting video title. No title found",
        ))?
        .inner_html();
    let hashtag_pos = page_title.find("#").ok_or(OutsaverException::new(
        "Error formatting video title. No hashtag found",
    ))?;
    let pipe_pos = page_title.find("|").ok_or(OutsaverException::new(
        "Error formatting video title. No pipe found",
    ))?;
    let game = page_title[hashtag_pos + 1..pipe_pos].trim().to_string();

    let video_selector = Selector::parse("video")?;
    let mut videos = document.select(&video_selector);
    let video_count = videos.clone().count();

    match video_count {
        0 => return Err(OutsaverException::new("No video found"))?,
        1 => {}
        _ => return Err(OutsaverException::new("More than one video found"))?,
    }

    let video = videos.next().ok_or(OutsaverException::new(
        "Error mounting video information. No video found",
    ))?;
    let video_url = match video.value().attr("src") {
        Some(url) => url.to_string(),
        None => return Err(OutsaverException::new("No video url found"))?,
    };

    let extension = video_url
        .split('.')
        .last()
        .ok_or(OutsaverException::new("No extension found"))?
        .to_string();

    Ok(VideoInformation {
        game,
        url: video_url,
        extension,
    })
}

async fn download_and_save_temporary_video(
    video_information: &VideoInformation,
    tmp_dir: &tempfile::TempDir,
) -> Result<PathBuf, Box<dyn Error>> {
    info!("Downloading {}", video_information.url);
    let video_response = reqwest::get(&video_information.url).await?;
    let content = video_response.bytes().await?;

    let temporary_file_name = format!(
        "{}.{}",
        Local::now().format("%Y%m%d%H%M%S"),
        video_information.extension
    );
    let destination = tmp_dir.path().join(temporary_file_name);
    let mut file = File::create(&destination)?;

    copy(&mut &content.to_vec()[..], &mut file)?;

    Ok(destination)
}

#[tokio::main]
async fn main() {
    pretty_env_logger::init_timed();
    info!("Starting the bot...");

    let config = match config::Config::load() {
        Ok(config) => {
            info!("Config loaded");
            config
        }
        Err(why) => {
            error!("Error during loading the config: {why}");
            process::exit(1);
        }
    };

    let intents = GatewayIntents::GUILD_MESSAGES
        | GatewayIntents::GUILD_MESSAGE_REACTIONS
        | GatewayIntents::DIRECT_MESSAGES
        | GatewayIntents::MESSAGE_CONTENT;

    let mut mega = match Mega::new() {
        Ok(mega) => mega,
        Err(why) => {
            error!("Failed to create MEGA client. Error: {why}");
            process::exit(1);
        }
    };
    match mega
        .login(&config.env.mega.email, &config.env.mega.password)
        .await
    {
        Ok(_) => info!("Logged in to MEGA"),
        Err(why) => {
            error!("Failed to login to MEGA. Error: {why}");
            process::exit(1);
        }
    };

    let mut serenity_client = match Client::builder(&config.env.discord.token, intents)
        .event_handler(DiscordHandler {
            mega,
            destination_node: config.env.mega.destination_node,
        })
        .await
    {
        Ok(client) => client,
        Err(why) => {
            error!("Error creating the serenity client: {why}");
            process::exit(1);
        }
    };
    info!("Serenity client created");

    if let Err(why) = serenity_client.start().await {
        error!("Client error: {why:?}");
        process::exit(1);
    }
}

struct VideoInformation {
    game: String,
    url: String,
    extension: String,
}
