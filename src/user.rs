use crate::modul::ModulGruppe;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use serenity::{http::client::Http, model::id::UserId};
use std::fs::OpenOptions;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

#[derive(Deserialize, Serialize)]
struct Config {
    user: Vec<User>,
}

impl Config {
    fn new<P: AsRef<Path>>(file_path: P) -> Result<Self> {
        let mut file = OpenOptions::new().read(true).open(file_path)?;
        let mut string = String::new();
        file.read_to_string(&mut string)?;
        Ok(toml::from_str(&string)?)
    }
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(transparent)]
pub struct Duration {
    pub minutes: u64,
}

#[derive(Clone, Deserialize, Serialize)]
pub struct User {
    pub id: UserId,
    pub gruppe: Option<ModulGruppe>,
    pub enabled: bool,
    pub send_before: Option<Duration>,
    pub send_after_previous: bool,
}

pub struct Users {
    file_path: PathBuf,
    config: Config,
}

impl Users {
    pub fn new(file_path: PathBuf) -> Result<Self> {
        Ok(Self {
            config: Config::new(&file_path)?,
            file_path,
        })
    }

    pub fn refresh(&mut self) -> Result<()> {
        self.config = Config::new(&self.file_path)?;
        Ok(())
    }

    pub fn get_all(&self) -> &[User] {
        &self.config.user
    }

    pub fn get(&self, user_id: UserId) -> Option<&User> {
        self.config.user.iter().find(|v| v.id == user_id)
    }

    fn write(&mut self) -> Result<()> {
        let mut file = OpenOptions::new().write(true).open(&self.file_path)?;
        let string = toml::to_string_pretty(&self.config)?;
        file.write_all(string.as_bytes())?;
        Ok(())
    }

    pub fn enable_or_add(
        &mut self,
        user_id: UserId,
        http: &Http,
        config: &crate::Config,
    ) -> Result<()> {
        match self.config.user.iter_mut().find(|user| user.id == user_id) {
            Some(v) => v.enabled = true,
            None => {
                let user = user_id.to_user(http)?;
                let gruppe = if user.has_role(
                    http,
                    config.discord.guild_id,
                    config.discord.gruppe_1.role_id,
                )? {
                    Some(ModulGruppe::Gruppe1)
                } else if user.has_role(
                    http,
                    config.discord.guild_id,
                    config.discord.gruppe_2.role_id,
                )? {
                    Some(ModulGruppe::Gruppe2)
                } else if user.has_role(
                    http,
                    config.discord.guild_id,
                    config.discord.gruppe_3.role_id,
                )? {
                    Some(ModulGruppe::Gruppe3)
                } else if user.has_role(
                    http,
                    config.discord.guild_id,
                    config.discord.gruppe_4.role_id,
                )? {
                    Some(ModulGruppe::Gruppe4)
                } else {
                    None
                };
                self.config.user.push(User {
                    id: user_id,
                    gruppe,
                    enabled: true,
                    send_before: Some(Duration { minutes: 30 }),
                    send_after_previous: false,
                })
            }
        };
        self.write()
    }

    pub fn disable(&mut self, user_id: UserId) -> Result<()> {
        if let Some(v) = self.config.user.iter_mut().find(|user| user.id == user_id) {
            v.enabled = false;
        }
        self.write()
    }

    pub fn remove(&mut self, user_id: UserId) -> Result<()> {
        if let Some(i) = self.config.user.iter().position(|user| user.id == user_id) {
            self.config.user.remove(i);
        }
        self.write()
    }

    pub fn set_send_before(&mut self, user_id: UserId, value: Option<Duration>) -> Result<()> {
        if let Some(v) = self.config.user.iter_mut().find(|user| user.id == user_id) {
            v.send_before = value;
        }
        self.write()
    }

    pub fn set_send_after(&mut self, user_id: UserId, value: bool) -> Result<()> {
        if let Some(v) = self.config.user.iter_mut().find(|user| user.id == user_id) {
            v.send_after_previous = value;
        }
        self.write()
    }
}
