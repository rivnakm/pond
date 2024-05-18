# pond

[![Build Action](https://github.com/mrivnak/pond/actions/workflows/build.yml/badge.svg)](https://github.com/mrivnak/pond/actions/workflows/build.yml)
[![Test Action](https://github.com/mrivnak/pond/actions/workflows/test.yml/badge.svg)](https://github.com/mrivnak/pond/actions/workflows/test.yml)
![GitHub release (latest SemVer)](https://img.shields.io/github/v/release/mrivnak/pond)

![Rust](https://img.shields.io/badge/rust-%23000000.svg?style=for-the-badge&logo=rust&logoColor=white)
![SQLite](https://img.shields.io/badge/sqlite-%2307405e.svg?style=for-the-badge&logo=sqlite&logoColor=white)

Simple, local, persistent cache. Backed by SQLite

## Example usage

```rust
use std::path::PathBuf;
use uuid::Uuid;

use pond_cache::Cache;

#[derive(Clone, serde::Serialize, serde::Deserialize)]
struct User {
    pub first_name: String,
    pub last_name: String,
}

fn main() {
    let cache = Cache::new(PathBuf::from("./db.sqlite")).unwrap();

    let user_id = Uuid::new_v4();
    let user = User {
        first_name: "John",
        last_name: "Doe",
    };

    cache.store(&user_id, user).unwrap();

    let result: Option<User> = cache.get(&key).unwrap();
}
```
