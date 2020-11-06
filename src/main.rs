use anyhow::{Context as _, Result};
use chrono::{NaiveDate, TimeZone, Utc};
use chrono_humanize::HumanTime;
use clap::Clap;
use config::Config;
use modul::{Modul, ModulGruppe};
use serenity::client::{Client, Context, EventHandler};
use serenity::framework::standard::macros::{command, group, help};
use serenity::framework::standard::{
    help_commands, Args, CommandGroup, CommandResult, HelpOptions, StandardFramework,
};
use serenity::model::{channel::Message, id::ChannelId, id::UserId};
use serenity::prelude::TypeMapKey;
use std::sync::{Arc, Mutex};
use std::{collections::HashSet, fs, io, thread, time::Duration};
use user::Users;

mod arg;
mod config;
mod modul;
mod scraper;
mod user;

struct Data {
    module: Vec<Modul>,
    users: Users,
}

impl TypeMapKey for Data {
    type Value = Arc<Mutex<Data>>;
}

#[group]
#[commands(list, update)]
struct General;

#[group]
#[prefixes("dm")]
#[commands(enable, disable, remove, set)]
struct DirectMessages;

#[command]
fn list(ctx: &mut Context, msg: &Message, args: Args) -> CommandResult {
    let map = ctx.data.read();
    let config = map.get::<Config>().unwrap();
    let data = map.get::<Data>().unwrap();
    let module = &data.lock().unwrap().module;
    let date = match args.current() {
        Some(arg) => match NaiveDate::parse_from_str(arg, "%d.%m.%Y") {
            Ok(v) => chrono_tz::Europe::Berlin.from_local_date(&v).unwrap(),
            Err(_) => {
                msg.reply(&ctx.http, "Error: Invalid date format")?;
                return Ok(());
            }
        },
        None => Utc::now().with_timezone(&chrono_tz::Europe::Berlin).date(),
    };
    let mut messages = module
        .iter()
        .flat_map(|modul| modul.messages(|termin| date == termin.beginn.date()))
        .filter(|message| {
            let author_has_role = |role_id: u64| {
                msg.author
                    .has_role(&ctx.http, config.discord.guild_id, role_id)
                    .unwrap()
            };
            match message.modul.gruppe {
                Some(ModulGruppe::Gruppe1) => author_has_role(config.discord.gruppe_1.role_id),
                Some(ModulGruppe::Gruppe2) => author_has_role(config.discord.gruppe_2.role_id),
                Some(ModulGruppe::Gruppe3) => author_has_role(config.discord.gruppe_3.role_id),
                Some(ModulGruppe::Gruppe4) => author_has_role(config.discord.gruppe_4.role_id),
                None => true,
            }
        })
        .collect::<Vec<_>>();
    messages.sort_by_key(|m| m.modul_termin.beginn);
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

/// Enables direct messages
#[command]
fn enable(ctx: &mut Context, msg: &Message) -> CommandResult {
    let map = ctx.data.read();
    let config = map.get::<Config>().unwrap().clone();
    drop(map);
    let mut map = ctx.data.write();
    let data = map.get_mut::<Data>().unwrap();
    data.lock()
        .unwrap()
        .users
        .enable_or_add(msg.author.id, &ctx.http, &config)
        .unwrap();
    msg.reply(&ctx.http, "Enabled direct messages")?;
    Ok(())
}

/// Disables direct messages
#[command]
fn disable(ctx: &mut Context, msg: &Message) -> CommandResult {
    let mut map = ctx.data.write();
    let data = map.get_mut::<Data>().unwrap();
    data.lock().unwrap().users.disable(msg.author.id).unwrap();
    Ok(())
}

/// Disables direct messages
#[command]
fn remove(ctx: &mut Context, msg: &Message) -> CommandResult {
    let mut map = ctx.data.write();
    let data = map.get_mut::<Data>().unwrap();
    data.lock().unwrap().users.remove(msg.author.id).unwrap();
    Ok(())
}

/// Modifies configuration options for direct messages
///
/// Available subcommands:
/// - `send-before`: Takes either a number or `off` as value
/// - `send-after-previous`: Takes either `on` or `off` as value
#[command]
fn set(ctx: &mut Context, msg: &Message, mut args: Args) -> CommandResult {
    let mut map = ctx.data.write();
    let data = map.get_mut::<Data>().unwrap();
    match args.single::<String>().unwrap().as_str() {
        "send-before" => {
            let duration = match args.single::<String>().unwrap().as_str() {
                "off" => None,
                v => match v.parse::<u64>() {
                    Ok(v) => Some(user::Duration { minutes: v }),
                    Err(_) => {
                        msg.reply(
                            &ctx.http,
                            format!(
                                "Error: Unknown value `{}` (available values: number, `off`)",
                                v
                            ),
                        )?;
                        return Ok(());
                    }
                },
            };
            data.lock()
                .unwrap()
                .users
                .set_send_before(msg.author.id, duration)
                .unwrap();
        }
        "send-after-previous" => {
            let enable = match args.single::<String>().unwrap().as_str() {
                "off" => false,
                "on" => true,
                v => {
                    msg.reply(
                        &ctx.http,
                        format!(
                            "Error: Unknown value `{}` (available values: `on`, `off`)",
                            v
                        ),
                    )?;
                    return Ok(());
                }
            };
            data.lock()
                .unwrap()
                .users
                .set_send_after(msg.author.id, enable)
                .unwrap();
        }
        v => {
            msg.reply(&ctx.http, format!("Error: Unknown subcommand `{}`", v))?;
            return Ok(());
        }
    };
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
    drop(map);
    let mut map = ctx.data.write();
    let data = map.get_mut::<Data>().unwrap();
    data.lock().unwrap().module = module;
    msg.reply(&ctx.http, "Stundenplan wurde aktualisiert")?;
    Ok(())
}

#[help]
fn help(
    context: &mut Context,
    msg: &Message,
    args: Args,
    help_options: &'static HelpOptions,
    groups: &[&'static CommandGroup],
    owners: HashSet<UserId>,
) -> CommandResult {
    help_commands::with_embeds(context, msg, args, help_options, groups, owners)
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

    let data = Arc::new(Mutex::new(Data {
        module: Vec::new(),
        users: Users::new(config.users.file.clone()).context("Failed to read users")?,
    }));

    let mut client = Client::new(&config.discord.bot_token, Handler).unwrap();

    let http_client = Arc::clone(&client.cache_and_http.http);
    let bot_id = http_client.get_current_user().unwrap().id;
    let framework = StandardFramework::new()
        .configure(|c| c.on_mention(Some(bot_id)))
        .help(&HELP)
        .group(&GENERAL_GROUP)
        .group(&DIRECTMESSAGES_GROUP);
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

        let send_message =
            |message: &modul::MessageData, group: &config::Group| match ChannelId(group.channel_id)
                .send_message(&http_client, |m| {
                    message
                        .to_create_message(m, &config)
                        .content(format!("<@&{}>", group.role_id))
                }) {
                Ok(_) => log::info!("Sent reminder message to channel `{}`", group.channel_id),
                Err(e) => log::error!("Failed to send reminder message: {}", e),
            };

        loop {
            let mut data_lock = data.lock().unwrap();
            data_lock.users.refresh().unwrap();
            let module = &data_lock.module;
            let mut messages = module
                .iter()
                .flat_map(|modul| {
                    modul.messages(|termin| {
                        let duration = termin.beginn.signed_duration_since(Utc::now());
                        duration.num_minutes() > 25 && duration.num_minutes() < 30
                    })
                })
                .collect::<Vec<_>>();
            messages.sort_by_key(|m| m.modul_termin.beginn);
            for message in messages {
                match message.modul.gruppe {
                    Some(ModulGruppe::Gruppe1) => send_message(&message, &config.discord.gruppe_1),
                    Some(ModulGruppe::Gruppe2) => send_message(&message, &config.discord.gruppe_2),
                    Some(ModulGruppe::Gruppe3) => send_message(&message, &config.discord.gruppe_3),
                    Some(ModulGruppe::Gruppe4) => send_message(&message, &config.discord.gruppe_4),
                    None => {
                        send_message(&message, &config.discord.gruppe_1);
                        send_message(&message, &config.discord.gruppe_2);
                        send_message(&message, &config.discord.gruppe_3);
                        send_message(&message, &config.discord.gruppe_4);
                    }
                }
            }

            for user in data_lock.users.get() {
                if !user.enabled {
                    continue;
                }
                let messages = module
                    .iter()
                    .flat_map(|modul| {
                        modul.messages(|termin| {
                            let duration = termin.beginn.signed_duration_since(Utc::now());
                            match user.send_before.as_ref().map(|v| v.minutes) {
                                Some(minutes) => {
                                    duration.num_minutes() > minutes as i64 - 5
                                        && duration.num_minutes() < minutes as i64
                                        && (modul.gruppe.is_none() || modul.gruppe == user.gruppe)
                                }
                                None => false,
                            }
                        })
                    })
                    .collect::<Vec<_>>();

                if !messages.is_empty() {
                    let channel = match user.id.create_dm_channel(&http_client) {
                        Ok(v) => v,
                        Err(e) => {
                            log::error!("Failed to create dm channel: {}", e);
                            continue;
                        }
                    };
                    for message in messages {
                        match channel
                            .send_message(&http_client, |m| message.to_create_message(m, &config))
                        {
                            Ok(_) => {
                                log::info!("Sent reminder message to dm channel `{}`", channel.id.0)
                            }
                            Err(e) => log::error!("Failed to send reminder message: {}", e),
                        };
                    }
                }

                if user.send_after_previous {
                    let messages_today = module
                        .iter()
                        .flat_map(|modul| {
                            modul.messages(|termin| {
                                termin.beginn.date() == Utc::now().date()
                                    && (modul.gruppe.is_none() || modul.gruppe == user.gruppe)
                            })
                        })
                        .collect::<Vec<_>>();
                    let last = messages_today
                        .iter()
                        .filter(|v| v.modul_termin.beginn < Utc::now())
                        .map(|v| v.modul_termin.ende)
                        .find(|v| {
                            let duration = v.signed_duration_since(Utc::now());
                            duration.num_minutes() > 0 && duration.num_minutes() < 5
                        });
                    let next_message = last.and_then(|last| {
                        messages_today
                            .into_iter()
                            .filter(|v| v.modul_termin.beginn > last)
                            .min_by_key(|v| v.modul_termin.beginn)
                    });
                    if let Some(message) = next_message {
                        let channel = match user.id.create_dm_channel(&http_client) {
                            Ok(v) => v,
                            Err(e) => {
                                log::error!("Failed to create dm channel: {}", e);
                                continue;
                            }
                        };
                        match channel.send_message(&http_client, |m| {
                            let duration = HumanTime::from(
                                message
                                    .modul_termin
                                    .beginn
                                    .signed_duration_since(Utc::now()),
                            );
                            message
                                .to_create_message(m, &config)
                                .content(duration.to_text_en(
                                    chrono_humanize::Accuracy::Precise,
                                    chrono_humanize::Tense::Future,
                                ))
                        }) {
                            Ok(_) => {
                                log::info!("Sent reminder message to dm channel `{}`", channel.id.0)
                            }
                            Err(e) => log::error!("Failed to send reminder message: {}", e),
                        };
                    }
                }
            }
            drop(data_lock);
            thread::sleep(Duration::from_secs(300));
        }
    });

    start_client_join_handle.join().unwrap()?;
    reminder_join_handle.join().unwrap();

    Ok(())
}
