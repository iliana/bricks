use crate::chronicler;
use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Deserializer};
use uuid::Uuid;

pub async fn load(id: Uuid, at: DateTime<Utc>) -> Result<Option<Team>> {
    chronicler::load("team", id, at).await
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Team {
    pub full_name: String,
    pub nickname: String,
    pub shorthand: String,
    #[serde(deserialize_with = "deserialize_emoji")]
    pub emoji: String,
    pub lineup: Vec<Uuid>,
}

fn deserialize_emoji<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    Ok(s.strip_prefix("0x")
        .and_then(|hex| u32::from_str_radix(hex, 16).ok())
        .and_then(|s| char::try_from(s).ok())
        .map(|c| c.to_string())
        .unwrap_or(s))
}
