use crate::game::{Game, Stats};
use crate::{seasons::Season, DB};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use sled::transaction::{
    ConflictableTransactionError, ConflictableTransactionResult, TransactionalTree,
};
use uuid::Uuid;
use zerocopy::{AsBytes, FromBytes, LayoutVerified};

pub const TREE: &str = "summary_v1";

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Summary {
    pub season: Season,
    pub first_day: u16,
    pub is_postseason: bool,
    pub player_id: Uuid,
    pub team_id: Uuid,
    pub stats: Stats,
}

pub fn write_summary(
    tree: &TransactionalTree,
    game: &Game,
) -> ConflictableTransactionResult<(), serde_json::Error> {
    for team in game.teams() {
        for (id, stats) in &team.stats {
            for key in [
                build_key(team.id, *id, &game.season, game.is_postseason),
                build_key(*id, team.id, &game.season, game.is_postseason),
            ] {
                let mut value = match tree.get(&key)? {
                    None => Value::new(game.day),
                    Some(value) => serde_json::from_slice(&value)
                        .map_err(ConflictableTransactionError::Abort)?,
                };
                value.stats += *stats;
                tree.insert(
                    key.as_slice(),
                    serde_json::to_vec(&value)
                        .map_err(ConflictableTransactionError::Abort)?
                        .as_slice(),
                )?;
            }
        }
    }

    Ok(())
}

pub fn player_summary(player_id: Uuid) -> Result<Vec<Summary>> {
    load_summary(player_id, true, |_, _| true)
}

pub fn team_summary(team_id: Uuid, sim: &str, season: u16) -> Result<Vec<Summary>> {
    load_summary(team_id, false, move |prefix, the_sim| {
        the_sim == sim && prefix.season == season
    })
}

fn load_summary<F>(scan_id: Uuid, scan_id_is_player: bool, filter: F) -> Result<Vec<Summary>>
where
    F: Fn(KeyPrefix, &str) -> bool,
{
    let mut v = DB
        .open_tree(TREE)?
        .scan_prefix(scan_id.as_bytes())
        .map(move |res| {
            res.map_err(anyhow::Error::from).and_then(|(key, value)| {
                let (prefix, sim): (LayoutVerified<&[u8], KeyPrefix>, &[u8]) =
                    LayoutVerified::new_from_prefix(&*key).context("invalid key format")?;
                let (player_id, team_id) = if scan_id_is_player {
                    (prefix.scan_id, prefix.other_id)
                } else {
                    (prefix.other_id, prefix.scan_id)
                };
                let sim = std::str::from_utf8(sim)?;
                if !filter(*prefix, sim) {
                    return Ok(None);
                }
                let season = Season {
                    sim: sim.into(),
                    season: prefix.season,
                };
                let value: Value = serde_json::from_slice(&value)?;
                Ok(Some(Summary {
                    player_id: Uuid::from_bytes(player_id),
                    team_id: Uuid::from_bytes(team_id),
                    season,
                    is_postseason: prefix.is_postseason > 0,
                    stats: value.stats,
                    first_day: value.first_day,
                }))
            })
        })
        .filter_map(|res| res.transpose())
        .collect::<Result<Vec<_>>>()?;
    v.sort_unstable();
    Ok(v)
}

#[derive(Clone, Copy, AsBytes, FromBytes)]
#[repr(C)]
struct KeyPrefix {
    scan_id: [u8; 16],
    other_id: [u8; 16],
    season: u16,
    is_postseason: u16,
}

fn build_key(scan_id: Uuid, other_id: Uuid, season: &Season, is_postseason: bool) -> Vec<u8> {
    let mut key = Vec::with_capacity(std::mem::size_of::<KeyPrefix>() + season.sim.len());
    key.extend_from_slice(
        KeyPrefix {
            scan_id: *scan_id.as_bytes(),
            other_id: *other_id.as_bytes(),
            season: season.season,
            is_postseason: if is_postseason { 1 } else { 0 },
        }
        .as_bytes(),
    );
    key.extend_from_slice(season.sim.as_bytes());
    key
}

#[derive(Serialize, Deserialize)]
struct Value {
    stats: Stats,
    first_day: u16,
}

impl Value {
    fn new(first_day: u16) -> Value {
        Value {
            stats: Stats::default(),
            first_day,
        }
    }
}
