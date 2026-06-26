use crate::benchmarks::types::BenchmarkEntry;

pub fn load_static() -> Vec<BenchmarkEntry> {
    let raw: &str = include_str!("static_data.json");
    serde_json::from_str(raw).unwrap_or_else(|e| {
        eprintln!("Warning: failed to parse static benchmarks: {e}");
        vec![]
    })
}
