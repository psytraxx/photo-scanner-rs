use std::collections::HashMap;

#[derive(Debug)]
pub struct VectorSearchResult {
    pub id: u64,
    pub score: f32,
    pub payload: HashMap<String, String>,
}