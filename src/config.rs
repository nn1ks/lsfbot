use serde::Deserialize;
use serenity::prelude::TypeMapKey;
use std::sync::Arc;

#[derive(Deserialize)]
pub struct Config {
    pub discord: Discord,
    pub links: Links,
}

impl TypeMapKey for Config {
    type Value = Arc<Config>;
}

#[derive(Deserialize)]
pub struct Discord {
    pub bot_token: String,
    pub guild_id: u64,
    pub gruppe_1: Group,
    pub gruppe_2: Group,
    pub gruppe_3: Group,
    pub gruppe_4: Group,
}

#[derive(Deserialize)]
pub struct Group {
    pub channel_id: u64,
    pub role_id: u64,
}

#[derive(Deserialize)]
pub struct Links {
    pub mathematik1: LinkData,
    pub programmiertechnik1: LinkData,
    pub softwaremodellierung: LinkData,
    pub digitaltechnik: LinkData,
}

impl Links {
    pub fn to_vec(&self) -> Vec<&LinkData> {
        vec![
            &self.mathematik1,
            &self.programmiertechnik1,
            &self.softwaremodellierung,
            &self.digitaltechnik,
        ]
    }
}

#[derive(Deserialize)]
pub struct LinkData {
    pub lsf: String,
    pub vorlesungen: Option<String>,
    pub uebungen: Option<String>,
}
