use std::collections::HashMap;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::JsFuture;
use web_sys::Response;

/// Cyrillic -> Traditional Mongolian dictionary.
///
/// Keys are the Cyrillic column (lower-cased, trimmed). Each key maps to one
/// or more Mongolian traditional variants (duplicates in the source TSV are
/// preserved as separate entries in the `Vec`).
pub struct Dictionary {
    map: HashMap<String, Vec<String>>,
}

impl Dictionary {
    /// Fetches the prepared TSV file and parses it. Safe to call once at
    /// startup or lazily on first use.
    pub async fn load(path: &str) -> Result<Self, JsValue> {
        let window = web_sys::window().ok_or("No window")?;
        let resp_value = JsFuture::from(window.fetch_with_str(path)).await?;
        let resp: Response = resp_value.dyn_into()?;
        if !resp.ok() {
            return Err(JsValue::from_str(&format!(
                "Failed to fetch dictionary: HTTP {}",
                resp.status()
            )));
        }
        let text_promise = resp.text()?;
        let text_value = JsFuture::from(text_promise).await?;
        let text = text_value
            .as_string()
            .ok_or("Failed to read dictionary as string")?;

        Ok(Self::from_tsv(&text))
    }

    /// Parses the TSV text into an in-memory HashMap. Exposed separately so
    /// unit tests (added in a future pass) can cover it without the fetch.
    pub fn from_tsv(text: &str) -> Self {
        let mut map: HashMap<String, Vec<String>> = HashMap::new();

        for line in text.lines() {
            // Skip blank lines and comments.
            if line.is_empty() {
                continue;
            }
            if line.starts_with('#') {
                continue;
            }

            // Expect exactly one tab separator.
            let mut parts = line.splitn(2, '\t');
            let cyrillic = match parts.next() {
                Some(s) => s.trim(),
                None => continue,
            };
            let mongolian = match parts.next() {
                Some(s) => s.trim(),
                None => continue,
            };

            if cyrillic.is_empty() || mongolian.is_empty() {
                continue;
            }

            let key = cyrillic.to_lowercase();
            map.entry(key)
                .or_insert_with(Vec::new)
                .push(mongolian.to_string());
        }

        Self { map }
    }

    /// Returns the list of Mongolian variants for a given Cyrillic input.
    /// The lookup is case-insensitive and trims surrounding whitespace.
    pub fn lookup(&self, cyrillic: &str) -> Option<&[String]> {
        let key = cyrillic.trim().to_lowercase();
        if key.is_empty() {
            return None;
        }
        self.map.get(&key).map(|v| v.as_slice())
    }

    /// Total number of unique Cyrillic keys loaded.
    pub fn len(&self) -> usize {
        self.map.len()
    }
}
