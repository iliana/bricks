use crate::game::{Game, Stats};
use crate::{seasons, DB};
use anyhow::{Context, Result};
use sled::transaction::{
    ConflictableTransactionError, ConflictableTransactionResult, TransactionalTree,
};
use uuid::Uuid;
use zerocopy::{AsBytes, FromBytes, LayoutVerified};

pub const TREE: &str = "summary_v1";

pub fn write_summary(
    tree: &TransactionalTree,
    game: &Game,
) -> ConflictableTransactionResult<(), serde_json::Error> {
    let is_postseason = game.day + 1 > 99;

    for team in game.teams() {
        for (id, stats) in &team.stats {
            for key in [
                build_key(team.id, *id, &game.sim, game.season, is_postseason),
                build_key(*id, team.id, &game.sim, game.season, is_postseason),
            ] {
                let new = match tree.get(&key)? {
                    None => Stats::default(),
                    Some(value) => serde_json::from_slice(&value)
                        .map_err(ConflictableTransactionError::Abort)?,
                } + *stats;
                tree.insert(
                    key.as_slice(),
                    serde_json::to_vec(&new)
                        .map_err(ConflictableTransactionError::Abort)?
                        .as_slice(),
                )?;
            }
        }
    }

    Ok(())
}

pub fn player_summary(player_id: Uuid) -> Result<impl Iterator<Item = Result<Summary>>> {
    load_summary(player_id, true)
}

fn load_summary(
    scan_id: Uuid,
    scan_id_is_player: bool,
) -> Result<impl Iterator<Item = Result<Summary>>> {
    Ok(DB
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
                let era = seasons::era_name(sim, prefix.season)?.unwrap_or_else(|| sim.to_owned());
                Ok(Summary {
                    player_id: Uuid::from_bytes(player_id),
                    team_id: Uuid::from_bytes(team_id),
                    era,
                    season: prefix.season,
                    is_postseason: prefix.is_postseason > 0,
                    stats: serde_json::from_slice(&value)?,
                })
            })
        }))
}

#[derive(Debug)]
pub struct Summary {
    pub player_id: Uuid,
    pub team_id: Uuid,
    pub era: String,
    pub season: u16,
    pub is_postseason: bool,
    pub stats: Stats,
}

#[derive(Clone, Copy, AsBytes, FromBytes)]
#[repr(C)]
struct KeyPrefix {
    scan_id: [u8; 16],
    other_id: [u8; 16],
    season: u16,
    is_postseason: u16,
}

fn build_key(
    scan_id: Uuid,
    other_id: Uuid,
    sim: &str,
    season: u16,
    is_postseason: bool,
) -> Vec<u8> {
    let mut key = Vec::with_capacity(std::mem::size_of::<KeyPrefix>() + sim.len());
    key.extend_from_slice(
        KeyPrefix {
            scan_id: *scan_id.as_bytes(),
            other_id: *other_id.as_bytes(),
            season,
            is_postseason: if is_postseason { 1 } else { 0 },
        }
        .as_bytes(),
    );
    key.extend_from_slice(sim.as_bytes());
    key
}