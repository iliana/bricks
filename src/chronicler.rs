use crate::{CHRONICLER_BASE, CLIENT, DB};
use anyhow::{Context, Result};
use chrono::{DateTime, SecondsFormat, TimeZone, Utc};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::value::RawValue;
use uuid::Uuid;
use zerocopy::{AsBytes, BigEndian, FromBytes, I64};

pub async fn load<T: DeserializeOwned>(
    ty: &'static str,
    id: Uuid,
    at: DateTime<Utc>,
) -> Result<Option<T>> {
    let tree = DB.open_tree(format!("cache_chronicler_v1_{}", ty.to_ascii_lowercase()))?;

    if let Some((key, value)) = tree.get_lt(Key::new(id, at).as_bytes())? {
        if let Some(key) = Key::read_from(&*key) {
            let value: Value<T> = serde_json::from_slice(&value)?;
            if key.id == *id.as_bytes() && key.valid_from() <= at && at < value.valid_to {
                return Ok(Some(value.data));
            }
        }
    }

    let response = CLIENT
        .get(format!(
            "{}/v2/entities?type={}&id={}&at={}",
            CHRONICLER_BASE,
            ty,
            id,
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

    let versions: Versions = response.json().await?;
    let version = match versions.items.into_iter().next() {
        Some(v) => v,
        None => return Ok(None),
    };
    let value = serde_json::from_str(version.data.get())?;

    tree.insert(
        Key::new(id, version.valid_from).as_bytes(),
        serde_json::to_vec(&Value {
            valid_to: version.valid_to.unwrap_or(response_time),
            data: version.data,
        })?,
    )?;

    Ok(Some(value))
}

/// Updates chronicler cache with all entities of a certain type, returning all entities
pub async fn update_and_load_all<T: DeserializeOwned>(ty: &'static str) -> Result<Vec<(Key, T)>> {
    let tree = DB.open_tree(format!("cache_chronicler_v1_{}", ty.to_ascii_lowercase()))?;

    let mut results = Vec::new();

    let response = CLIENT
        .get(format!("{}/v2/entities?type={}", CHRONICLER_BASE, ty,))
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

    let versions: Versions = response.json().await?;

    for version in versions.items {
        let key = Key::new(version.entity_id, version.valid_from);
        if let Some(value) = tree.get(key.as_bytes())? {
            results.push((key, serde_json::from_slice::<Value<T>>(&value)?.data));
        } else {
            let value = serde_json::from_str(version.data.get())?;
            results.push((key, value));

            tree.insert(
                key.as_bytes(),
                serde_json::to_vec(&Value {
                    valid_to: version.valid_to.unwrap_or(response_time),
                    data: version.data,
                })?,
            )?;
        }
    }

    Ok(results)
}

#[derive(Copy, Clone, AsBytes, FromBytes)]
#[repr(C)]
pub struct Key {
    id: [u8; 16],
    valid_from: I64<BigEndian>,
}

impl Key {
    pub fn new(id: Uuid, valid_from: DateTime<Utc>) -> Key {
        Key {
            id: *id.as_bytes(),
            valid_from: valid_from.timestamp_nanos().into(),
        }
    }

    pub fn valid_from(&self) -> DateTime<Utc> {
        Utc.timestamp_nanos(self.valid_from.into())
    }
}

#[derive(Debug, Deserialize, Serialize)]
struct Value<T> {
    valid_to: DateTime<Utc>,
    data: T,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Versions {
    items: Vec<Version>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Version {
    entity_id: Uuid,
    valid_from: DateTime<Utc>,
    valid_to: Option<DateTime<Utc>>,
    data: Box<RawValue>,
}
