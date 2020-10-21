use crate::config::{self, Config};
use crate::modul::{Modul, ModulGruppe, ModulTermin, ModulTyp};
use anyhow::{Context, Result};
use chrono::{NaiveDate, NaiveDateTime, NaiveTime, TimeZone};
use reqwest::blocking::Client;
use scraper::{Html, Selector};
use std::{thread, time::Duration};

pub fn fetch_module(cfg: &Config) -> Result<Vec<Modul>> {
    log::info!("Fetching data from website");
    let client = Client::builder()
        .danger_accept_invalid_certs(true)
        .build()
        .context("Failed to create HTTP client")?;

    let mut module = Vec::<Modul>::new();

    for config::LinkData { lsf, .. } in cfg.links.to_vec() {
        let response = client
            .get(lsf)
            .send()
            .context("Failed to send HTTP request")?;
        let text = response.text().context("Failed to get text of response")?;
        let document = Html::parse_document(&text);

        let name_selector = Selector::parse("div > form > h1").unwrap();
        let name = document
            .select(&name_selector)
            .next()
            .context("Failed to get name")?
            .inner_html();
        let name = name.trim();
        let name = name.trim_end_matches(" - Einzelansicht");
        let modul_typ = ModulTyp::parse(name).context("Failed to parse name to `ModulTyp`")?;

        let table_selector =
            Selector::parse("table[summary='Übersicht über alle Veranstaltungstermine']").unwrap();
        for (i, table) in document.select(&table_selector).enumerate() {
            let row_selector = Selector::parse("tbody > tr:nth-child(n+2)").unwrap();
            for (j, row) in table.select(&row_selector).enumerate() {
                let expand_link_selector =
                    Selector::parse("td:first-child > a:first-child").unwrap();
                let expand_link = row
                    .select(&expand_link_selector)
                    .next()
                    .context("Failed to get expand button")?
                    .value()
                    .attr("href")
                    .context("Failed to get `href` attribute of expand button")?;

                let response = client
                    .get(expand_link)
                    .send()
                    .context("Failed to send HTTP request")?;
                let text = response.text().context("Failed to get text of response")?;
                let document = Html::parse_document(&text);
                let table = document
                    .select(&table_selector)
                    .nth(i)
                    .context("Failed to get table")?;

                let gruppe_selector = Selector::parse("caption.t_capt").unwrap();
                let gruppe = table
                    .select(&gruppe_selector)
                    .next()
                    .context("Failed to get group")?;
                let gruppe = gruppe
                    .text()
                    .next()
                    .context("Failed to get text of group")?
                    .trim();
                let gruppe = gruppe.trim_start_matches("Termine Gruppe: ");
                let gruppe = match gruppe {
                    "[unbenannt]" => None,
                    v => Some(
                        ModulGruppe::parse(v).context("Failed to parse group to `ModulGruppe`")?,
                    ),
                };

                let raum_selector = Selector::parse("td:nth-child(6) > a").unwrap();
                let raum = row
                    .select(&raum_selector)
                    .next()
                    .map(|v| v.inner_html().trim().to_owned());

                let bemerkung_selector = Selector::parse("td:nth-child(10)").unwrap();
                let bemerkung = row
                    .select(&bemerkung_selector)
                    .next()
                    .map(|v| v.inner_html().trim().to_owned());

                let zeit_selector = Selector::parse("td:nth-child(3)").unwrap();
                let zeit = row
                    .select(&zeit_selector)
                    .next()
                    .context("Failed to get time")?
                    .inner_html();
                let zeit = zeit.replace("&nbsp;", " ");
                let mut split = zeit.trim().split(" bis ");
                let zeit_beginn = NaiveTime::parse_from_str(
                    split.next().context("Failed to parse time")?,
                    "%H:%M",
                )
                .context("Failed to parse time")?;
                let zeit_ende = NaiveTime::parse_from_str(
                    split.next().context("Failed to parse time")?,
                    "%H:%M",
                )
                .context("Failed to parse time")?;
                let mut termine = Vec::new();
                let termine_row = table
                    .select(&row_selector)
                    .nth(j + 1)
                    .context("Failed to get dates")?;
                let termine_selector = Selector::parse("td > div > ul > li").unwrap();
                for termin in termine_row.select(&termine_selector) {
                    let date = NaiveDate::parse_from_str(
                        termin
                            .text()
                            .next()
                            .context("Failed to get text of date")?
                            .trim(),
                        "%d.%m.%Y",
                    )
                    .context("Failed to parse date")?;
                    let termin = ModulTermin {
                        beginn: chrono_tz::Europe::Berlin
                            .from_local_datetime(&NaiveDateTime::new(date, zeit_beginn))
                            .unwrap(),
                        ende: chrono_tz::Europe::Berlin
                            .from_local_datetime(&NaiveDateTime::new(date, zeit_ende))
                            .unwrap(),
                    };
                    termine.push(termin);
                }
                if termine.is_empty() {
                    log::warn!("Found entry without any dates");
                }

                match (
                    &raum,
                    module.iter_mut().find(|modul| {
                        modul.termine == termine && modul.typ == modul_typ && modul.gruppe == gruppe
                    }),
                ) {
                    (
                        Some(raum),
                        Some(Modul {
                            raum: Some(modul_raum),
                            ..
                        }),
                    ) => {
                        modul_raum.push_str(" & ");
                        modul_raum.push_str(raum);
                    }
                    _ => {
                        module.push(Modul {
                            typ: modul_typ.clone(),
                            gruppe,
                            termine,
                            raum,
                            bemerkung,
                        });
                    }
                };
            }
        }
        thread::sleep(Duration::from_secs(2));
    }
    log::info!("Successfully fetched data from website");
    Ok(module)
}
