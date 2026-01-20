// PLAN: 1. Add leak tracking counters -> 2. Expose leak check API -> 3. Add std.db query -> 4. Add std.fs/html/spider proxy -> 5. Add Python bridge
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
    use sqlx::{Column, Row};

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

pub mod fs {
    pub fn read(path: &str) -> String {
        std::fs::read_to_string(path).unwrap_or_default()
    }

    pub fn write(path: &str, content: &str) {
        let _ = std::fs::write(path, content);
    }
}

pub mod html {
    use scraper::{Html, Selector};

    pub fn select_text(html: &str, selector: &str) -> Vec<String> {
        let mut out = Vec::new();
        let doc = Html::parse_document(html);
        let sel = match Selector::parse(selector) {
            Ok(s) => s,
            Err(_) => return out,
        };
        for el in doc.select(&sel) {
            let text = el.text().collect::<String>().trim().to_string();
            if !text.is_empty() {
                out.push(text);
            }
        }
        out
    }
}

pub mod json {
    use serde_json::Value;

    pub fn parse(s: &str) -> Value {
        serde_json::from_str(s).unwrap_or(Value::Null)
    }

    pub fn get(val: &Value, key: &str) -> Value {
        val.get(key).cloned().unwrap_or(Value::Null)
    }

    pub fn at(val: &Value, idx: usize) -> Value {
        match val {
            Value::Array(items) => items.get(idx).cloned().unwrap_or(Value::Null),
            _ => Value::Null,
        }
    }

    pub fn to_string(val: &Value) -> String {
        serde_json::to_string(val).unwrap_or_else(|_| "null".to_string())
    }
}

pub mod python {
    use pyo3::prelude::*;
    use std::ffi::CString;

    pub fn eval(code: &str) -> String {
        let code_c = CString::new(code).unwrap();
        Python::with_gil(|py| {
            let result = py.eval(&code_c, None, None);
            match result {
                Ok(value) => value.to_string(),
                Err(err) => err.to_string(),
            }
        })
    }
}

pub mod spider {
    use wreq::Client;
    use wreq_util::Emulation;

    pub fn get(url: &str, profile: Option<&str>) -> String {
        get_with_proxy(url, profile, None)
    }

    pub fn get_with_proxy(url: &str, profile: Option<&str>, proxy: Option<&str>) -> String {
        tokio::runtime::Runtime::new().unwrap().block_on(async {
            let emu = match profile.unwrap_or("chrome") {
                "safari" => Emulation::Safari26,
                _ => Emulation::Chrome124,
            };

            let mut builder = Client::builder().emulation(emu);
            if let Some(proxy_url) = proxy {
                builder = builder.proxy(wreq::Proxy::all(proxy_url).unwrap());
            }

            let client = builder.build().unwrap();
            client.get(url).send().await.unwrap().text().await.unwrap()
        })
    }
}
