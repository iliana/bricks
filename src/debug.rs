use json_patch::Patch;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
#[serde(untagged)]
pub enum LogEntry {
    Ok {
        description: String,
        patch: Patch,
    },
    Err {
        description: Option<String>,
        error: String,
    },
}
