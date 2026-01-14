// PLAN: 1. Add leak tracking counters -> 2. Expose leak check API -> 3. Add std.db query
// Library choice: std::sync::atomic is the safest zero-dependency counter.

use std::sync::atomic::{AtomicUsize, Ordering};

static LIVE_OBJECTS: AtomicUsize = AtomicUsize::new(0);

pub fn track_alloc() {
    LIVE_OBJECTS.fetch_add(1, Ordering::Relaxed);
}

pub fn track_free() {
    LIVE_OBJECTS.fetch_sub(1, Ordering::Relaxed);
}

pub fn check_leaks() {
    let count = LIVE_OBJECTS.load(Ordering::Relaxed);
    if count > 0 {
        eprintln!("Memory Leak Detected: {} objects leaked.", count);
    }
}

pub fn leak() {
    track_alloc();
    eprintln!("Leaking an object...");
}

pub mod db {
    use anyhow::Result;
    use serde_json::{json, Map, Value};
    use sqlx::any::{AnyPoolOptions, AnyRow};
    use sqlx::{Column, Row, TypeInfo};

    pub fn query(url: &str, sql: &str) -> String {
        query_inner(url, sql).unwrap_or_else(|e| format!("{{\"error\":\"{}\"}}", e))
    }

    fn query_inner(url: &str, sql: &str) -> Result<String> {
        tokio::runtime::Runtime::new().unwrap().block_on(async {
            sqlx::any::install_default_drivers();
            let pool = AnyPoolOptions::new().max_connections(5).connect(url).await?;
            let rows = sqlx::query(sql).fetch_all(&pool).await?;
            let rows_json = rows_to_json(&rows);
            Ok(serde_json::to_string(&rows_json)?)
        })
    }

    fn rows_to_json(rows: &[AnyRow]) -> Value {
        let mut out = Vec::new();
        for row in rows {
            let mut map = Map::new();
            let len = row.len();
            for idx in 0..len {
                let col = row.column(idx);
                let name = col.name();
                let value = cell_to_json(row, idx);
                map.insert(name.to_string(), value);
            }
            out.push(Value::Object(map));
        }
        Value::Array(out)
    }

    fn cell_to_json(row: &AnyRow, idx: usize) -> Value {
        use sqlx::Row;
        if let Ok(v) = row.try_get::<i64, _>(idx) {
            return json!(v);
        }
        if let Ok(v) = row.try_get::<f64, _>(idx) {
            return json!(v);
        }
        if let Ok(v) = row.try_get::<bool, _>(idx) {
            return json!(v);
        }
        if let Ok(v) = row.try_get::<String, _>(idx) {
            return json!(v);
        }
        json!(null)
    }
}

pub mod spider {
    use wreq::Client;
    use wreq_util::Emulation;

    pub fn get(url: &str, profile: Option<&str>) -> String {
        tokio::runtime::Runtime::new().unwrap().block_on(async {
            let emu = match profile.unwrap_or("chrome") {
                "safari" => Emulation::Safari26,
                _ => Emulation::Chrome124,
            };

            let client = Client::builder()
                .emulation(emu)
                .build()
                .unwrap();

            client.get(url).send().await.unwrap().text().await.unwrap()
        })
    }
}
