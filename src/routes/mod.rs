pub mod debug;
pub mod game;

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
