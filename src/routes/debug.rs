use crate::debug::LogEntry;
use crate::game::{DEBUG_TREE, GAME_STATS_TREE};
use crate::routes::ResponseResult;
use crate::DB;
use anyhow::Result;
use askama::Template;
use rocket::get;
use rocket::response::content::Html;
use std::borrow::Cow;
use std::collections::BTreeMap;
use uuid::Uuid;

#[get("/errors")]
pub fn errors() -> ResponseResult<Html<String>> {
    Ok(Html(
        ErrorDashboard {
            errors: load_errors()?,
        }
        .render()
        .map_err(anyhow::Error::from)?,
    ))
}

fn load_errors() -> Result<BTreeMap<String, Vec<Uuid>>> {
    let debug_tree = DB.open_tree(DEBUG_TREE)?;
    let stats_tree = DB.open_tree(GAME_STATS_TREE)?;
    let mut map: BTreeMap<String, Vec<Uuid>> = BTreeMap::new();
    for row in debug_tree.iter() {
        let (key, value) = row?;
        if !stats_tree.contains_key(&key)? {
            let debug: Vec<LogEntry> = serde_json::from_slice(&value)?;
            if let Some(LogEntry::Err { error, .. }) = debug.last() {
                let error = error.lines().last().unwrap().trim();
                map.entry(error.into())
                    .or_default()
                    .push(Uuid::from_slice(&key)?);
            }
        }
    }
    Ok(map)
}

#[derive(Template)]
#[template(path = "error_dashboard.html")]
struct ErrorDashboard {
    errors: BTreeMap<String, Vec<Uuid>>,
}

#[get("/game/<id>/debug")]
pub fn debug(id: Uuid) -> ResponseResult<Option<Html<String>>> {
    Ok(match load_debug(id)? {
        Some(log) => Some(Html(
            GameDebugPage { id, log }
                .render()
                .map_err(anyhow::Error::from)?,
        )),
        None => None,
    })
}

fn load_debug(id: Uuid) -> Result<Option<Vec<LogEntry>>> {
    let tree = DB.open_tree(DEBUG_TREE)?;
    Ok(match tree.get(id.as_bytes())? {
        Some(value) => Some(serde_json::from_slice(&value)?),
        None => None,
    })
}

impl LogEntry {
    fn description(&self) -> &str {
        match self {
            LogEntry::Ok { description, .. } => description,
            LogEntry::Err { description, .. } => description.as_deref().unwrap_or("[end of feed]"),
        }
    }

    fn info(&self) -> Cow<'_, str> {
        match self {
            LogEntry::Ok { patch, .. } => patch
                .0
                .iter()
                .map(|p| {
                    serde_json::to_string(p)
                        .unwrap_or_else(|_| "[failed to serialize patch]".to_string())
                })
                .collect::<Vec<_>>()
                .join("\n")
                .into(),
            LogEntry::Err { error, .. } => error.into(),
        }
    }
}

#[derive(Template)]
#[template(path = "debug_game.html")]
struct GameDebugPage {
    id: Uuid,
    log: Vec<LogEntry>,
}
