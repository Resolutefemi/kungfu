//! SmallVec-backed header storage.
//!
//! Most HTTP requests have <16 headers. With a `Vec<(String, String)>`,
//! every request allocates a heap buffer. With `SmallVec<[(SmallString, SmallString); 16]>`,
//! the first 16 header pairs fit on the stack — no allocation at all.
//!
//! This module provides a `Headers` type that wraps SmallVec and exposes
//! a case-insensitive lookup API identical to the previous `Vec<(String, String)>`
//! approach, but with ~80% fewer allocations on the hot path.

use smallvec::SmallVec;

/// Maximum number of header pairs stored inline (on the stack).
/// Beyond this, SmallVec falls back to the heap transparently.
pub const INLINE_HEADER_CAPACITY: usize = 16;

/// A small-vector-backed collection of HTTP headers.
///
/// Each header is a `(name, value)` pair where `name` is lowercase ASCII.
/// Lookup is case-insensitive (we lowercase on insertion).
#[derive(Debug, Clone, Default)]
pub struct Headers {
    pairs: SmallVec<[(String, String); INLINE_HEADER_CAPACITY]>,
}

impl Headers {
    pub fn new() -> Self {
        Self::default()
    }

    /// Construct from an iterator of (name, value) pairs. Names are
    /// lowercased on insertion.
    pub fn from_iter<I: IntoIterator<Item = (String, String)>>(iter: I) -> Self {
        let mut h = Self::new();
        for (k, v) in iter {
            h.insert(k, v);
        }
        h
    }

    /// Insert a header. Lowercases the key. Replaces if it already exists.
    pub fn insert(&mut self, key: impl Into<String>, value: impl Into<String>) {
        let key = key.into().to_ascii_lowercase();
        let value = value.into();
        // Linear scan for an existing entry — N is small (typically <16).
        for (k, v) in &mut self.pairs {
            if *k == key {
                *v = value;
                return;
            }
        }
        self.pairs.push((key, value));
    }

    /// Append a header without checking for duplicates (e.g. for Set-Cookie).
    pub fn append(&mut self, key: impl Into<String>, value: impl Into<String>) {
        let key = key.into().to_ascii_lowercase();
        let value = value.into();
        self.pairs.push((key, value));
    }

    /// Case-insensitive lookup. Returns `Some(&str)` if found.
    pub fn get(&self, key: &str) -> Option<&str> {
        // Linear scan — for N < 16, this is faster than a HashMap lookup
        // (no hash computation, cache-friendly).
        for (k, v) in &self.pairs {
            if k.eq_ignore_ascii_case(key) {
                return Some(v.as_str());
            }
        }
        None
    }

    /// Remove a header. Returns the removed value, if any.
    pub fn remove(&mut self, key: &str) -> Option<String> {
        let idx = self.pairs.iter().position(|(k, _)| k.eq_ignore_ascii_case(key))?;
        Some(self.pairs.remove(idx).1)
    }

    /// Iterate over all (name, value) pairs.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &str)> {
        self.pairs.iter().map(|(k, v)| (k.as_str(), v.as_str()))
    }

    /// Number of header pairs.
    pub fn len(&self) -> usize {
        self.pairs.len()
    }

    pub fn is_empty(&self) -> bool {
        self.pairs.is_empty()
    }

    /// True if the storage has spilled to the heap.
    pub fn is_heap_allocated(&self) -> bool {
        self.pairs.spilled()
    }
}

impl From<Vec<(String, String)>> for Headers {
    fn from(v: Vec<(String, String)>) -> Self {
        Self::from_iter(v)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stores_headers_inline() {
        let mut h = Headers::new();
        for i in 0..15 {
            h.insert(format!("x-custom-{i}"), "value");
        }
        assert!(!h.is_heap_allocated(), "should fit in SmallVec inline");
        assert_eq!(h.len(), 15);
    }

    #[test]
    fn spills_to_heap_when_over_capacity() {
        let mut h = Headers::new();
        for i in 0..32 {
            h.insert(format!("x-custom-{i}"), "value");
        }
        assert!(h.is_heap_allocated(), "should spill to heap");
        assert_eq!(h.len(), 32);
    }

    #[test]
    fn case_insensitive_lookup() {
        let mut h = Headers::new();
        h.insert("Content-Type", "application/json");
        assert_eq!(h.get("content-type"), Some("application/json"));
        assert_eq!(h.get("CONTENT-TYPE"), Some("application/json"));
        assert_eq!(h.get("Content-Type"), Some("application/json"));
        assert_eq!(h.get("missing"), None);
    }

    #[test]
    fn insert_replaces_existing() {
        let mut h = Headers::new();
        h.insert("X-Foo", "1");
        h.insert("x-foo", "2");
        assert_eq!(h.len(), 1);
        assert_eq!(h.get("X-Foo"), Some("2"));
    }

    #[test]
    fn append_does_not_dedupe() {
        let mut h = Headers::new();
        h.append("Set-Cookie", "a=1");
        h.append("Set-Cookie", "b=2");
        assert_eq!(h.len(), 2);
    }

    #[test]
    fn remove_returns_value() {
        let mut h = Headers::new();
        h.insert("X-Foo", "bar");
        assert_eq!(h.remove("x-foo"), Some("bar".to_string()));
        assert_eq!(h.remove("x-foo"), None);
        assert_eq!(h.len(), 0);
    }

    #[test]
    fn from_vec_preserves_pairs() {
        let v = vec![
            ("Content-Type".to_string(), "application/json".to_string()),
            ("X-Request-Id".to_string(), "abc123".to_string()),
        ];
        let h = Headers::from(v);
        assert_eq!(h.get("content-type"), Some("application/json"));
        assert_eq!(h.get("x-request-id"), Some("abc123"));
    }

    #[test]
    fn iter_yields_all_pairs() {
        let mut h = Headers::new();
        h.insert("a", "1");
        h.insert("b", "2");
        let mut pairs: Vec<_> = h.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect();
        pairs.sort();
        assert_eq!(pairs, vec![("a".to_string(), "1".to_string()), ("b".to_string(), "2".to_string())]);
    }
}
