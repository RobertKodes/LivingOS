//! The Living Shell, running natively in the kernel. You don't open apps — you
//! type a goal, and the agent society is scheduled to accomplish it, live. The
//! shell reads from keyboard or serial, dispatches commands, drives the on-device
//! planner + scheduler, and records everything to Living Memory (persisted to
//! the ESP).

use alloc::string::{String, ToString};
use uefi::proto::console::text::Color;

use crate::console;
use crate::memgraph::Memory;
use crate::planner;
use crate::sched::{AgentState, Capability, Kernel, Scheduler};
use crate::society;
use crate::fs;

pub struct Shell {
    k: Kernel,
    mem: Memory,
    loaded: bool,
}

enum Action {
    Continue,
    Shutdown,
}

impl Shell {
    pub fn new() -> Self {
        let mut k = Kernel::new();
        for spec in society::society() {
            k.spawn(spec.name, spec.caps, spec.priority);
        }
        let (mem, loaded) = match fs::load() {
            Some(blob) => (Memory::deserialize(&blob), true),
            None => (Memory::new(), false),
        };
        Shell { k, mem, loaded }
    }

    /// A one-time self-test at boot: prove the capability gate works (visible
    /// even on a headless capture before any input arrives).
    pub fn boot_selftest(&mut self) {
        console::set_color(Color::Yellow);
        kprintln!("[selftest] capability gate");
        console::reset_color();
        if let Some(eyes) = self.k.find("Eyes") {
            let ok = self.k.authorize(eyes, Capability::ScreenCapture, "selftest");
            kprintln!("  Eyes  -> screen_capture : {}", if ok { "GRANTED" } else { "DENIED" });
        }
        if let Some(coder) = self.k.find("Coder") {
            let ok = self.k.authorize(coder, Capability::ScreenCapture, "selftest");
            kprintln!("  Coder -> screen_capture : {}", if ok { "GRANTED" } else { "DENIED (correct)" });
        }
    }

    pub fn run(&mut self) -> ! {
        self.welcome();
        loop {
            console::set_color(Color::LightGreen);
            kprint!("\nliving> ");
            console::reset_color();
            let line = console::read_line();
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            match self.dispatch(line) {
                Action::Continue => {}
                Action::Shutdown => {
                    kprintln!("Shutting down LivingOS. Goodbye.");
                    uefi::runtime::reset(uefi::runtime::ResetType::SHUTDOWN, uefi::Status::SUCCESS, None);
                }
            }
        }
    }

    fn welcome(&self) {
        let (n, e) = self.mem.counts();
        console::set_color(Color::Cyan);
        kprintln!();
        kprintln!("  Welcome to LivingOS. The society is awake.");
        console::reset_color();
        if self.loaded {
            kprintln!("  Living Memory restored from disk: {} nodes, {} edges.", n, e);
        } else {
            kprintln!("  Fresh Living Memory (nothing persisted yet).");
        }
        kprintln!("  Type 'help' for commands, or just state a goal: goal <text>");
    }

    fn dispatch(&mut self, line: &str) -> Action {
        let (cmd, rest) = match line.split_once(' ') {
            Some((c, r)) => (c, r.trim()),
            None => (line, ""),
        };
        match cmd {
            "help" | "?" => self.cmd_help(),
            "goal" | "g" => self.cmd_goal(rest),
            "ps" | "agents" => self.cmd_ps(),
            "mem" | "memory" => self.cmd_mem(),
            "log" | "audit" => self.cmd_log(),
            "msgs" | "messages" => self.cmd_msgs(),
            "clear" | "cls" => console::clear(),
            "about" => self.cmd_about(),
            "shutdown" | "reboot" | "exit" => return Action::Shutdown,
            other => {
                console::set_color(Color::Red);
                kprintln!("unknown command: {}", other);
                console::reset_color();
                kprintln!("(try 'help'; or state a goal with: goal <text>)");
            }
        }
        Action::Continue
    }

    fn cmd_help(&self) {
        console::set_color(Color::Yellow);
        kprintln!("COMMANDS");
        console::reset_color();
        kprintln!("  goal <text>   assemble the society to pursue a goal");
        kprintln!("  ps            list agents (roles, state, reputation, capabilities)");
        kprintln!("  mem [        ] browse Living Memory (recent nodes)");
        kprintln!("  log           the transparent audit trail");
        kprintln!("  msgs          inter-agent messages (kernel-routed)");
        kprintln!("  clear         clear the screen");
        kprintln!("  about         what LivingOS is");
        kprintln!("  shutdown      power off the machine");
    }

    fn cmd_about(&self) {
        console::set_color(Color::Cyan);
        kprintln!("LivingOS");
        console::reset_color();
        kprintln!("  An AI-native OS where agents are first-class, kernel-managed");
        kprintln!("  resources. You express goals; the society assembles. Every");
        kprintln!("  privileged action is capability-gated and written to the audit");
        kprintln!("  trail. Memory persists across reboots.");
    }

    fn cmd_ps(&self) {
        console::set_color(Color::Yellow);
        kprintln!("  {:<3} {:<10} {:<8} {:<4} {}", "ID", "ROLE", "STATE", "REP", "CAPABILITIES");
        console::reset_color();
        for a in self.k.agents() {
            kprintln!(
                "  {:<3} {:<10} {:<8} {:<4} {}",
                a.id,
                a.name,
                a.state.label(),
                crate::rep(a.reputation),
                a.caps_label()
            );
        }
    }

    fn cmd_mem(&self) {
        let (n, e) = self.mem.counts();
        console::set_color(Color::Yellow);
        kprintln!("Living Memory — {} nodes, {} edges (most recent first)", n, e);
        console::reset_color();
        let mut any = false;
        for node in self.mem.recent(16) {
            any = true;
            kprintln!("  #{:<4} [{:<9}] {}", node.id, node.kind, node.label);
        }
        if !any {
            kprintln!("  (empty — state a goal to begin building experience)");
        }
    }

    fn cmd_log(&self) {
        console::set_color(Color::Yellow);
        kprintln!("AUDIT TRAIL (every privileged action is explainable)");
        console::reset_color();
        let len = self.k.audit.len();
        let start = len.saturating_sub(24);
        for line in &self.k.audit[start..] {
            if line.allowed {
                kprintln!("  ok      {:<10} {:<24} {}", line.agent, line.action, line.detail);
            } else {
                console::set_color(Color::Red);
                kprintln!("  DENIED  {:<10} {:<24} {}", line.agent, line.action, line.detail);
                console::reset_color();
            }
        }
    }

    fn cmd_goal(&mut self, goal: &str) {
        if goal.is_empty() {
            kprintln!("usage: goal <what you want>");
            return;
        }
        console::set_color(Color::Cyan);
        kprintln!("GOAL  \"{}\"", goal);
        console::reset_color();

        let gid = self.mem.add_node("Goal", goal);

        // Plan on-device, then schedule each task to its specialist.
        let tasks = planner::plan(goal);
        let mut sched = Scheduler::new();
        for t in &tasks {
            sched.submit(t.title.clone(), t.role, t.priority);
        }
        kprintln!("[plan] {} tasks; dispatching by priority...", tasks.len());

        while let Some(task) = sched.next() {
            if let Some(id) = self.k.find(task.role) {
                self.k.set_state(id, AgentState::Running);
                let needed = match task.role {
                    "Designer" => Capability::ImageGen,
                    "Eyes" => Capability::ScreenCapture,
                    _ => Capability::ModelInference,
                };
                let ok = self.k.authorize(id, needed, &task.title);
                if ok {
                    kprintln!("  [{:<10}] {}", task.role, task.title);
                } else {
                    console::set_color(Color::Red);
                    kprintln!("  [{:<10}] DENIED: {}", task.role, task.title);
                    console::reset_color();
                }
                self.k.record(id, ok, &task.title);
                let tnode = self.mem.add_node("Task", &task.title);
                self.mem.link(gid, tnode, "produced");
            }
        }

        // The society collaborates via kernel-routed messages.
        self.collaborate(&tasks);

        // Observer synthesizes (placeholder synthesis on-device).
        if let Some(obs) = self.k.find("Observer") {
            self.k.authorize(obs, Capability::ModelInference, "synthesize result");
            self.k.record(obs, true, "synthesized");
        }
        let summary = self.synthesize(goal, &tasks);
        let snode = self.mem.add_node("Answer", &summary);
        self.mem.link(gid, snode, "answered_by");

        console::set_color(Color::LightGreen);
        kprintln!("[done] {}", summary);
        console::reset_color();

        // Persist the updated graph.
        if fs::save(&self.mem.serialize()) {
            kprintln!("[mem] Living Memory persisted to disk.");
        }
    }

    fn relay(&mut self, from: &str, to: &str, body: &str) {
        if let (Some(f), Some(t)) = (self.k.find(from), self.k.find(to)) {
            self.k.send(f, t, body);
            console::set_color(Color::Magenta);
            kprintln!("  [msg] {:<10} -> {:<10} {}", from, to, body);
            console::reset_color();
        }
    }

    fn collaborate(&mut self, tasks: &[planner::PlanTask]) {
        let has = |r: &str| tasks.iter().any(|t| t.role == r);
        console::set_color(Color::Magenta);
        kprintln!("[society] agents coordinating...");
        console::reset_color();
        if has("Architect") {
            self.relay("Architect", "Researcher", "what constraints apply?");
            self.relay("Researcher", "Architect", "here is the domain context");
        }
        if has("Coder") {
            self.relay("Coder", "Tester", "core ready for validation");
        }
        if has("Security") && has("Coder") {
            self.relay("Security", "Coder", "address these risks before ship");
        }
        if has("Designer") {
            self.relay("Designer", "Observer", "visual asset attached");
        }
        for r in ["Researcher", "Architect", "Coder", "Tester", "Security"] {
            if has(r) {
                self.relay(r, "Observer", "task report filed");
            }
        }
    }

    fn cmd_msgs(&self) {
        console::set_color(Color::Yellow);
        kprintln!("AGENT MESSAGES (kernel-routed)");
        console::reset_color();
        let len = self.k.messages.len();
        if len == 0 {
            kprintln!("  (none yet — run a goal)");
            return;
        }
        let start = len.saturating_sub(20);
        for m in &self.k.messages[start..] {
            kprintln!("  {:<10} -> {:<10} {}", m.from, m.to, m.body);
        }
    }

    fn synthesize(&self, goal: &str, tasks: &[planner::PlanTask]) -> String {
        let mut roles = String::new();
        for (i, t) in tasks.iter().enumerate() {
            if i > 0 {
                roles.push_str(", ");
            }
            roles.push_str(t.role);
        }
        let mut s = String::from("Goal '");
        s.push_str(goal);
        s.push_str("' handled by ");
        s.push_str(&tasks.len().to_string());
        s.push_str(" agents (");
        s.push_str(&roles);
        s.push_str(").");
        s
    }
}
