use crate::modul::ModulGruppe;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use serenity::{model::id::UserId, CacheAndHttp};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::{fs::OpenOptions, sync::Arc};

#[derive(Deserialize, Serialize)]
struct Config {
    #[serde(default)]
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
    users_config: Config,
    app_config: Arc<crate::Config>,
    cache_and_http: Arc<CacheAndHttp>,
}

impl Users {
    pub fn new(
        file_path: PathBuf,
        app_config: Arc<crate::Config>,
        cache_and_http: Arc<CacheAndHttp>,
    ) -> Result<Self> {
        Ok(Self {
            users_config: Config::new(&file_path)?,
            file_path,
            app_config,
            cache_and_http,
        })
    }

    pub fn refresh(&mut self) -> Result<()> {
        self.users_config = Config::new(&self.file_path)?;
        Ok(())
    }

    pub fn get_all(&self) -> &[User] {
        &self.users_config.user
    }

    pub fn get(&self, user_id: UserId) -> Option<&User> {
        self.users_config.user.iter().find(|v| v.id == user_id)
    }

    fn write(&mut self) -> Result<()> {
        let mut file = OpenOptions::new().write(true).open(&self.file_path)?;
        let string = toml::to_string_pretty(&self.users_config)?;
        file.write_all(string.as_bytes())?;
        Ok(())
    }

    fn get_mut_or_add(&mut self, user_id: UserId) -> Result<&mut User> {
        if let Some(i) = self
            .users_config
            .user
            .iter()
            .position(|user| user.id == user_id)
        {
            return Ok(&mut self.users_config.user[i]);
        }
        let user = user_id.to_user(&self.cache_and_http)?;
        let user_has_role = |role_id: u64| {
            user.has_role(
                &self.cache_and_http,
                self.app_config.discord.guild_id,
                role_id,
            )
        };
        let gruppe = if user_has_role(self.app_config.discord.gruppe_1.role_id)? {
            Some(ModulGruppe::Gruppe1)
        } else if user_has_role(self.app_config.discord.gruppe_2.role_id)? {
            Some(ModulGruppe::Gruppe2)
        } else if user_has_role(self.app_config.discord.gruppe_3.role_id)? {
            Some(ModulGruppe::Gruppe3)
        } else if user_has_role(self.app_config.discord.gruppe_4.role_id)? {
            Some(ModulGruppe::Gruppe4)
        } else {
            None
        };
        let users = &mut self.users_config.user;
        users.push(User {
            id: user_id,
            gruppe,
            enabled: false,
            send_before: Some(Duration { minutes: 30 }),
            send_after_previous: false,
        });
        Ok(self
            .users_config
            .user
            .iter_mut()
            .find(|user| user.id == user_id)
            .unwrap())
    }

    pub fn enable(&mut self, user_id: UserId) -> Result<()> {
        let user = self.get_mut_or_add(user_id)?;
        user.enabled = true;
        self.write()
    }

    pub fn disable(&mut self, user_id: UserId) -> Result<()> {
        if let Some(v) = self
            .users_config
            .user
            .iter_mut()
            .find(|user| user.id == user_id)
        {
            v.enabled = false;
        }
        self.write()
    }

    pub fn remove(&mut self, user_id: UserId) -> Result<()> {
        if let Some(i) = self
            .users_config
            .user
            .iter()
            .position(|user| user.id == user_id)
        {
            self.users_config.user.remove(i);
        }
        self.write()
    }

    pub fn set_send_before(&mut self, user_id: UserId, value: Option<Duration>) -> Result<()> {
        let user = self.get_mut_or_add(user_id)?;
        user.send_before = value;
        self.write()
    }

    pub fn set_send_after(&mut self, user_id: UserId, value: bool) -> Result<()> {
        let user = self.get_mut_or_add(user_id)?;
        user.send_after_previous = value;
        self.write()
    }
}
