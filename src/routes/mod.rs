pub mod debug;
pub mod game;
pub mod player;
pub mod season;
pub mod team;

use crate::seasons::Season;
use anyhow::Result;
use rocket::http::{uri::Origin, ContentType};
use rocket::response::{status::BadRequest, Debug, Redirect};
use rocket::{get, Either};

type ResponseResult<T> = std::result::Result<T, Debug<anyhow::Error>>;

#[get("/")]
pub fn index() -> ResponseResult<Option<Redirect>> {
    fn last_season() -> Result<Option<Season>> {
        let mut seasons = Season::iter_recorded()?.collect::<Result<Vec<_>>>()?;
        seasons.sort_unstable();
        Ok(seasons.into_iter().rev().next())
    }

    Ok(last_season()
        .map_err(anyhow::Error::from)?
        .map(|season| Redirect::to(season.uri(&true))))
}

macro_rules! asset {
    ($path:expr) => {
        include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/", $path))
    };
}

#[get("/styles.css")]
pub fn css() -> (ContentType, &'static str) {
    (
        ContentType::CSS,
        include_str!(concat!(env!("OUT_DIR"), "/styles.css")),
    )
}

#[get("/brick.svg")]
pub fn brick() -> (ContentType, &'static str) {
    (ContentType::SVG, asset!("brick.svg"))
}

#[get("/tablesort.min.js")]
pub fn tablesort() -> (ContentType, &'static str) {
    (
        ContentType::JavaScript,
        asset!("node_modules/tablesort/dist/tablesort.min.js"),
    )
}

#[get("/tablesort.number.min.js")]
pub fn tablesort_number() -> (ContentType, &'static str) {
    (
        ContentType::JavaScript,
        asset!("node_modules/tablesort/dist/sorts/tablesort.number.min.js"),
    )
}

#[get("/jump?<path>")]
pub fn jump(path: String) -> Either<Redirect, BadRequest<()>> {
    match Origin::try_from(path) {
        Ok(path) => Either::Left(Redirect::to(path)),
        Err(_) => Either::Right(BadRequest(None)),
    }
}
