use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
pub(crate) struct Stats {
    pub(crate) plate_appearances: usize,
}
