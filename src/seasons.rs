use crate::{CLIENT, CONFIGS_BASE, DB};
use anyhow::{Context, Result};
use serde::Deserialize;
use zerocopy::{BigEndian, LayoutVerified, U16};

const NAME_TREE: &str = "sim_names_v1";
const SORT_TREE: &str = "sim_order_v1";

pub async fn load() -> Result<()> {
    let name_tree = DB.open_tree(NAME_TREE)?;
    let sort_tree = DB.open_tree(SORT_TREE)?;
    let response: Response = CLIENT
        .get(format!("{}/feed_season_list.json", CONFIGS_BASE))
        .send()
        .await?
        .json()
        .await?;
    for era in response.collection {
        sort_tree.insert(era.sim.as_bytes(), &era.index.to_be_bytes())?;

        let len = era.sim.len();
        let mut key = era.sim.into_bytes();
        for season in era.seasons {
            key.extend_from_slice(&(season - 1).to_be_bytes());
            name_tree.insert(&key, era.name.as_bytes())?;
            key.truncate(len);
        }
    }
    Ok(())
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Response {
    collection: Vec<SeasonEntry>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SeasonEntry {
    index: u16,
    name: String,
    sim: String,
    seasons: Vec<u16>,
}

pub fn iter() -> Result<impl Iterator<Item = Result<(String, u16)>>> {
    Ok(DB.open_tree(NAME_TREE)?.iter().map(|res| {
        res.map_err(anyhow::Error::from).and_then(|(key, _)| {
            let (sim, season): (&[u8], LayoutVerified<&[u8], U16<BigEndian>>) =
                LayoutVerified::new_unaligned_from_suffix(&*key).context("invalid key format")?;
            Ok((std::str::from_utf8(sim)?.to_owned(), season.get()))
        })
    }))
}

pub fn era_name(sim: &str, season: u16) -> Result<Option<String>> {
    let tree = DB.open_tree(NAME_TREE)?;
    let mut key = Vec::with_capacity(sim.len() + std::mem::size_of::<u16>());
    key.extend_from_slice(sim.as_bytes());
    key.extend_from_slice(&season.to_be_bytes());
    match tree.get(&key)? {
        Some(v) => Ok(Some(std::str::from_utf8(&v)?.to_owned())),
        None => Ok(None),
    }
}
