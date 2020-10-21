use crate::config::Config;
use anyhow::{anyhow, Result};
use chrono::{DateTime, Datelike, Weekday};
use chrono_tz::Tz;
use derive_more::Display;
use serenity::{builder::CreateMessage, utils::Color};

pub struct MessageData<'m> {
    pub modul: &'m Modul,
    pub modul_termin: &'m ModulTermin,
}

impl MessageData<'_> {
    pub fn to_create_message<'a, 'b>(
        &self,
        msg: &'b mut CreateMessage<'a>,
        cfg: &Config,
    ) -> &'b mut CreateMessage<'a> {
        msg.embed(|mut embed| {
            embed = embed
                .title(self.modul.title())
                .color(self.modul.embed_color())
                .description(format!(
                    "{} {} - {}",
                    match self.modul_termin.beginn.weekday() {
                        Weekday::Mon => "Montag",
                        Weekday::Tue => "Dienstag",
                        Weekday::Wed => "Mittwoch",
                        Weekday::Thu => "Donnerstag",
                        Weekday::Fri => "Freitag",
                        Weekday::Sat => "Samstag",
                        Weekday::Sun => "Sonntag",
                    },
                    self.modul_termin.beginn.format("%H:%M"),
                    self.modul_termin.ende.format("%H:%M")
                ));
            if let Some(online_link) = self.modul.online_link(cfg) {
                embed = embed.field("Online", online_link, false);
            }
            if let Some(raum) = &self.modul.raum {
                embed = embed.field("Raum", raum, false);
            }
            if let Some(bemerkung) = &self.modul.bemerkung {
                embed = embed.field("Bemerkung", bemerkung, false);
            }
            embed
        })
    }
}

#[derive(Debug, Eq, PartialEq)]
pub struct Modul {
    pub typ: ModulTyp,
    pub gruppe: Option<ModulGruppe>,
    pub termine: Vec<ModulTermin>,
    pub raum: Option<String>,
    pub bemerkung: Option<String>,
}

impl Modul {
    pub fn messages<F>(&self, filter: F) -> Vec<MessageData>
    where
        F: Fn(&ModulTermin) -> bool,
    {
        self.termine
            .iter()
            .filter(|termin| filter(termin))
            .map(|termin| MessageData {
                modul: self,
                modul_termin: termin,
            })
            .collect()
    }

    fn title(&self) -> String {
        match &self.gruppe {
            Some(gruppe) => format!("{} ({})", self.typ, gruppe),
            None => self.typ.to_string(),
        }
    }

    fn online_link(&self, cfg: &Config) -> Option<String> {
        let link_data = match self.typ {
            ModulTyp::Mathematik1 => &cfg.links.mathematik1,
            ModulTyp::Programmiertechnik1 => &cfg.links.programmiertechnik1,
            ModulTyp::Softwaremodellierung => &cfg.links.softwaremodellierung,
            ModulTyp::Digitaltechnik => &cfg.links.digitaltechnik,
        };
        match self.gruppe {
            Some(_) => link_data.uebungen.clone(),
            None => link_data.vorlesungen.clone(),
        }
    }

    fn embed_color(&self) -> Color {
        match self.typ {
            ModulTyp::Mathematik1 => Color::BLUE,
            ModulTyp::Programmiertechnik1 => Color::ORANGE,
            ModulTyp::Softwaremodellierung => Color::PURPLE,
            ModulTyp::Digitaltechnik => Color::DARK_GREEN,
        }
    }
}

#[derive(Clone, Debug, Display, Eq, PartialEq)]
pub enum ModulTyp {
    #[display(fmt = "Mathematik 1")]
    Mathematik1,
    #[display(fmt = "Programmiertechnik 1")]
    Programmiertechnik1,
    #[display(fmt = "Softwaremodellierung")]
    Softwaremodellierung,
    #[display(fmt = "Digitaltechnik")]
    Digitaltechnik,
}

impl ModulTyp {
    pub fn parse(input: &str) -> Result<Self> {
        match input {
            "AIN1 Mathematik 1" => Ok(Self::Mathematik1),
            "AIN1 Programmiertechnik1 - findet online statt" => Ok(Self::Programmiertechnik1),
            "AIN1 Softwaremodellierung" => Ok(Self::Softwaremodellierung),
            "AIN1 Digitaltechnik" => Ok(Self::Digitaltechnik),
            _ => Err(anyhow!("Unknown name `{}`", input)),
        }
    }
}

#[derive(Debug, Display, Eq, PartialEq)]
pub enum ModulGruppe {
    #[display(fmt = "Gruppe 1")]
    Gruppe1,
    #[display(fmt = "Gruppe 2")]
    Gruppe2,
    #[display(fmt = "Gruppe 3")]
    Gruppe3,
    #[display(fmt = "Gruppe 4")]
    Gruppe4,
}

impl ModulGruppe {
    pub fn parse(input: &str) -> Result<Self> {
        match input {
            "Gruppe 1" => Ok(Self::Gruppe1),
            "Gruppe 2" => Ok(Self::Gruppe2),
            "Gruppe 3" => Ok(Self::Gruppe3),
            "Gruppe 4" => Ok(Self::Gruppe4),
            _ => Err(anyhow!("Unknown group `{}`", input)),
        }
    }
}

#[derive(Debug, Eq, PartialEq)]
pub struct ModulTermin {
    pub beginn: DateTime<Tz>,
    pub ende: DateTime<Tz>,
}
