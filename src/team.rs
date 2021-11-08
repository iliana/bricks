use crate::chronicler::Versions;
use crate::{cache, CHRONICLER_BASE, CLIENT};
use anyhow::{Context, Result};
use chrono::{DateTime, SecondsFormat, Utc};
use serde::Deserialize;
use serde_json::value::RawValue;
use std::ops::Range;
use uuid::Uuid;

const CACHE_KIND: &str = "ChroniclerTeam";

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Team {
    pub(crate) full_name: String,
    pub(crate) nickname: String,
    pub(crate) shorthand: String,
    pub(crate) emoji: String,
    pub(crate) lineup: Vec<Uuid>,
}

pub(crate) async fn load_team(team_id: &str, at: DateTime<Utc>) -> Result<Team> {
    Ok(
        if let Some(cached) = cache::load(CACHE_KIND, team_id, Some(at)).await? {
            serde_json::from_slice(&cached)?
        } else {
            let response = CLIENT
                .get(format!(
                    "{}/v2/entities?type=Team&id={}&at={}",
                    CHRONICLER_BASE,
                    team_id,
                    at.to_rfc3339_opts(SecondsFormat::AutoSi, true),
                ))
                .send()
                .await?;
            let response_time: DateTime<Utc> = DateTime::parse_from_rfc2822(
                response
                    .headers()
                    .get("date")
                    .context("no date header in response")?
                    .to_str()?,
            )?
            .into();
            let versions: Versions<Box<RawValue>> = response.json().await?;
            let version = versions
                .items
                .into_iter()
                .next()
                .with_context(|| format!("team id {} not found", team_id))?;
            let team = serde_json::from_str(version.data.get())?;
            cache::store(
                CACHE_KIND,
                team_id,
                version.data.get().as_bytes(),
                Some(Range {
                    start: version.valid_from,
                    end: version.valid_to.unwrap_or(response_time),
                }),
            )
            .await?;
            team
        },
    )
}
