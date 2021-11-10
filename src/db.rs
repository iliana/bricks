use anyhow::Result;
use chrono::{DateTime, Utc};
use rocket::{Build, Rocket};
use rocket_sync_db_pools::database;
use rusqlite::{OptionalExtension, ToSql};
use std::fmt::{self, Debug};
use std::ops::Range;

refinery::embed_migrations!("./migrations");

#[database("db")]
pub(crate) struct Db(rusqlite::Connection);

impl Debug for Db {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Db").finish()
    }
}

impl Db {
    pub(crate) async fn cache_load(
        &self,
        kind: &'static str,
        key: String,
        at: Option<DateTime<Utc>>,
    ) -> Result<Option<Vec<u8>>> {
        let value: Option<Vec<u8>> = self
            .run(move |conn| {
                let mut sql =
                    "SELECT value FROM caches WHERE kind = :kind AND key = :key".to_owned();
                let mut params = vec![(":kind", &kind as &dyn ToSql), (":key", &key)];
                if let Some(ref at) = at {
                    sql.push_str(" AND start_time <= :at AND :at < end_time");
                    params.push((":at", at));
                }
                conn.query_row(&sql, params.as_slice(), |row| row.get(0))
                    .optional()
            })
            .await?;
        Ok(match value {
            Some(value) => Some(zstd::decode_all(&*value)?),
            None => None,
        })
    }

    #[fix_hidden_lifetime_bug::fix_hidden_lifetime_bug]
    pub(crate) async fn cache_store(
        &self,
        kind: &'static str,
        key: String,
        value: &[u8],
        valid: Option<Range<DateTime<Utc>>>,
    ) -> Result<()> {
        let compressed = zstd::encode_all(&*value, 19)?;
        self.run(move |conn| {
            let sql = "INSERT INTO caches (kind, key, value, start_time, end_time) \
               VALUES (:kind, :key, :value, :start_time, :end_time) \
               ON CONFLICT (kind, key, start_time) DO UPDATE \
               SET value = :value, end_time = :end_time";
            let params = &[
                (":kind", &kind as &dyn ToSql),
                (":key", &key),
                (":value", &compressed),
                (":start_time", &valid.as_ref().map(|r| r.start)),
                (":end_time", &valid.as_ref().map(|r| r.end)),
            ];
            conn.execute(sql, params)
        })
        .await?;
        Ok(())
    }
}

pub(crate) async fn run_migrations(rocket: Rocket<Build>) -> rocket::fairing::Result {
    if let Some(db) = Db::get_one(&rocket).await {
        if db.run(|conn| migrations::runner().run(conn)).await.is_ok() {
            return Ok(rocket);
        }
    }

    Err(rocket)
}
