//! The native agent subsystem of the Living Kernel.
//!
//! This is the thesis of LivingOS made concrete: the kernel manages **agents**
//! the way a conventional kernel manages processes. Identity, capabilities,
//! lifecycle state, scheduling priority, reputation, and an audit trail are all
//! native kernel data structures here — `no_std`, no operating system beneath
//! us. The large language models that give agents their intelligence run in
//! user space; the kernel owns the agents themselves.

use alloc::string::String;
use alloc::vec::Vec;

pub type AgentId = u64;

/// A scoped permission. Agents are granted a set at spawn time and the kernel
/// refuses any action outside it — every check is recorded in the audit trail.
#[derive(Clone, Copy, PartialEq, Eq)]
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
    pub fn label(self) -> &'static str {
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

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum AgentState {
    Ready,
    Running,
    Blocked,
    Done,
}

impl AgentState {
    pub fn label(self) -> &'static str {
        match self {
            AgentState::Ready => "Ready",
            AgentState::Running => "Running",
            AgentState::Blocked => "Blocked",
            AgentState::Done => "Done",
        }
    }
}

/// The Agent Control Block — the agent equivalent of a Process Control Block.
pub struct Acb {
    pub id: AgentId,
    pub name: &'static str,
    pub caps: Vec<Capability>,
    pub state: AgentState,
    pub priority: u8,
    pub reputation: f32,
    pub tasks_done: u32,
    pub tasks_failed: u32,
}

impl Acb {
    pub fn has(&self, cap: Capability) -> bool {
        self.caps.iter().any(|c| *c == cap)
    }
    pub fn caps_label(&self) -> String {
        let mut s = String::new();
        for (i, c) in self.caps.iter().enumerate() {
            if i > 0 {
                s.push_str(", ");
            }
            s.push_str(c.label());
        }
        s
    }
}

/// One line of the transparent-reasoning audit trail.
pub struct AuditLine {
    pub agent: &'static str,
    pub action: String,
    pub detail: String,
    pub allowed: bool,
}

/// A message routed between agents by the kernel. The "Agent Society" is real:
/// agents collaborate by sending these, and every one is logged.
pub struct Message {
    pub from: &'static str,
    pub to: &'static str,
    pub body: String,
}

/// A unit of schedulable work, routed to a specialist role.
pub struct Task {
    pub title: String,
    pub role: &'static str,
    pub priority: u8,
}

/// The Native Agent Scheduler: a priority run-queue. Highest priority is
/// dispatched first; ties run in submission order (FIFO).
pub struct Scheduler {
    queue: Vec<Task>,
}

impl Scheduler {
    pub fn new() -> Self {
        Scheduler { queue: Vec::new() }
    }

    pub fn submit(&mut self, title: String, role: &'static str, priority: u8) {
        self.queue.push(Task { title, role, priority });
    }

    pub fn pending(&self) -> usize {
        self.queue.len()
    }

    /// Pop the highest-priority task (FIFO among equal priorities).
    pub fn next(&mut self) -> Option<Task> {
        if self.queue.is_empty() {
            return None;
        }
        let mut best = 0usize;
        for i in 1..self.queue.len() {
            if self.queue[i].priority > self.queue[best].priority {
                best = i;
            }
        }
        Some(self.queue.remove(best))
    }
}

/// The Living Kernel's agent table, capability gate, audit log, and reputation
/// bookkeeping. Single-threaded by design — the kernel is the one scheduler.
pub struct Kernel {
    agents: Vec<Acb>,
    pub audit: Vec<AuditLine>,
    pub messages: Vec<Message>,
    next_id: AgentId,
}

impl Kernel {
    pub fn new() -> Self {
        Kernel { agents: Vec::new(), audit: Vec::new(), messages: Vec::new(), next_id: 0 }
    }

    pub fn name_of(&self, id: AgentId) -> &'static str {
        self.index(id).map(|i| self.agents[i].name).unwrap_or("<unknown>")
    }

    /// Route a message from one agent to another. Kernel-mediated, so it is
    /// recorded in both the message log and the audit trail.
    pub fn send(&mut self, from: AgentId, to: AgentId, body: &str) {
        let fname = self.name_of(from);
        let tname = self.name_of(to);
        self.messages.push(Message { from: fname, to: tname, body: String::from(body) });
        let mut detail = String::from("-> ");
        detail.push_str(tname);
        detail.push_str(": ");
        detail.push_str(body);
        self.audit.push(AuditLine { agent: fname, action: String::from("message:send"), detail, allowed: true });
    }

    pub fn spawn(&mut self, name: &'static str, caps: Vec<Capability>, priority: u8) -> AgentId {
        self.next_id += 1;
        let id = self.next_id;
        self.agents.push(Acb {
            id,
            name,
            caps,
            state: AgentState::Ready,
            priority,
            reputation: 1.0,
            tasks_done: 0,
            tasks_failed: 0,
        });
        id
    }

    pub fn agents(&self) -> &[Acb] {
        &self.agents
    }

    pub fn find(&self, name: &str) -> Option<AgentId> {
        self.agents.iter().find(|a| a.name.eq_ignore_ascii_case(name)).map(|a| a.id)
    }

    fn index(&self, id: AgentId) -> Option<usize> {
        self.agents.iter().position(|a| a.id == id)
    }

    pub fn set_state(&mut self, id: AgentId, state: AgentState) {
        if let Some(i) = self.index(id) {
            self.agents[i].state = state;
        }
    }

    /// The capability gate. Returns whether the action is permitted, and records
    /// the decision either way.
    pub fn authorize(&mut self, id: AgentId, cap: Capability, reason: &str) -> bool {
        let (name, allowed) = match self.index(id) {
            Some(i) => (self.agents[i].name, self.agents[i].has(cap)),
            None => ("<unknown>", false),
        };
        let mut action = String::from("request:");
        action.push_str(cap.label());
        self.audit.push(AuditLine {
            agent: name,
            action,
            detail: String::from(reason),
            allowed,
        });
        allowed
    }

    /// Record a task outcome and move reputation (the Evolution Engine signal).
    pub fn record(&mut self, id: AgentId, success: bool, note: &str) {
        if let Some(i) = self.index(id) {
            if success {
                self.agents[i].tasks_done += 1;
                let r = self.agents[i].reputation + 0.1;
                self.agents[i].reputation = if r > 5.0 { 5.0 } else { r };
            } else {
                self.agents[i].tasks_failed += 1;
                let r = self.agents[i].reputation - 0.3;
                self.agents[i].reputation = if r < 0.0 { 0.0 } else { r };
            }
            self.agents[i].state = AgentState::Ready;
        }
        let name = self.index(id).map(|i| self.agents[i].name).unwrap_or("<unknown>");
        self.audit.push(AuditLine {
            agent: name,
            action: String::from(if success { "task:success" } else { "task:failure" }),
            detail: String::from(note),
            allowed: success,
        });
    }
}
