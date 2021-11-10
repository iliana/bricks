use crate::{db::Db, ResponseResult};
use anyhow::Result;
use askama::Template;
use rocket::get;
use rocket::response::content::Html;
use std::collections::BTreeMap;
use uuid::Uuid;

#[get("/errors")]
pub(crate) async fn errors(db: Db) -> ResponseResult<Html<String>> {
    Ok(Html(
        ErrorDashboard {
            errors: load_errors(db).await?,
        }
        .render()
        .map_err(anyhow::Error::from)?,
    ))
}

async fn load_errors(db: Db) -> Result<BTreeMap<String, Vec<Uuid>>> {
    db.run(|conn| {
        let mut statement =
            conn.prepare_cached("SELECT error, game_id FROM game_debug WHERE error IS NOT NULL")?;
        let rows = statement.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?;

        let mut map: BTreeMap<String, Vec<Uuid>> = BTreeMap::new();
        for row in rows {
            let (error, id) = row?;
            map.entry(error).or_default().push(id);
        }
        Ok(map)
    })
    .await
}

#[derive(Template)]
#[template(path = "error_dashboard.html")]
struct ErrorDashboard {
    errors: BTreeMap<String, Vec<Uuid>>,
}
