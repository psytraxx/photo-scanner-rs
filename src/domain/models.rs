use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct VectorSearchResult {
    pub id: u64,
    pub score: f32,
    pub payload: HashMap<String, String>,
}
