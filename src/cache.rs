use crate::DB;
use anyhow::Result;
use chrono::{DateTime, Utc};
use rusqlite::{OptionalExtension, ToSql};
use std::ops::Range;

pub(crate) async fn load(
    kind: &'static str,
    key: &str,
    at: Option<DateTime<Utc>>,
) -> Result<Option<Vec<u8>>> {
    let mut sql = "SELECT value FROM caches WHERE kind = :kind AND key = :key".to_owned();
    let mut params = vec![(":kind", &kind as &dyn ToSql), (":key", &key)];
    if let Some(ref at) = at {
        sql.push_str(" AND start_time <= :at AND :at < end_time");
        params.push((":at", at));
    }
    let value: Option<Vec<u8>> = DB
        .lock()
        .await
        .query_row(&sql, params.as_slice(), |row| row.get(0))
        .optional()?;
    Ok(match value {
        Some(value) => Some(zstd::decode_all(&*value)?),
        None => None,
    })
}

#[fix_hidden_lifetime_bug::fix_hidden_lifetime_bug]
pub(crate) async fn store(
    kind: &'static str,
    key: &str,
    value: &[u8],
    valid: Option<Range<DateTime<Utc>>>,
) -> Result<()> {
    let compressed = zstd::encode_all(&*value, 0)?;
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
    DB.lock().await.execute(sql, params)?;
    Ok(())
}
