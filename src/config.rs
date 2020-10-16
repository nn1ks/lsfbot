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
    pub channel_id: u64,
    pub bot_token: String,
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
