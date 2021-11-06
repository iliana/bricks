use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Deserialize, Serialize)]
pub(crate) struct Stats {
    // Batting stats
    pub(crate) stolen_bases: u16,
    pub(crate) walks: u16,
    pub(crate) struckouts: u16,

    // Pitching stats
    pub(crate) outs_recorded: u16,
    pub(crate) strikeouts: u16,
    pub(crate) walks_issued: u16,
}
