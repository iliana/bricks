use crate::chronicler::Key;
use crate::table::Table;
use crate::DB;
use crate::{names, routes::ResponseResult, salmon};
use anyhow::Result;
use askama::Template;
use chrono::Utc;
use rocket::get;
use rocket::response::content::Html;
use uuid::Uuid;
use zerocopy::AsBytes;

#[get("/salmon/<id>")]
pub fn salmon_page(id: Uuid) -> ResponseResult<Option<Html<String>>> {
    Ok(match load_player(id)? {
        Some(player) => Some(Html(player.render().map_err(anyhow::Error::from)?)),
        None => None,
    })
}

fn load_player(id: Uuid) -> Result<Option<SalmonPage>> {
    let name = match names::player_name(id)? {
        Some(name) => name,
        None => return Ok(None),
    };

    let salmon_tree = DB.open_tree(salmon::SUMMARY_TREE)?;
    if let Some((_, summary)) = salmon_tree.get_lt(Key::new(id, Utc::now()).as_bytes())? {
        Ok(Some(SalmonPage {
            name,
            id,
            salmon: salmon::table(serde_json::from_slice(&summary)?),
        }))
    } else {
        Ok(None)
    }
}

#[derive(Template)]
#[template(path = "salmon.html")]
struct SalmonPage {
    name: String,
    id: Uuid,
    salmon: Table<{ salmon::COLS }>,
}
