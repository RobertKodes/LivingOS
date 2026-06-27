//! los-kernel — the Living Kernel.
//!
//! This is the heart of LivingOS's central thesis: **agents are first-class,
//! kernel-managed resources**, exactly like processes. The kernel does not run
//! any LLM itself (models stay in user space, served by the Intelligence
//! Router). Instead it owns the things an OS owns about a process:
//!
//!   * identity            — every agent has a stable [`AgentId`]
//!   * capabilities        — what an agent is *allowed* to do ([`Capability`])
//!   * lifecycle / state   — Ready / Running / Blocked / Done ([`AgentState`])
//!   * scheduling          — a priority run-queue of [`Task`]s ([`Scheduler`])
//!   * accountability      — every authorized action is logged ([`AuditEntry`])
//!   * reputation          — feeds the Evolution Engine
//!
//! Capability checks are mediated here, so the runtime can never perform a
//! privileged action (read a file, capture the screen, call a model, generate
//! an image) without the kernel authorizing it first — and authorizing it
//! means it is logged and explainable.

use std::collections::{BinaryHeap, HashMap};
use std::io::Write as _;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

pub type AgentId = u64;
pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

/// A capability is a scoped permission. Agents are granted a *set* of these at
/// spawn time and can never act outside them. This is the security model from
/// the PRD: "every agent receives capabilities instead of unrestricted
/// permissions."
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Capability {
    ReadFiles,
    WriteFiles,
    Internet,
    Terminal,
    ModelInference,
    Memory,
    ScreenCapture,
    ImageGen,
    Camera,
    Microphone,
    Speaker,
    Git,
    Compiler,
}

impl Capability {
    pub fn label(&self) -> &'static str {
        match self {
            Capability::ReadFiles => "read_files",
            Capability::WriteFiles => "write_files",
            Capability::Internet => "internet",
            Capability::Terminal => "terminal",
            Capability::ModelInference => "model_inference",
            Capability::Memory => "memory",
            Capability::ScreenCapture => "screen_capture",
            Capability::ImageGen => "image_gen",
            Capability::Camera => "camera",
            Capability::Microphone => "microphone",
            Capability::Speaker => "speaker",
            Capability::Git => "git",
            Capability::Compiler => "compiler",
        }
    }
}

impl std::fmt::Display for Capability {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.label())
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentState {
    Ready,
    Running,
    Blocked,
    Sleeping,
    Done,
}

/// The Agent Control Block — the agent equivalent of a Process Control Block.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct Acb {
    pub id: AgentId,
    pub name: String,
    pub role: String,
    pub caps: Vec<Capability>,
    pub state: AgentState,
    pub priority: u8,
    pub reputation: f32,
    pub tasks_done: u32,
    pub tasks_failed: u32,
}

impl Acb {
    pub fn has(&self, cap: Capability) -> bool {
        self.caps.contains(&cap)
    }
}

/// One line in the transparent-reasoning audit trail.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct AuditEntry {
    pub ts: u64,
    pub agent_id: AgentId,
    pub agent: String,
    pub action: String,
    pub detail: String,
    pub allowed: bool,
}

/// A unit of schedulable work, assigned to a specialist role.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct Task {
    pub id: u64,
    pub title: String,
    pub role: String,
    pub detail: String,
    pub priority: u8,
    pub assigned: Option<AgentId>,
}

// ---- Scheduler --------------------------------------------------------------

struct Prioritized {
    priority: u8,
    seq: u64,
    task: Task,
}

impl PartialEq for Prioritized {
    fn eq(&self, other: &Self) -> bool {
        self.priority == other.priority && self.seq == other.seq
    }
}
impl Eq for Prioritized {}
impl Ord for Prioritized {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        // Higher priority first; on a tie, lower seq (FIFO) first.
        self.priority
            .cmp(&other.priority)
            .then_with(|| other.seq.cmp(&self.seq))
    }
}
impl PartialOrd for Prioritized {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

/// The Native Agent Scheduler: a priority run-queue. Higher-priority tasks are
/// dispatched first; ties run FIFO.
#[derive(Default)]
pub struct Scheduler {
    inner: Mutex<SchedInner>,
}

#[derive(Default)]
struct SchedInner {
    queue: BinaryHeap<Prioritized>,
    next_id: u64,
    next_seq: u64,
}

impl Scheduler {
    pub fn new() -> Self {
        Self::default()
    }

    /// Enqueue work. Returns the assigned task id.
    pub fn submit(&self, title: impl Into<String>, role: impl Into<String>, detail: impl Into<String>, priority: u8, assigned: Option<AgentId>) -> u64 {
        let mut g = self.inner.lock().unwrap();
        g.next_id += 1;
        let id = g.next_id;
        let seq = g.next_seq;
        g.next_seq += 1;
        let task = Task { id, title: title.into(), role: role.into(), detail: detail.into(), priority, assigned };
        g.queue.push(Prioritized { priority, seq, task });
        id
    }

    /// Pop the next task to run, highest priority first.
    pub fn next(&self) -> Option<Task> {
        let mut g = self.inner.lock().unwrap();
        g.queue.pop().map(|p| p.task)
    }

    pub fn pending(&self) -> usize {
        self.inner.lock().unwrap().queue.len()
    }
}

// ---- Kernel -----------------------------------------------------------------

pub struct Kernel {
    inner: Mutex<KernelInner>,
    audit_path: Option<std::path::PathBuf>,
}

struct KernelInner {
    next_id: AgentId,
    agents: HashMap<AgentId, Acb>,
    audit: Vec<AuditEntry>,
}

fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

impl Kernel {
    pub fn new(audit_path: Option<std::path::PathBuf>) -> Self {
        Kernel {
            inner: Mutex::new(KernelInner { next_id: 0, agents: HashMap::new(), audit: Vec::new() }),
            audit_path,
        }
    }

    /// Register a new agent as a first-class kernel resource.
    pub fn spawn(&self, name: impl Into<String>, role: impl Into<String>, caps: Vec<Capability>, priority: u8) -> AgentId {
        let mut g = self.inner.lock().unwrap();
        g.next_id += 1;
        let id = g.next_id;
        let acb = Acb {
            id,
            name: name.into(),
            role: role.into(),
            caps,
            state: AgentState::Ready,
            priority,
            reputation: 1.0,
            tasks_done: 0,
            tasks_failed: 0,
        };
        g.agents.insert(id, acb);
        id
    }

    pub fn agents(&self) -> Vec<Acb> {
        let g = self.inner.lock().unwrap();
        let mut v: Vec<Acb> = g.agents.values().cloned().collect();
        v.sort_by_key(|a| a.id);
        v
    }

    pub fn get(&self, id: AgentId) -> Option<Acb> {
        self.inner.lock().unwrap().agents.get(&id).cloned()
    }

    pub fn find_by_role(&self, role: &str) -> Option<AgentId> {
        let g = self.inner.lock().unwrap();
        g.agents.values().find(|a| a.role.eq_ignore_ascii_case(role)).map(|a| a.id)
    }

    pub fn set_state(&self, id: AgentId, state: AgentState) {
        if let Some(a) = self.inner.lock().unwrap().agents.get_mut(&id) {
            a.state = state;
        }
    }

    /// The capability gate. Every privileged action funnels through here, so
    /// every privileged action is both *checked* and *logged*. Returns Err if
    /// the agent lacks the capability.
    pub fn authorize(&self, id: AgentId, cap: Capability, reason: &str) -> Result<()> {
        let (name, allowed) = {
            let g = self.inner.lock().unwrap();
            match g.agents.get(&id) {
                Some(a) => (a.name.clone(), a.has(cap)),
                None => (format!("<unknown:{id}>"), false),
            }
        };
        self.log(id, &name, &format!("request:{cap}"), reason, allowed);
        if allowed {
            Ok(())
        } else {
            Err(format!("capability denied: agent '{name}' lacks '{cap}' ({reason})").into())
        }
    }

    /// Record the outcome of a task; updates reputation (Evolution Engine input).
    pub fn record_result(&self, id: AgentId, success: bool, note: &str) {
        let name = {
            let mut g = self.inner.lock().unwrap();
            if let Some(a) = g.agents.get_mut(&id) {
                if success {
                    a.tasks_done += 1;
                    a.reputation = (a.reputation + 0.1).min(5.0);
                } else {
                    a.tasks_failed += 1;
                    a.reputation = (a.reputation - 0.3).max(0.0);
                }
                a.state = AgentState::Ready;
                a.name.clone()
            } else {
                return;
            }
        };
        self.log(id, &name, if success { "task:success" } else { "task:failure" }, note, success);
    }

    fn log(&self, agent_id: AgentId, name: &str, action: &str, detail: &str, allowed: bool) {
        let entry = AuditEntry {
            ts: now(),
            agent_id,
            agent: name.to_string(),
            action: action.to_string(),
            detail: detail.to_string(),
            allowed,
        };
        if let Some(path) = &self.audit_path {
            if let Ok(line) = serde_json::to_string(&entry) {
                if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
                    let _ = writeln!(f, "{line}");
                }
            }
        }
        self.inner.lock().unwrap().audit.push(entry);
    }

    pub fn audit_tail(&self, n: usize) -> Vec<AuditEntry> {
        let g = self.inner.lock().unwrap();
        let len = g.audit.len();
        let start = len.saturating_sub(n);
        g.audit[start..].to_vec()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capability_gate_blocks_ungranted() {
        let k = Kernel::new(None);
        let a = k.spawn("Coder", "Coder", vec![Capability::ModelInference], 5);
        assert!(k.authorize(a, Capability::ModelInference, "test").is_ok());
        assert!(k.authorize(a, Capability::ScreenCapture, "test").is_err());
    }

    #[test]
    fn scheduler_is_priority_then_fifo() {
        let s = Scheduler::new();
        s.submit("low", "X", "", 1, None);
        s.submit("high", "X", "", 9, None);
        s.submit("low2", "X", "", 1, None);
        assert_eq!(s.next().unwrap().title, "high");
        assert_eq!(s.next().unwrap().title, "low");
        assert_eq!(s.next().unwrap().title, "low2");
    }

    #[test]
    fn reputation_moves_with_results() {
        let k = Kernel::new(None);
        let a = k.spawn("Tester", "Tester", vec![], 5);
        k.record_result(a, true, "ok");
        assert!(k.get(a).unwrap().reputation > 1.0);
        k.record_result(a, false, "bad");
        assert!(k.get(a).unwrap().reputation < 1.1);
    }
}
