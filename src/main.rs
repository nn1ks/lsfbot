use anyhow::{Context as _, Result};
use chrono::{NaiveDate, TimeZone, Utc};
use clap::Clap;
use config::Config;
use modul::Modul;
use serenity::client::{Client, Context, EventHandler};
use serenity::framework::standard::macros::{command, group};
use serenity::framework::standard::{Args, CommandResult, StandardFramework};
use serenity::model::{channel::Message, id::ChannelId};
use serenity::prelude::TypeMapKey;
use std::sync::{Arc, Mutex};
use std::{fs, io, thread, time::Duration};

mod arg;
mod config;
mod modul;
mod scraper;

struct Data {
    module: Vec<Modul>,
}

impl TypeMapKey for Data {
    type Value = Arc<Mutex<Data>>;
}

#[group]
#[commands(list, update)]
struct General;

#[command]
fn list(ctx: &mut Context, msg: &Message, args: Args) -> CommandResult {
    let map = ctx.data.read();
    let config = map.get::<Config>().unwrap();
    let data = map.get::<Data>().unwrap();
    let module = &data.lock().unwrap().module;
    let date = if args.is_empty() {
        chrono_tz::Europe::Berlin.from_utc_date(&Utc::today().naive_utc())
    } else {
        let date = match args.parse::<NaiveDate>() {
            Ok(v) => v,
            Err(_) => {
                msg.reply(&ctx.http, "Error: Invalid date format")?;
                return Ok(());
            }
        };
        chrono_tz::Europe::Berlin.from_local_date(&date).unwrap()
    };
    let mut messages = module
        .iter()
        .flat_map(|modul| modul.messages(|termin| date == termin.anfang.date()))
        .collect::<Vec<_>>();
    messages.sort_by_key(|m| m.1.anfang);
    if messages.is_empty() {
        msg.channel_id.send_message(&ctx.http, |m| {
            m.content(format!(
                "Keine Lehrveranstaltungen am {}",
                date.format("%d.%m.%Y")
            ))
        })?;
    } else {
        for message in messages {
            msg.channel_id
                .send_message(&ctx.http, |m| message.to_create_message(m, &config))?;
        }
    }
    Ok(())
}

#[command]
fn update(ctx: &mut Context, msg: &Message) -> CommandResult {
    let map = ctx.data.read();
    let config = map.get::<Config>().unwrap();
    let module = match scraper::fetch_module(&config) {
        Ok(v) => v,
        Err(e) => {
            msg.reply(&ctx.http, format!("Error: {}", e))?;
            return Ok(());
        }
    };
    let mut map = ctx.data.write();
    let data = map.get_mut::<Data>().unwrap();
    data.lock().unwrap().module = module;
    msg.reply(&ctx.http, "Stundenplan wurde aktualisiert")?;
    Ok(())
}

struct Handler;

impl EventHandler for Handler {}

fn main() -> Result<()> {
    fern::Dispatch::new()
        .format(|out, message, record| {
            out.finish(format_args!(
                "{} [{:>5}] {}: {}",
                chrono::Local::now().format("%Y-%m-%dT%H:%M:%S"),
                record.level(),
                record.target(),
                message
            ))
        })
        .level(log::LevelFilter::Info)
        .chain(io::stdout())
        .apply()?;

    let args = arg::Args::parse();

    let config_data = fs::read_to_string(args.config).context("Failed to read config file")?;
    let config: Config =
        toml::from_str(&config_data).context("Failed to deserialize config file")?;
    let config = Arc::new(config);

    let data = Arc::new(Mutex::new(Data { module: Vec::new() }));

    let mut client = Client::new(&config.discord.bot_token, Handler).unwrap();

    let http_client = Arc::clone(&client.cache_and_http.http);
    let bot_id = http_client.get_current_user().unwrap().id;
    let framework = StandardFramework::new()
        .configure(|c| c.on_mention(Some(bot_id)))
        .group(&GENERAL_GROUP);
    client.with_framework(framework);

    {
        let mut client_data = client.data.write();
        client_data.insert::<Config>(Arc::clone(&config));
        client_data.insert::<Data>(Arc::clone(&data));
    }

    let start_client_join_handle = thread::spawn(move || {
        log::info!("Starting discord client");
        client.start()
    });

    let module = scraper::fetch_module(&config).context("Failed to fetch data from website")?;
    data.lock().unwrap().module = module;

    let reminder_join_handle = thread::spawn(move || {
        log::info!("Checking for reminders");
        loop {
            for modul in &data.lock().unwrap().module {
                let messages = modul.messages(|termin| {
                    let duration = termin.anfang.signed_duration_since(Utc::now());
                    duration.num_minutes() > 25 && duration.num_minutes() < 30
                });
                for message in messages {
                    match ChannelId(config.discord.channel_id)
                        .send_message(&http_client, |m| message.to_create_message(m, &config))
                    {
                        Ok(_) => log::info!("Sent reminder message"),
                        Err(e) => log::error!("Failed to send reminder message: {}", e),
                    }
                }
            }
            thread::sleep(Duration::from_secs(300));
        }
    });

    start_client_join_handle.join().unwrap()?;
    reminder_join_handle.join().unwrap();

    Ok(())
}
