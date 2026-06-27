//! los-memory — Living Memory.
//!
//! A persistent graph store. Where a traditional OS has a filesystem of bytes,
//! LivingOS has a graph of *meaning*: goals, tasks, knowledge, agents,
//! successes and failures, all linked by typed relationships. Agents build
//! long-term experience across sessions instead of restarting cold.
//!
//! The store is intentionally dependency-light: nodes + edges serialized to a
//! single JSON file. An optional embedding vector lives on each node so the
//! Intelligence Router's embedding model can power semantic recall later.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Node {
    pub id: u64,
    /// e.g. "Goal", "Task", "Knowledge", "Agent", "Observation", "Image", "Success", "Failure"
    pub kind: String,
    pub label: String,
    #[serde(default)]
    pub data: serde_json::Value,
    pub ts: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub embedding: Option<Vec<f32>>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Edge {
    pub from: u64,
    pub to: u64,
    pub rel: String,
}

#[derive(Default, Serialize, Deserialize)]
struct Graph {
    next_id: u64,
    nodes: Vec<Node>,
    edges: Vec<Edge>,
}

pub struct Memory {
    path: PathBuf,
    graph: Graph,
}

fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

impl Memory {
    /// Load the graph from disk, or start an empty one if the file is absent.
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        let graph = if path.exists() {
            let bytes = std::fs::read(&path)?;
            serde_json::from_slice(&bytes).unwrap_or_default()
        } else {
            Graph::default()
        };
        Ok(Memory { path, graph })
    }

    pub fn save(&self) -> Result<()> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let bytes = serde_json::to_vec_pretty(&self.graph)?;
        std::fs::write(&self.path, bytes)?;
        Ok(())
    }

    pub fn add_node(&mut self, kind: &str, label: impl Into<String>, data: serde_json::Value) -> u64 {
        self.graph.next_id += 1;
        let id = self.graph.next_id;
        self.graph.nodes.push(Node {
            id,
            kind: kind.to_string(),
            label: label.into(),
            data,
            ts: now(),
            embedding: None,
        });
        id
    }

    pub fn set_embedding(&mut self, id: u64, embedding: Vec<f32>) {
        if let Some(n) = self.graph.nodes.iter_mut().find(|n| n.id == id) {
            n.embedding = Some(embedding);
        }
    }

    pub fn link(&mut self, from: u64, to: u64, rel: &str) {
        self.graph.edges.push(Edge { from, to, rel: rel.to_string() });
    }

    pub fn node(&self, id: u64) -> Option<&Node> {
        self.graph.nodes.iter().find(|n| n.id == id)
    }

    pub fn by_kind(&self, kind: &str) -> Vec<&Node> {
        self.graph.nodes.iter().filter(|n| n.kind.eq_ignore_ascii_case(kind)).collect()
    }

    pub fn recent(&self, n: usize) -> Vec<&Node> {
        let mut v: Vec<&Node> = self.graph.nodes.iter().collect();
        v.sort_by(|a, b| b.id.cmp(&a.id));
        v.truncate(n);
        v
    }

    pub fn neighbors(&self, id: u64) -> Vec<(&Edge, &Node)> {
        self.graph
            .edges
            .iter()
            .filter(|e| e.from == id)
            .filter_map(|e| self.node(e.to).map(|n| (e, n)))
            .collect()
    }

    /// Cheap substring recall. When embeddings are present and a query vector is
    /// supplied, prefer [`Memory::semantic_search`].
    pub fn search(&self, query: &str) -> Vec<&Node> {
        let q = query.to_lowercase();
        self.graph
            .nodes
            .iter()
            .filter(|n| n.label.to_lowercase().contains(&q))
            .collect()
    }

    /// Cosine-similarity recall over nodes that have embeddings.
    pub fn semantic_search(&self, query_vec: &[f32], top_k: usize) -> Vec<(&Node, f32)> {
        let mut scored: Vec<(&Node, f32)> = self
            .graph
            .nodes
            .iter()
            .filter_map(|n| n.embedding.as_ref().map(|e| (n, cosine(query_vec, e))))
            .collect();
        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        scored.truncate(top_k);
        scored
    }

    pub fn stats(&self) -> (usize, usize) {
        (self.graph.nodes.len(), self.graph.edges.len())
    }
}

fn cosine(a: &[f32], b: &[f32]) -> f32 {
    let n = a.len().min(b.len());
    let mut dot = 0.0f32;
    let mut na = 0.0f32;
    let mut nb = 0.0f32;
    for i in 0..n {
        dot += a[i] * b[i];
        na += a[i] * a[i];
        nb += b[i] * b[i];
    }
    if na == 0.0 || nb == 0.0 {
        0.0
    } else {
        dot / (na.sqrt() * nb.sqrt())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nodes_and_edges_round_trip() {
        let dir = std::env::temp_dir().join(format!("losmem-{}", now()));
        let path = dir.join("mem.json");
        let mut m = Memory::open(&path).unwrap();
        let g = m.add_node("Goal", "build a game", serde_json::json!({}));
        let k = m.add_node("Knowledge", "use a game loop", serde_json::json!({}));
        m.link(g, k, "produced");
        m.save().unwrap();

        let m2 = Memory::open(&path).unwrap();
        assert_eq!(m2.by_kind("Goal").len(), 1);
        assert_eq!(m2.neighbors(g).len(), 1);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
