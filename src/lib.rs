use std::path::PathBuf;

use chrono::{DateTime, Duration, Utc};
use rusqlite::Connection;
use uuid::Uuid;

pub use rusqlite::types::{FromSql, ToSql};
pub use rusqlite::Error;

pub struct Cache {
    path: PathBuf,
    ttl: Duration,
}

pub struct CacheEntry<T> {
    key: Uuid,
    value: T,
    expiration: DateTime<Utc>,
}

impl Cache {
    pub fn new(path: PathBuf) -> Result<Self, Error> {
        Self::with_time_to_live(path, Duration::minutes(10))
    }

    pub fn with_time_to_live(path: PathBuf, ttl: Duration) -> Result<Self, Error> {
        let db = Connection::open(path.as_path())?;

        db.execute(
            "CREATE TABLE IF NOT EXISTS items (
            id      TEXT PRIMARY KEY,
            expires TEXT NOT NULL,
            data    TEXT NOT NULL
        )",
            (), // empty list of parameters.
        )?;

        db.close().expect("Failed to close database connection");

        Ok(Self { path, ttl })
    }

    pub fn get<T: FromSql>(&self, key: &Uuid) -> Result<Option<T>, Error> {
        let db = Connection::open(self.path.as_path())?;

        let mut stmt = db.prepare(
            "SELECT id, expires, data
                FROM items
                WHERE id = ?1",
        )?;

        let item_id = key.to_string();
        let mut rows = stmt.query([&item_id]).unwrap();

        let Some(row) = rows.next().unwrap() else {
            return Ok(None);
        };

        let expires: DateTime<Utc> = row
            .get::<usize, String>(1)
            .map(|expires_string| {
                DateTime::parse_from_rfc3339(&expires_string)
                    .unwrap()
                    .with_timezone(&Utc)
            })
            .unwrap();
        let data: T = row.get(2).unwrap();

        drop(rows);
        drop(stmt);
        db.close().expect("Failed to close database connection");

        if expires < Utc::now() {
            Ok(None)
        } else {
            Ok(Some(data))
        }
    }

    pub fn store<T: ToSql>(&self, key: &Uuid, value: T) -> Result<(), Error> {
        let value = CacheEntry {
            key: *key,
            value,
            expiration: Utc::now() + self.ttl,
        };

        let db = Connection::open(self.path.as_path())?;

        db.execute(
            "INSERT OR REPLACE INTO items (id, expires, data) VALUES (?1, ?2, ?3);",
            (
                &value.key.to_string(),
                &value.expiration.to_rfc3339(),
                &value.value,
            ),
        )?;

        db.close().expect("Failed to close database connection");

        Ok(())
    }

    pub fn clean(&self) -> Result<(), Error> {
        let db = Connection::open(self.path.as_path())?;

        db.execute(
            "DELETE FROM items WHERE expires < ?1;",
            (&Utc::now().to_rfc3339(),),
        )?;

        db.close().expect("Failed to close database connection");

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store_manual(
        path: PathBuf,
        key: &Uuid,
        value: String,
        expires: DateTime<Utc>,
    ) -> Result<(), Error> {
        let db = Connection::open(path.as_path()).unwrap();

        db.execute(
            "INSERT OR REPLACE INTO items (id, expires, data) VALUES (?1, ?2, ?3);",
            (&key.to_string(), &expires.to_rfc3339(), &value),
        )
        .unwrap();

        db.close().unwrap();
        Ok(())
    }

    fn get_manual<T: FromSql>(path: PathBuf, key: &Uuid) -> Result<Option<CacheEntry<T>>, Error> {
        let db = Connection::open(path.as_path())?;

        let mut stmt = db.prepare(
            "SELECT id, expires, data
                FROM items
                WHERE id = ?1",
        )?;

        let item_id = key.to_string();
        let mut rows = stmt.query([&item_id]).unwrap();

        let Some(row) = rows.next().unwrap() else {
            return Ok(None);
        };

        let expires: DateTime<Utc> = row
            .get::<usize, String>(1)
            .map(|expires_string| {
                DateTime::parse_from_rfc3339(&expires_string)
                    .unwrap()
                    .with_timezone(&Utc)
            })
            .unwrap();
        let data: T = row.get(2).unwrap();

        Ok(Some(CacheEntry {
            key: *key,
            value: data,
            expiration: expires,
        }))
    }

    #[test]
    fn test_new() {
        let filename = std::env::temp_dir().join(format!(
            "pond-test-{}-{}.sqlite",
            Uuid::new_v4(),
            rand::random::<u8>()
        ));
        let cache = Cache::new(filename.clone()).unwrap();
        assert_eq!(cache.path, filename);
        assert_eq!(cache.ttl, Duration::minutes(10));
    }

    #[test]
    fn test_load_existing() {
        let filename = std::env::temp_dir().join(format!(
            "pond-test-{}-{}.sqlite",
            Uuid::new_v4(),
            rand::random::<u8>()
        ));
        let _ = Cache::new(filename.clone()).unwrap();
        let _ = Cache::new(filename).unwrap();
    }

    #[test]
    fn test_time_to_live() {
        let filename = std::env::temp_dir().join(format!(
            "pond-test-{}-{}.sqlite",
            Uuid::new_v4(),
            rand::random::<u8>()
        ));
        let cache = Cache::with_time_to_live(filename.clone(), Duration::minutes(5)).unwrap();
        assert_eq!(cache.path, filename);
        assert_eq!(cache.ttl, Duration::minutes(5));
    }

    #[test]
    fn test_store_get() {
        let filename = std::env::temp_dir().join(format!(
            "pond-test-{}-{}.sqlite",
            Uuid::new_v4(),
            rand::random::<u8>()
        ));

        let cache = Cache::new(filename).unwrap();

        let key = Uuid::new_v4();
        let value = String::from("Hello, world!");

        cache.store(&key, value.clone()).unwrap();
        let result: Option<_> = cache.get(&key).unwrap();

        assert_eq!(result, Some(value));
    }

    #[test]
    fn test_store_existing() {
        let filename = std::env::temp_dir().join(format!(
            "pond-test-{}-{}.sqlite",
            Uuid::new_v4(),
            rand::random::<u8>()
        ));

        let cache = Cache::new(filename).unwrap();

        let key = Uuid::new_v4();
        let value = String::from("Hello, world!");

        cache.store(&key, value).unwrap();

        let value = String::from("Hello, world! 2");
        cache.store(&key, value.clone()).unwrap();
        let result: Option<_> = cache.get(&key).unwrap();

        assert_eq!(result, Some(value));
    }

    #[test]
    fn test_get_expired() {
        let filename = std::env::temp_dir().join(format!(
            "pond-test-{}-{}.sqlite",
            Uuid::new_v4(),
            rand::random::<u8>()
        ));

        let cache = Cache::new(filename.clone()).unwrap();

        let key = Uuid::new_v4();
        let value = String::from("Hello, world!");

        store_manual(filename, &key, value, Utc::now() - Duration::minutes(5)).unwrap();
        let result: Option<String> = cache.get(&key).unwrap();

        assert_eq!(result, None);
    }

    #[test]
    fn test_get_nonexistent() {
        let filename = std::env::temp_dir().join(format!(
            "pond-test-{}-{}.sqlite",
            Uuid::new_v4(),
            rand::random::<u8>()
        ));

        let cache = Cache::new(filename).unwrap();

        let key = Uuid::new_v4();

        let result: Option<String> = cache.get(&key).unwrap();

        assert_eq!(result, None);
    }

    #[test]
    fn test_invalid_path() {
        let cache = Cache::new(PathBuf::from("invalid/path/db.sqlite"));

        assert!(cache.is_err());
    }

    #[test]
    fn test_clean() {
        let filename = std::env::temp_dir().join(format!(
            "pond-test-{}-{}.sqlite",
            Uuid::new_v4(),
            rand::random::<u8>()
        ));

        let cache = Cache::with_time_to_live(filename.clone(), Duration::minutes(5)).unwrap();

        let key = Uuid::new_v4();
        let value = String::from("Hello, world!");

        store_manual(
            filename.clone(),
            &key,
            value.clone(),
            Utc::now() - Duration::minutes(5),
        )
        .unwrap();

        let result: Option<CacheEntry<String>> = get_manual(filename.clone(), &key).unwrap();
        if let Some(result) = result {
            assert_eq!(result.value, value);
        } else {
            panic!("Expected result to be Some");
        }

        cache.clean().unwrap();
        let result: Option<CacheEntry<String>> = get_manual(filename, &key).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_clean_leaves_unexpired() {
        let filename = std::env::temp_dir().join(format!(
            "pond-test-{}-{}.sqlite",
            Uuid::new_v4(),
            rand::random::<u8>()
        ));

        let cache = Cache::with_time_to_live(filename.clone(), Duration::minutes(5)).unwrap();

        let key = Uuid::new_v4();
        let value = String::from("Hello, world!");

        store_manual(
            filename.clone(),
            &key,
            value.clone(),
            Utc::now() + Duration::minutes(15),
        )
        .unwrap();

        let result: Option<CacheEntry<String>> = get_manual(filename.clone(), &key).unwrap();
        if let Some(result) = result {
            assert_eq!(result.value, value);
        } else {
            panic!("Expected result to be Some");
        }

        cache.clean().unwrap();

        let result: Option<CacheEntry<String>> = get_manual(filename, &key).unwrap();
        if let Some(result) = result {
            assert_eq!(result.value, value);
        } else {
            panic!("Expected result to be Some");
        }
    }
}
