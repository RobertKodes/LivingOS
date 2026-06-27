//! Living Memory, in the kernel. A persistent graph of what the OS has done:
//! goals, the plans they produced, the knowledge/observations that came back,
//! and how they link. Held in RAM while running and serialised to a flat text
//! format so it can be written to the EFI System Partition and survive reboots
//! (see `fs.rs`).

use alloc::string::{String, ToString};
use alloc::vec::Vec;

#[derive(Clone)]
pub struct Node {
    pub id: u64,
    pub kind: String,
    pub label: String,
}

#[derive(Clone)]
pub struct Edge {
    pub from: u64,
    pub to: u64,
    pub rel: String,
}

#[derive(Default)]
pub struct Memory {
    next_id: u64,
    nodes: Vec<Node>,
    edges: Vec<Edge>,
}

fn sanitize(s: &str) -> String {
    s.chars().map(|c| if c == '|' || c == '\n' || c == '\r' { ' ' } else { c }).collect()
}

impl Memory {
    pub fn new() -> Self {
        Memory::default()
    }

    pub fn add_node(&mut self, kind: &str, label: &str) -> u64 {
        self.next_id += 1;
        let id = self.next_id;
        self.nodes.push(Node { id, kind: kind.to_string(), label: sanitize(label) });
        id
    }

    pub fn link(&mut self, from: u64, to: u64, rel: &str) {
        self.edges.push(Edge { from, to, rel: rel.to_string() });
    }

    pub fn counts(&self) -> (usize, usize) {
        (self.nodes.len(), self.edges.len())
    }

    pub fn recent(&self, n: usize) -> impl Iterator<Item = &Node> {
        self.nodes.iter().rev().take(n)
    }

    pub fn by_kind<'a>(&'a self, kind: &'a str) -> impl Iterator<Item = &'a Node> {
        self.nodes.iter().filter(move |n| n.kind.eq_ignore_ascii_case(kind))
    }

    /// Serialise to a flat, line-oriented text blob for persistence.
    pub fn serialize(&self) -> String {
        let mut s = String::new();
        s.push_str("LIVINGOS-MEM-1\n");
        for n in &self.nodes {
            s.push_str("N|");
            push_u64(&mut s, n.id);
            s.push('|');
            s.push_str(&n.kind);
            s.push('|');
            s.push_str(&n.label);
            s.push('\n');
        }
        for e in &self.edges {
            s.push_str("E|");
            push_u64(&mut s, e.from);
            s.push('|');
            push_u64(&mut s, e.to);
            s.push('|');
            s.push_str(&e.rel);
            s.push('\n');
        }
        s
    }

    /// Reload from a blob produced by [`Memory::serialize`].
    pub fn deserialize(blob: &str) -> Self {
        let mut m = Memory::new();
        for line in blob.lines() {
            if line.starts_with("N|") {
                let mut it = line[2..].splitn(3, '|');
                if let (Some(id), Some(kind), Some(label)) = (it.next(), it.next(), it.next()) {
                    let id = id.parse::<u64>().unwrap_or(0);
                    m.nodes.push(Node { id, kind: kind.to_string(), label: label.to_string() });
                    if id > m.next_id {
                        m.next_id = id;
                    }
                }
            } else if line.starts_with("E|") {
                let mut it = line[2..].splitn(3, '|');
                if let (Some(f), Some(t), Some(rel)) = (it.next(), it.next(), it.next()) {
                    m.edges.push(Edge {
                        from: f.parse().unwrap_or(0),
                        to: t.parse().unwrap_or(0),
                        rel: rel.to_string(),
                    });
                }
            }
        }
        m
    }
}

fn push_u64(s: &mut String, mut v: u64) {
    if v == 0 {
        s.push('0');
        return;
    }
    let mut buf = [0u8; 20];
    let mut i = buf.len();
    while v > 0 {
        i -= 1;
        buf[i] = b'0' + (v % 10) as u8;
        v /= 10;
    }
    s.push_str(core::str::from_utf8(&buf[i..]).unwrap_or(""));
}
