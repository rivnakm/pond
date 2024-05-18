#![crate_name = "pond_cache"]

use std::hash::{DefaultHasher, Hash, Hasher};
use std::path::PathBuf;

use chrono::{DateTime, Duration, Utc};
use rusqlite::Connection;
use serde::de::DeserializeOwned;
use serde::Serialize;

pub use rusqlite::Error;

/// Pond cache struct
pub struct Cache<T> {
    path: PathBuf,
    ttl: Duration,
    data: std::marker::PhantomData<T>,
}

#[derive(Debug)]
struct CacheEntry<T>
where
    T: Serialize + DeserializeOwned + Clone,
{
    key: u32,
    value: T,
    expiration: DateTime<Utc>,
}

impl<T: Serialize + DeserializeOwned + Clone> Cache<T> {
    /// Create a new cache with a default time-to-live of 10 minutes
    ///
    /// # Arguments
    /// * `path` - Path to the SQLite database file
    ///
    /// # Returns
    /// A new cache instance
    ///
    /// # Errors
    /// Returns an error if the database connection cannot be established
    ///
    /// # Example
    /// ```rust
    /// use pond_cache::Cache;
    /// use std::path::PathBuf;
    ///
    /// let cache: Cache<String> = Cache::new(PathBuf::from("cache.db")).expect("Failed to create cache");
    /// ```
    pub fn new(path: PathBuf) -> Result<Self, Error> {
        Self::with_time_to_live(path, Duration::minutes(10))
    }

    /// Create a new cache with a custom time-to-live
    ///
    /// # Arguments
    /// * `path` - Path to the SQLite database file
    /// * `ttl` - Time-to-live for cache entries
    ///
    /// # Returns
    /// A new cache instance
    ///
    /// # Errors
    /// Returns an error if the database connection cannot be established
    ///
    /// # Example
    /// ```rust
    /// use pond_cache::Cache;
    /// use std::path::PathBuf;
    /// use chrono::Duration;
    ///
    /// let cache: Cache<String> = Cache::with_time_to_live(PathBuf::from("cache.db"), Duration::minutes(5)).expect("Failed to create cache");
    /// ```
    pub fn with_time_to_live(path: PathBuf, ttl: Duration) -> Result<Self, Error> {
        let db = Connection::open(path.as_path())?;

        db.execute(
            "CREATE TABLE IF NOT EXISTS items (
            id      TEXT PRIMARY KEY,
            expires TEXT NOT NULL,
            data    BLOB NOT NULL
        )",
            (),
        )?;

        db.close().expect("Failed to close database connection");

        Ok(Self {
            path,
            ttl,
            data: std::marker::PhantomData,
        })
    }

    /// Retrieve a value from the cache
    ///
    /// # Arguments
    /// * `key` - Key to retrieve the value for
    ///
    /// # Returns
    /// The value associated with the key, if it exists and has not expired
    /// If the value does not exist or has expired, returns `None`
    ///
    /// # Errors
    /// Returns an error if the database connection cannot be established
    ///
    /// # Example
    /// ```rust
    /// use pond_cache::Cache;
    /// use std::path::PathBuf;
    ///
    /// let cache: Cache<String> = Cache::new(PathBuf::from("cache.db")).expect("Failed to create cache");
    /// let key = "key";
    /// let value: Option<String> = cache.get(key).expect("Failed to get value");
    /// ```
    pub fn get<K: Hash>(&self, key: K) -> Result<Option<T>, Error> {
        let db = Connection::open(self.path.as_path())?;

        let mut stmt = db.prepare(
            "SELECT id, expires, data
                FROM items
                WHERE id = ?1",
        )?;

        let mut hasher = DefaultHasher::new();
        let hash = {
            key.hash(&mut hasher);
            hasher.finish() as u32
        };
        let mut rows = stmt.query([hash]).unwrap();

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
        let data: Vec<u8> = row.get(2).unwrap();

        drop(rows);
        drop(stmt);
        db.close().expect("Failed to close database connection");

        let data: T = bitcode::deserialize(&data).unwrap();

        if expires < Utc::now() {
            Ok(None)
        } else {
            Ok(Some(data))
        }
    }

    /// Store a value in the cache
    /// The value will be stored with the cache's time-to-live
    /// If the value already exists, it will be replaced
    ///
    /// # Arguments
    /// * `key` - Key to store the value under
    /// * `value` - Value to store
    ///
    /// # Returns
    /// Ok if the value was stored successfully
    /// Err if the value could not be stored
    ///
    /// # Errors
    /// Returns an error if the database connection cannot be established
    ///
    /// # Example
    /// ```rust
    /// use pond_cache::Cache;
    /// use std::path::PathBuf;
    ///
    /// let cache: Cache<String> = Cache::new(PathBuf::from("cache.db")).expect("Failed to create cache");
    /// let key = "key";
    /// let value = String::from("value");
    /// cache.store(key, value).expect("Failed to store value");
    /// ```
    pub fn store<K: Hash>(&self, key: K, value: T) -> Result<(), Error> {
        self.store_with_expiration(key, value, Utc::now() + self.ttl)
    }

    /// Store a value in the cache with a custom expiration time
    /// If the value already exists, it will be replaced
    ///
    /// # Arguments
    /// * `key` - Key to store the value under
    /// * `value` - Value to store
    /// * `expiration` - Expiration time for the value
    ///
    /// # Returns
    /// Ok if the value was stored successfully
    /// Err if the value could not be stored
    ///
    /// # Errors
    /// Returns an error if the database connection cannot be established
    ///
    /// # Example
    /// ```rust
    /// use pond_cache::Cache;
    /// use std::path::PathBuf;
    /// use chrono::{Duration, Utc};
    ///
    /// let cache: Cache<String> = Cache::new(PathBuf::from("cache.db")).expect("Failed to create cache");
    /// let key = "key";
    /// let value = String::from("value");
    /// let expiration = Utc::now() + Duration::minutes(5);
    ///
    /// cache.store_with_expiration(key, value, expiration).expect("Failed to store value");
    /// ```
    pub fn store_with_expiration<K: Hash>(
        &self,
        key: K,
        value: T,
        expiration: DateTime<Utc>,
    ) -> Result<(), Error> {
        let mut hasher = DefaultHasher::new();
        let hash = {
            key.hash(&mut hasher);
            hasher.finish() as u32
        };

        let value = CacheEntry {
            key: hash,
            value,
            expiration,
        };

        let db = Connection::open(self.path.as_path())?;

        db.execute(
            "INSERT OR REPLACE INTO items (id, expires, data) VALUES (?1, ?2, ?3);",
            (
                &value.key.to_string(),
                &value.expiration.to_rfc3339(),
                &bitcode::serialize(&value.value).unwrap(),
            ),
        )?;

        db.close().expect("Failed to close database connection");

        Ok(())
    }

    /// Clean up the cache by removing expired entries
    /// This method should be called periodically to prevent the cache from growing indefinitely
    ///
    /// # Returns
    /// Ok if the cache was cleaned successfully
    /// Err if the cache could not be cleaned
    ///
    /// # Errors
    /// Returns an error if the database connection cannot be established
    ///
    /// # Example
    /// ```rust
    /// use pond_cache::Cache;
    /// use std::path::PathBuf;
    ///
    /// let cache: Cache<String> = Cache::new(PathBuf::from("cache.db")).expect("Failed to create cache");
    /// cache.clean().expect("Failed to clean cache");
    /// ```
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
    use serde::Deserialize;
    use serde::Serialize;
    use uuid::Uuid;

    use super::*;

    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    struct User {
        id: Uuid,
        name: String,
    }

    fn store_manual(
        path: PathBuf,
        key: String,
        value: Vec<u8>,
        expires: DateTime<Utc>,
    ) -> Result<(), Error> {
        let mut hasher = DefaultHasher::new();
        let hash = {
            key.hash(&mut hasher);
            hasher.finish() as u32
        };

        let db = Connection::open(path.as_path()).unwrap();

        db.execute(
            "INSERT OR REPLACE INTO items (id, expires, data) VALUES (?1, ?2, ?3);",
            (hash, &expires.to_rfc3339(), &value),
        )
        .unwrap();

        db.close().unwrap();
        Ok(())
    }

    fn get_manual<T: Serialize + DeserializeOwned + Clone>(
        path: PathBuf,
        key: String,
    ) -> Result<Option<CacheEntry<T>>, Error> {
        let db = Connection::open(path.as_path())?;

        let mut stmt = db.prepare(
            "SELECT id, expires, data
                FROM items
                WHERE id = ?1",
        )?;

        let mut hasher = DefaultHasher::new();
        let hash = {
            key.hash(&mut hasher);
            hasher.finish() as u32
        };

        let mut rows = stmt.query([hash]).unwrap();

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
        let data: Vec<u8> = row.get(2).unwrap();

        drop(rows);
        drop(stmt);
        db.close().expect("Failed to close database connection");

        let data: T = bitcode::deserialize(&data).unwrap();

        Ok(Some(CacheEntry {
            key: hash,
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
        let cache: Cache<String> = Cache::new(filename.clone()).unwrap();
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
        let _: Cache<String> = Cache::new(filename.clone()).unwrap();
        let _: Cache<String> = Cache::new(filename).unwrap();
    }

    #[test]
    fn test_time_to_live() {
        let filename = std::env::temp_dir().join(format!(
            "pond-test-{}-{}.sqlite",
            Uuid::new_v4(),
            rand::random::<u8>()
        ));
        let cache: Cache<String> =
            Cache::with_time_to_live(filename.clone(), Duration::minutes(5)).unwrap();
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

        cache.store(key, value.clone()).unwrap();
        let result: Option<_> = cache.get(key).unwrap();

        assert_eq!(result, Some(value));
    }

    #[test]
    fn test_store_get_struct() {
        let filename = std::env::temp_dir().join(format!(
            "pond-test-{}-{}.sqlite",
            Uuid::new_v4(),
            rand::random::<u8>()
        ));

        let cache = Cache::new(filename).unwrap();

        let key = Uuid::new_v4();
        let value = User {
            id: Uuid::new_v4(),
            name: String::from("Alice"),
        };

        cache.store(key, value.clone()).unwrap();
        let result: Option<_> = cache.get(key).unwrap();

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

        cache.store(key, value).unwrap();

        let value = String::from("Hello, world! 2");
        cache.store(key, value.clone()).unwrap();
        let result: Option<_> = cache.get(key).unwrap();

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

        store_manual(
            filename,
            key.to_string(),
            bitcode::serialize(&value).unwrap(),
            Utc::now() - Duration::minutes(5),
        )
        .unwrap();
        let result: Option<String> = cache.get(key).unwrap();

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

        let result: Option<String> = cache.get(key).unwrap();

        assert_eq!(result, None);
    }

    #[test]
    fn test_invalid_path() {
        let cache: Result<Cache<String>, Error> =
            Cache::new(PathBuf::from("invalid/path/db.sqlite"));

        assert!(cache.is_err());
    }

    #[test]
    fn test_clean() {
        let filename = std::env::temp_dir().join(format!(
            "pond-test-{}-{}.sqlite",
            Uuid::new_v4(),
            rand::random::<u8>()
        ));

        let cache: Cache<String> =
            Cache::with_time_to_live(filename.clone(), Duration::minutes(5)).unwrap();

        let key = Uuid::new_v4().to_string();
        let value = String::from("Hello, world!");

        store_manual(
            filename.clone(),
            key.clone(),
            bitcode::serialize(&value).unwrap(),
            Utc::now() - Duration::minutes(5),
        )
        .unwrap();

        let result: Option<CacheEntry<String>> = get_manual(filename.clone(), key.clone()).unwrap();
        if let Some(result) = result {
            assert_eq!(result.value, value);
        } else {
            panic!("Expected result to be Some");
        }

        cache.clean().unwrap();
        let result: Option<CacheEntry<String>> = get_manual(filename, key).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_clean_leaves_unexpired() {
        let filename = std::env::temp_dir().join(format!(
            "pond-test-{}-{}.sqlite",
            Uuid::new_v4(),
            rand::random::<u8>()
        ));

        let cache: Cache<String> =
            Cache::with_time_to_live(filename.clone(), Duration::minutes(5)).unwrap();

        let key = Uuid::new_v4().to_string();
        let value = String::from("Hello, world!");

        store_manual(
            filename.clone(),
            key.clone(),
            bitcode::serialize(&value).unwrap(),
            Utc::now() + Duration::minutes(15),
        )
        .unwrap();

        let result: Option<CacheEntry<String>> = get_manual(filename.clone(), key.clone()).unwrap();
        if let Some(result) = result {
            assert_eq!(result.value, value);
        } else {
            panic!("Expected result to be Some");
        }

        cache.clean().unwrap();

        let result: Option<CacheEntry<String>> = get_manual(filename, key).unwrap();
        if let Some(result) = result {
            assert_eq!(result.value, value);
        } else {
            panic!("Expected result to be Some");
        }
    }
}
