use crate::routes::season::{
    rocket_uri_macro_season_player_batting, rocket_uri_macro_season_player_pitching,
    rocket_uri_macro_season_team_batting, rocket_uri_macro_season_team_pitching,
};
use crate::routes::team::rocket_uri_macro_team;
use crate::DB;
use anyhow::{Context, Result};
use rocket::uri;
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::fmt::{self, Display};
use std::mem::size_of_val;
use uuid::Uuid;
use zerocopy::{BigEndian, LayoutVerified, U16};

const NAME_TREE: &str = "sim_names_v1";
const SORT_TREE: &str = "sim_order_v1";
pub const RECORDED_TREE: &str = "recorded_seasons_v1";

pub async fn load() -> Result<()> {
    let name_tree = DB.open_tree(NAME_TREE)?;
    let sort_tree = DB.open_tree(SORT_TREE)?;
    let response: Response = serde_json::from_str(include_str!("../feed_season_list.json"))?;
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

#[derive(Debug, Default, Clone, PartialEq, Eq, Hash, Deserialize, Serialize)]
pub struct Season {
    pub sim: String,
    pub season: u16,
}

impl Season {
    fn read_from_tree(tree: &'static str) -> Result<Vec<Season>> {
        let mut v = DB
            .open_tree(tree)?
            .iter()
            .map(|res| {
                res.map_err(anyhow::Error::from).and_then(|(key, _)| {
                    let (sim, season): (&[u8], LayoutVerified<&[u8], U16<BigEndian>>) =
                        LayoutVerified::new_unaligned_from_suffix(&*key)
                            .context("invalid key format")?;
                    Ok(Season {
                        sim: std::str::from_utf8(sim)?.to_owned(),
                        season: season.get(),
                    })
                })
            })
            .collect::<Result<Vec<_>>>()?;
        v.sort_unstable();
        Ok(v)
    }

    pub fn known() -> Result<Vec<Season>> {
        Season::read_from_tree(NAME_TREE)
    }

    pub fn recorded() -> Result<Vec<Season>> {
        Season::read_from_tree(RECORDED_TREE)
    }

    pub fn era_name(&self) -> Result<Option<String>> {
        let tree = DB.open_tree(NAME_TREE)?;
        let mut key = Vec::with_capacity(self.sim.len() + size_of_val(&self.season));
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

    pub fn uri(&self, is_batting: &bool, is_players: &bool) -> String {
        if *is_players {
            if *is_batting {
                uri!(season_player_batting(sim = &self.sim, season = self.season))
            } else {
                uri!(season_player_pitching(
                    sim = &self.sim,
                    season = self.season
                ))
            }
        } else if *is_batting {
            uri!(season_team_batting(sim = &self.sim, season = self.season))
        } else {
            uri!(season_team_pitching(sim = &self.sim, season = self.season))
        }
        .to_string()
    }

    pub fn team_uri(&self, id: &&Uuid) -> String {
        uri!(team(id = **id, sim = &self.sim, season = self.season)).to_string()
    }

    pub fn selected(&self, other: &Season) -> &'static str {
        if self == other {
            "selected"
        } else {
            ""
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
