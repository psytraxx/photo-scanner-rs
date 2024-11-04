use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct VectorOutput {
    pub id: u64,
    pub score: Option<f32>,
    pub payload: HashMap<String, String>,
}

#[derive(Debug, Clone)]
pub struct VectorInput {
    pub id: u64,
    pub embedding: Vec<f32>,
    pub payload: HashMap<String, String>,
}
