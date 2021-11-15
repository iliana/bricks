pub mod debug;
pub mod game;
pub mod player;

use rocket::get;
use rocket::http::ContentType;
use rocket::response::Debug;

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
