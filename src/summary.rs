use crate::game::{Game, Kind, Stats};
use crate::{seasons::Season, DB};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use sled::transaction::{
    ConflictableTransactionError, ConflictableTransactionResult, TransactionalTree,
};
use std::mem::{size_of, size_of_val};
use uuid::Uuid;
use zerocopy::{AsBytes, FromBytes, LayoutVerified};

pub const TREE: &str = "summary_v1";
pub const SEASON_TREE: &str = "season_summary_v1";

pub fn write_summary(
    tree: &TransactionalTree,
    season_tree: &TransactionalTree,
    game: &Game,
) -> ConflictableTransactionResult<(), serde_json::Error> {
    if game.kind == Kind::Special {
        return Ok(());
    }

    let mut totals = Stats::default();

    for team in game.teams() {
        let mut team_totals = Stats::default();

        for (id, stats) in team.stats.iter().map(|v| (*v.0, *v.1)) {
            for key in [
                build_key(team.id, id, &game.season, game.is_postseason()),
                build_key(id, team.id, &game.season, game.is_postseason()),
            ] {
                let mut value = match tree.get(&key)? {
                    None => Value::new(game.day),
                    Some(value) => serde_json::from_slice(&value)
                        .map_err(ConflictableTransactionError::Abort)?,
                };
                value.stats += stats;
                tree.insert(
                    key.as_slice(),
                    serde_json::to_vec(&value)
                        .map_err(ConflictableTransactionError::Abort)?
                        .as_slice(),
                )?;
            }

            if !game.is_postseason() {
                let key = build_season_key(&game.season, b'p', id);
                let mut value = match season_tree.get(&key)? {
                    None => SeasonValue::default(),
                    Some(value) => serde_json::from_slice(&value)
                        .map_err(ConflictableTransactionError::Abort)?,
                };
                value.stats += stats;
                value.team_id = team.id;
                value.team_abbr = team.name.shorthand.clone();
                if let Some(name) = team.player_names.get(&id) {
                    value.name = name.into();
                }
                season_tree.insert(
                    key.as_slice(),
                    serde_json::to_vec(&value)
                        .map_err(ConflictableTransactionError::Abort)?
                        .as_slice(),
                )?;
            }

            team_totals += stats;
        }

        team_totals.games_batted = 1;
        team_totals.games_pitched = 1;
        let key = build_season_key(
            &game.season,
            if game.is_postseason() { b'u' } else { b't' },
            team.id,
        );
        let mut value = match season_tree.get(&key)? {
            None => SeasonValue::default(),
            Some(value) => {
                serde_json::from_slice(&value).map_err(ConflictableTransactionError::Abort)?
            }
        };
        value.stats += team_totals;
        value.team_id = team.id;
        value.team_abbr = team.name.shorthand.clone();
        value.name = team.name.nickname.clone();
        season_tree.insert(
            key.as_slice(),
            serde_json::to_vec(&value)
                .map_err(ConflictableTransactionError::Abort)?
                .as_slice(),
        )?;

        totals += team_totals;
    }

    totals.games_batted = 1;
    totals.games_pitched = 1;
    let key = build_season_key(&game.season, b'l', Uuid::default());
    let mut value = match season_tree.get(&key)? {
        None => SeasonValue::default(),
        Some(value) => {
            serde_json::from_slice(&value).map_err(ConflictableTransactionError::Abort)?
        }
    };
    value.stats += totals;
    season_tree.insert(
        key.as_slice(),
        serde_json::to_vec(&value)
            .map_err(ConflictableTransactionError::Abort)?
            .as_slice(),
    )?;

    Ok(())
}

// =^..^=   =^..^=   =^..^=   =^..^=   =^..^=   =^..^=   =^..^=   =^..^=   =^..^=   =^..^=   =^..^=

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Summary {
    pub season: Season,
    pub first_day: u16,
    pub is_postseason: bool,
    pub player_id: Uuid,
    pub team_id: Uuid,
    pub stats: Stats,
}

pub fn player_summary(player_id: Uuid) -> Result<Vec<Summary>> {
    load_summary(player_id, true, None)
}

pub fn team_summary(team_id: Uuid, season: &Season) -> Result<Vec<Summary>> {
    load_summary(team_id, false, Some(season))
}

fn load_summary(
    scan_id: Uuid,
    scan_id_is_player: bool,
    season_filter: Option<&Season>,
) -> Result<Vec<Summary>> {
    let mut v = Vec::new();
    let tree = DB.open_tree(TREE)?;
    for row in tree.scan_prefix(scan_id.as_bytes()) {
        let (key, value) = row?;
        let (prefix, sim): (LayoutVerified<&[u8], KeyPrefix>, &[u8]) =
            LayoutVerified::new_from_prefix(&*key).context("invalid key format")?;
        let (player_id, team_id) = if scan_id_is_player {
            (prefix.scan_id, prefix.other_id)
        } else {
            (prefix.other_id, prefix.scan_id)
        };
        let sim = std::str::from_utf8(sim)?;
        let season = Season {
            sim: sim.into(),
            season: prefix.season,
        };
        if let Some(season_filter) = season_filter {
            if season_filter != &season {
                continue;
            }
        }
        let value: Value = serde_json::from_slice(&value)?;
        v.push(Summary {
            player_id: Uuid::from_bytes(player_id),
            team_id: Uuid::from_bytes(team_id),
            season,
            is_postseason: prefix.is_postseason > 0,
            stats: value.stats,
            first_day: value.first_day,
        });
    }
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
    let mut key = Vec::with_capacity(size_of::<KeyPrefix>() + season.sim.len());
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

// =^..^=   =^..^=   =^..^=   =^..^=   =^..^=   =^..^=   =^..^=   =^..^=   =^..^=   =^..^=   =^..^=

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct SeasonSummary {
    pub name: String,
    pub id: Uuid,
    pub team_id: Uuid,
    pub team_abbr: String,
    pub stats: Stats,
}

pub fn season_player_summary(season: &Season) -> Result<Vec<SeasonSummary>> {
    season_summary(season, b'p')
}

pub fn season_team_summary(season: &Season) -> Result<Vec<SeasonSummary>> {
    season_summary(season, b't')
}

fn season_summary(season: &Season, kind: u8) -> Result<Vec<SeasonSummary>> {
    let mut v = Vec::new();
    let tree = DB.open_tree(SEASON_TREE)?;
    let mut scan_key =
        Vec::with_capacity(season.sim.len() + size_of_val(&season.season) + size_of_val(&kind));
    scan_key.extend_from_slice(season.sim.as_bytes());
    scan_key.extend_from_slice(&season.season.to_ne_bytes());
    scan_key.push(kind);
    for row in tree.scan_prefix(scan_key) {
        let (key, value) = row?;
        let id = Uuid::from_slice(&key[key.len() - 16..])?;
        let value: SeasonValue = serde_json::from_slice(&value)?;
        v.push(SeasonSummary {
            name: value.name,
            id,
            team_id: value.team_id,
            team_abbr: value.team_abbr,
            stats: value.stats,
        });
    }
    v.sort_unstable();
    Ok(v)
}

pub fn team_totals(season: &Season, team_id: Uuid, is_postseason: bool) -> Result<Stats> {
    let tree = DB.open_tree(SEASON_TREE)?;
    let key = build_season_key(season, if is_postseason { b'u' } else { b't' }, team_id);
    Ok(match tree.get(&key)? {
        None => SeasonValue::default(),
        Some(value) => serde_json::from_slice(&value)?,
    }
    .stats)
}

pub fn league_totals(season: &Season) -> Result<Stats> {
    let tree = DB.open_tree(SEASON_TREE)?;
    let key = build_season_key(season, b'l', Uuid::default());
    Ok(match tree.get(&key)? {
        None => SeasonValue::default(),
        Some(value) => serde_json::from_slice(&value)?,
    }
    .stats)
}

fn build_season_key(season: &Season, kind: u8, id: Uuid) -> Vec<u8> {
    let mut key = Vec::with_capacity(
        season.sim.len() + size_of_val(&season.season) + size_of_val(&kind) + size_of_val(&id),
    );
    key.extend_from_slice(season.sim.as_bytes());
    key.extend_from_slice(&season.season.to_ne_bytes());
    key.push(kind);
    key.extend_from_slice(id.as_bytes());
    key
}

#[derive(Serialize, Deserialize, Default)]
struct SeasonValue {
    stats: Stats,
    name: String,
    team_id: Uuid,
    team_abbr: String,
}
