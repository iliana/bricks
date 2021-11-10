use crate::{API_BASE, CLIENT};
use anyhow::Result;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub(crate) struct SimData {
    pub(crate) id: String,
    pub(crate) season: u16,
    pub(crate) day: u16,
}

pub(crate) async fn load_today() -> Result<SimData> {
    Ok(CLIENT
        .get(format!("{}/database/simulationData", API_BASE))
        .send()
        .await?
        .json()
        .await?)
}
