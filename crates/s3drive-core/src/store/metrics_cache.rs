//! メトリクス・コスト・単価のキャッシュ（06 §4.2 の `metrics_cache`）。

use chrono::{DateTime, Duration, Utc};
use rusqlite::{OptionalExtension, params};
use serde::Serialize;
use serde::de::DeserializeOwned;

use super::Db;
use crate::error::CoreResult;
use crate::util::time;

pub struct Cached<T> {
    pub value: T,
    pub fetched_at: DateTime<Utc>,
    /// 期限切れ（`ttl` が `None` の項目は期限切れにならない）。
    pub expired: bool,
}

pub async fn get<T: DeserializeOwned + Send + 'static>(
    db: &Db,
    connection_id: &str,
    kind: &str,
) -> CoreResult<Option<Cached<T>>> {
    let (cid, kind) = (connection_id.to_string(), kind.to_string());
    let row = db
        .run(move |c| {
            Ok(c.query_row(
                "SELECT payload, fetched_at, expires_at FROM metrics_cache WHERE connection_id = ?1 AND kind = ?2",
                params![cid, kind],
                |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, Option<String>>(2)?)),
            )
            .optional()?)
        })
        .await?;
    let Some((payload, fetched_at, expires_at)) = row else {
        return Ok(None);
    };
    let Ok(value) = serde_json::from_str::<T>(&payload) else {
        // 形式が変わった古いキャッシュは無視する
        return Ok(None);
    };
    let now = time::now();
    let expired = expires_at
        .as_deref()
        .and_then(time::parse_rfc3339)
        .is_some_and(|e| e <= now);
    Ok(Some(Cached {
        value,
        fetched_at: time::parse_rfc3339(&fetched_at).unwrap_or(now),
        expired,
    }))
}

pub async fn put<T: Serialize>(
    db: &Db,
    connection_id: &str,
    kind: &str,
    value: &T,
    ttl: Option<Duration>,
) -> CoreResult<()> {
    let payload = serde_json::to_string(value)?;
    let now = time::now();
    let fetched_at = time::to_rfc3339(now);
    let expires_at = ttl.map(|t| time::to_rfc3339(now + t));
    let (cid, kind) = (connection_id.to_string(), kind.to_string());
    db.run(move |c| {
        c.execute(
            "INSERT INTO metrics_cache (connection_id, kind, payload, fetched_at, expires_at) VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT (connection_id, kind) DO UPDATE SET payload = excluded.payload, fetched_at = excluded.fetched_at, expires_at = excluded.expires_at",
            params![cid, kind, payload, fetched_at, expires_at],
        )?;
        Ok(())
    })
    .await
}

pub async fn clear_all(db: &Db) -> CoreResult<()> {
    db.run(|c| {
        c.execute("DELETE FROM metrics_cache", [])?;
        Ok(())
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn stores_and_expires_values() {
        let (db, _dir) = Db::open_temp().unwrap();
        put(
            &db,
            "c1",
            "storage",
            &vec![1, 2, 3],
            Some(Duration::hours(1)),
        )
        .await
        .unwrap();
        let hit = get::<Vec<i32>>(&db, "c1", "storage")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(hit.value, vec![1, 2, 3]);
        assert!(!hit.expired);

        put(&db, "c1", "storage", &vec![4], Some(Duration::seconds(-1)))
            .await
            .unwrap();
        let hit = get::<Vec<i32>>(&db, "c1", "storage")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(hit.value, vec![4]);
        assert!(hit.expired);

        put(&db, "c1", "cost", &"x", None).await.unwrap();
        assert!(
            !get::<String>(&db, "c1", "cost")
                .await
                .unwrap()
                .unwrap()
                .expired
        );

        clear_all(&db).await.unwrap();
        assert!(
            get::<Vec<i32>>(&db, "c1", "storage")
                .await
                .unwrap()
                .is_none()
        );
    }
}
