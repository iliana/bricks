pub mod debug;
pub mod game;
pub mod player;
pub mod team;

use rocket::http::{uri::Origin, ContentType};
use rocket::response::{status::BadRequest, Debug, Redirect};
use rocket::{get, Either};

type ResponseResult<T> = Result<T, Debug<anyhow::Error>>;

#[get("/styles.css")]
pub fn css() -> (ContentType, &'static str) {
    (
        ContentType::CSS,
        include_str!(concat!(env!("OUT_DIR"), "/styles.css")),
    )
}

#[get("/brick.svg")]
pub fn brick() -> (ContentType, &'static str) {
    (
        ContentType::SVG,
        include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/brick.svg")),
    )
}

#[get("/jump?<path>")]
pub fn jump(path: String) -> Either<Redirect, BadRequest<()>> {
    match Origin::try_from(path) {
        Ok(path) => Either::Left(Redirect::to(path)),
        Err(_) => Either::Right(BadRequest(None)),
    }
}
