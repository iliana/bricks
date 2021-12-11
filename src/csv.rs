use csv::Writer;
use rocket::http::{ContentType, Status};
use rocket::response::{self, content::Custom, Responder};
use rocket::Request;
use serde::Serialize;
use std::io::Cursor;

pub struct Csv<T>(pub T);

impl<'r, T: Serialize> Responder<'r, 'static> for Csv<Vec<T>> {
    fn respond_to(self, req: &'r Request<'_>) -> response::Result<'static> {
        let string = write_csv(self.0).map_err(|e| {
            log::error!("CSV failed to serialize: {:?}", e);
            Status::InternalServerError
        })?;
        Custom(ContentType::CSV, string).respond_to(req)
    }
}

fn write_csv<T: Serialize>(rows: Vec<T>) -> anyhow::Result<String> {
    let mut writer = Writer::from_writer(Cursor::new(Vec::new()));
    for row in rows {
        writer.serialize(row)?;
    }
    let buf = writer.into_inner()?.into_inner();
    Ok(String::from_utf8(buf)?)
}
