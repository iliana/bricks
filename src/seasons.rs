use crate::{CLIENT, CONFIGS_BASE, DB};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::fmt::{self, Display};
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

// =^..^=   =^..^=   =^..^=   =^..^=   =^..^=   =^..^=   =^..^=   =^..^=   =^..^=   =^..^=   =^..^=

#[derive(Debug, Default, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct Season {
    pub sim: String,
    pub season: u16,
}

impl Season {
    pub fn iter() -> Result<impl Iterator<Item = Result<Season>>> {
        Ok(DB.open_tree(NAME_TREE)?.iter().map(|res| {
            res.map_err(anyhow::Error::from).and_then(|(key, _)| {
                let (sim, season): (&[u8], LayoutVerified<&[u8], U16<BigEndian>>) =
                    LayoutVerified::new_unaligned_from_suffix(&*key)
                        .context("invalid key format")?;
                Ok(Season {
                    sim: std::str::from_utf8(sim)?.to_owned(),
                    season: season.get(),
                })
            })
        }))
    }

    pub fn era_name(&self) -> Result<Option<String>> {
        let tree = DB.open_tree(NAME_TREE)?;
        let mut key = Vec::with_capacity(self.sim.len() + std::mem::size_of::<u16>());
        key.extend_from_slice(self.sim.as_bytes());
        key.extend_from_slice(&self.season.to_be_bytes());
        match tree.get(&key)? {
            Some(v) => Ok(Some(std::str::from_utf8(&v)?.to_owned())),
            None => Ok(None),
        }
    }

    fn sim_cmp(&self, other: &Season) -> Ordering {
        let tree = match DB.open_tree(SORT_TREE) {
            Ok(tree) => tree,
            Err(_) => return self.sim.cmp(&other.sim),
        };
        let idx_a = match tree.get(&self.sim) {
            Ok(idx) => idx,
            Err(_) => return self.sim.cmp(&other.sim),
        };
        let idx_b = match tree.get(&other.sim) {
            Ok(idx) => idx,
            Err(_) => return self.sim.cmp(&other.sim),
        };
        match (idx_a, idx_b) {
            (Some(idx_a), Some(idx_b)) => idx_a.cmp(&idx_b),
            (Some(_), None) => Ordering::Less,
            (None, Some(_)) => Ordering::Greater,
            (None, None) => self.sim.cmp(&other.sim),
        }
    }
}

impl Display for Season {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.era_name().ok() {
            Some(Some(name)) => write!(f, "{}", name)?,
            _ => write!(f, "{}", self.sim)?,
        }
        if f.alternate() {
            write!(f, "/S{}", self.season + 1)
        } else {
            write!(f, ", Season {}", self.season + 1)
        }
    }
}

impl PartialOrd for Season {
    fn partial_cmp(&self, other: &Season) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Season {
    fn cmp(&self, other: &Season) -> Ordering {
        self.sim_cmp(other).then(self.season.cmp(&other.season))
    }
}
