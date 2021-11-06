#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Team {
}

pub(crate) async fn load_team(team_id: &str, at: DateTime<Utc>) -> Result<Team> {
}
