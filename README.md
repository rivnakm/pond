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

fn main() {
    let cache = Cache::new(PathBuf::from("./db.sqlite")).unwrap();

    let key = Uuid::new_v4();
    let value = String::from("Hello, world!");

    cache.store(&key, value).unwrap();

    let result: Option<String> = cache.get(&key).unwrap();
}
```
