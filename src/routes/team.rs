use crate::{routes::ResponseResult, summary};
use anyhow::Result;
use askama::Template;
use rocket::get;
use rocket::response::content::Html;
use uuid::Uuid;

#[get("/team/<id>/<sim>/<season>")]
pub fn team(id: Uuid, sim: &str, season: u16) -> ResponseResult<Option<Html<String>>> {
    Ok(match load_team(id, sim, season)? {
        Some(team) => Some(Html(team.render().map_err(anyhow::Error::from)?)),
        None => None,
    })
}

fn load_team(id: Uuid, sim: &str, season: u16) -> Result<Option<TeamPage>> {
    let mut summary = summary::team_summary(id, sim, season)?.collect::<Result<Vec<_>>>()?;
    if summary.is_empty() {
        return Ok(None);
    }
    summary.sort_unstable();

    unimplemented!();
}

#[derive(Template)]
#[template(path = "team.html")]
struct TeamPage {
    name: String,
}
