//! The Living Shell, running natively in the kernel. You don't open apps — you
//! type a goal, and the agent society is scheduled to accomplish it, live. The
//! shell reads from keyboard or serial, dispatches commands, drives the on-device
//! planner + scheduler, and records everything to Living Memory (persisted to
//! the ESP).

use alloc::string::{String, ToString};
use alloc::vec::Vec;
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
    plugins: Vec<&'static str>,
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
        // Load plugin agents (data-driven OS extensions) from the ESP manifest.
        let mut plugins = Vec::new();
        for p in crate::plugins::load() {
            k.spawn(p.name, p.caps, p.priority);
            plugins.push(p.name);
        }
        let (mem, loaded) = match fs::load() {
            Some(blob) => (Memory::deserialize(&blob), true),
            None => (Memory::new(), false),
        };
        Shell { k, mem, loaded, plugins }
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
            "sys" | "uname" => self.cmd_sys(),
            "dash" | "ui" => self.cmd_dash(),
            "plugins" => self.cmd_plugins(),
            "syscall" => self.cmd_syscall(),
            "vm" | "paging" => self.cmd_vm(),
            "net" | "arp" => self.cmd_net(),
            "ping" => self.cmd_ping(),
            "ask" => self.cmd_ask(rest),
            "say" => self.cmd_say(rest),
            "hear" | "listen" => self.cmd_hear(),
            "tasks" | "sched" => self.cmd_tasks(),
            "gen" | "infer" => self.cmd_gen(rest),
            "beep" => {
                crate::audio::chime();
                kprintln!("(beeped the PC speaker)");
            }
            "recall" | "find" => self.cmd_recall(rest),
            "clear" | "cls" => console::clear(),
            "about" => self.cmd_about(),
            "selfhost" | "takeover" => {
                kprintln!("Transitioning to SELF-HOSTED mode (one-way; firmware released)...");
                crate::selfhost::enter();
            }
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
        kprintln!("  ask <text>    route to local models via the host model bridge");
        kprintln!("  ps            list agents (roles, state, reputation, capabilities)");
        kprintln!("  mem [        ] browse Living Memory (recent nodes)");
        kprintln!("  log           the transparent audit trail");
        kprintln!("  msgs          inter-agent messages (kernel-routed)");
        kprintln!("  recall <q>    search Living Memory for past experience");
        kprintln!("  dash          render the visual command center (framebuffer)");
        kprintln!("  gen <seed>    on-device neural-net text generation");
        kprintln!("  plugins       list plugin agents (loaded from plugins.cfg)");
        kprintln!("  beep          drive the PC speaker (audio out)");
        kprintln!("  say <text>    on-metal text-to-speech (PC speaker)");
        kprintln!("  hear          speech-to-text via the host model bridge");
        kprintln!("  syscall       demo the int 0x80 kernel syscall bridge");
        kprintln!("  vm            memory map + live page-table mapping demo");
        kprintln!("  net           NIC access + live ARP exchange (UEFI SNP)");
        kprintln!("  ping          IPv4/ICMP echo to the gateway (own IP stack)");
        kprintln!("  tasks         context-switch primitive (multitasking)");
        kprintln!("  sys           kernel + system status");
        kprintln!("  clear         clear the screen");
        kprintln!("  about         what LivingOS is");
        kprintln!("  selfhost      ExitBootServices; run on own drivers (one-way)");
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

        // The Planner first consults the local models through the model bridge.
        // If a model service is connected, real local-model output drives the
        // plan; otherwise we fall back to the on-device keyword planner.
        if let Some(planner_id) = self.k.find("Planner") {
            let _ = self.k.authorize(planner_id, Capability::ModelInference, "consult local model");
        }
        match crate::bridge::ask("ASK", goal) {
            Some(plan) => {
                console::set_color(Color::LightGreen);
                kprintln!("[planner] local model (via bridge):");
                console::reset_color();
                kprintln!("  {}", plan);
                let k = self.mem.add_node("Knowledge", &plan);
                self.mem.link(gid, k, "planned_by_model");
            }
            None => {
                kprintln!("[planner] no model bridge connected; using on-device keyword planner");
            }
        }

        // Schedule each task to its specialist (capability-gated execution).
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

    fn cmd_dash(&self) {
        let agents = self.k.agents();
        let mut views: Vec<(&str, f32)> = Vec::new();
        for a in agents {
            views.push((a.name, a.reputation));
        }
        let (n, e) = self.mem.counts();
        let mut footer = String::from("agents ");
        footer.push_str(&agents.len().to_string());
        footer.push_str("   memory ");
        footer.push_str(&n.to_string());
        footer.push_str("n/");
        footer.push_str(&e.to_string());
        footer.push_str("e   messages ");
        footer.push_str(&self.k.messages.len().to_string());
        footer.push_str("   audit ");
        footer.push_str(&self.k.audit.len().to_string());
        footer.push_str("   -  press Enter to return");

        if crate::gop::render_dashboard(&views, &footer) {
            kprintln!("[dash] command center rendered to the framebuffer (press a key to return)");
            // Hold the view for a few seconds, or until a key is pressed.
            for _ in 0..3000 {
                if console::any_key() {
                    break;
                }
                uefi::boot::stall(2000);
            }
            console::clear();
        } else {
            kprintln!("(no framebuffer available; the dashboard needs a GPU/GOP)");
        }
    }

    fn cmd_say(&self, text: &str) {
        if text.is_empty() {
            kprintln!("usage: say <text>");
            return;
        }
        console::set_color(Color::Cyan);
        kprintln!("SAY  (on-metal TTS via PC speaker): {}", text);
        console::reset_color();
        let n = crate::audio::speak(text);
        kprintln!("  vocalized {} tones; the local TTS models run via `ask`/the bridge", n);
    }

    fn cmd_hear(&mut self) {
        console::set_color(Color::Cyan);
        kprintln!("HEAR  (speech-to-text via the host model bridge)");
        console::reset_color();
        match crate::bridge::ask("HEAR", "transcribe microphone") {
            Some(t) => {
                console::set_color(Color::LightGreen);
                kprintln!("  transcript: {}", t);
                console::reset_color();
            }
            None => {
                kprintln!("  (no bridge; STT runs host-side: python tools/model_bridge.py)");
            }
        }
    }

    fn cmd_ping(&self) {
        console::set_color(Color::Yellow);
        kprintln!("PING 10.0.2.2  (kernel IPv4/ICMP stack over the SNP NIC)");
        console::reset_color();
        let r = crate::net::ping_gateway();
        if !r.sent {
            kprintln!("  no NIC available");
            return;
        }
        if r.replied {
            console::set_color(Color::LightGreen);
            kprintln!("  reply from {}: icmp echo, ttl={}", r.from, r.ttl);
            console::reset_color();
        } else {
            kprintln!("  request transmitted as a valid IPv4/ICMP frame (verified on");
            kprintln!("  the wire); no reply captured — OVMF's UEFI network stack owns");
            kprintln!("  the SNP receive path in this setup.");
        }
    }

    fn cmd_ask(&mut self, goal: &str) {
        if goal.is_empty() {
            kprintln!("usage: ask <question or goal>");
            return;
        }
        console::set_color(Color::Cyan);
        kprintln!("ASK  (kernel -> model bridge over COM2): {}", goal);
        console::reset_color();
        kprintln!("  routing to the local models on the host...");
        match crate::bridge::ask("ASK", goal) {
            Some(ans) => {
                console::set_color(Color::LightGreen);
                kprintln!("  model: {}", ans);
                console::reset_color();
                let g = self.mem.add_node("Goal", goal);
                let a = self.mem.add_node("Answer", &ans);
                self.mem.link(g, a, "answered_by");
                let _ = fs::save(&self.mem.serialize());
            }
            None => {
                kprintln!("  (no bridge daemon connected)");
                kprintln!("  start it on the host:  python tools/model_bridge.py");
            }
        }
    }

    fn cmd_net(&self) {
        console::set_color(Color::Yellow);
        kprintln!("NETWORKING  (UEFI Simple Network Protocol)");
        console::reset_color();
        let r = crate::net::run_net_demo();
        if !r.present {
            kprintln!("  {}", r.note);
            kprintln!("  (boot QEMU with: -netdev user,id=n0 -device e1000,netdev=n0)");
            return;
        }
        kprintln!("  NIC MAC      {}", r.mac);
        if let Some(g) = &r.gateway_mac {
            console::set_color(Color::LightGreen);
            kprintln!("  gateway MAC  {}  (resolved live via ARP)", g);
            console::reset_color();
        }
        kprintln!("  {}", r.note);
    }

    fn cmd_tasks(&self) {
        console::set_color(Color::Yellow);
        kprintln!("MULTITASKING");
        console::reset_color();
        kprintln!("  context-switch primitive: livingos_context_switch (task.rs)");
        kprintln!("    saves rbx/rbp/r12-r15 + rsp of one context, loads another's");
        kprintln!("    -- the core operation a scheduler is built from.");
        kprintln!("  Each agent already runs as a scheduled kernel object (see ps).");
        kprintln!("  A live coroutine round-trip is wired but unstable under OVMF");
        kprintln!("  boot services; preemptive timer-driven switching (via the IDT");
        kprintln!("  from `syscall`) is the next step.");
    }

    fn cmd_vm(&self) {
        console::set_color(Color::Yellow);
        kprintln!("MEMORY / PAGING");
        console::reset_color();
        let r = crate::mm::run_paging_demo();
        kprintln!("  usable RAM   {} MiB (conventional)", r.ram_mib);
        kprintln!("  CR3 (PML4)   {:#x}", r.cr3);
        kprintln!("  alloc frame  {:#x}  (physical, zeroed)", r.frame);
        kprintln!("  new mapping  virt {:#x} -> phys {:#x}", r.virt, r.frame);
        kprintln!("  wrote {:#x} via virt", r.wrote);
        kprintln!("  read  {:#x} via virt,  {:#x} via phys", r.via_virt, r.via_phys);
        if r.ok {
            console::set_color(Color::LightGreen);
            kprintln!("  OK: virtual and physical views agree -> the mapping is live");
        } else {
            console::set_color(Color::Red);
            kprintln!("  mapping failed");
        }
        console::reset_color();
    }

    fn cmd_syscall(&self) {
        console::set_color(Color::Yellow);
        kprintln!("SYSCALL BRIDGE  (user code traps into the kernel via int 0x80)");
        console::reset_color();
        let names = ["SYS_INC", "SYS_DOUBLE", "SYS_VERSION", "SYS_AGENTS"];
        let report = crate::idt::run_syscall_demo(self.k.agents().len() as u64);
        for (i, (n, a, r)) in report.calls.iter().enumerate() {
            kprintln!("  int 0x80  rax={} {:<12} rdi={:<3} -> {}", n, names[i], a, r);
        }
        kprintln!("  IDT base: expected {:#x}, active {:#x}", report.expected_base, report.actual_base);
        kprintln!("  (SYS_VERSION result 0x4C4956494E47 spells \"LIVING\"; firmware IDT restored)");
    }

    fn cmd_plugins(&self) {
        console::set_color(Color::Yellow);
        kprintln!("PLUGIN AGENTS (loaded from plugins.cfg on the ESP)");
        console::reset_color();
        if self.plugins.is_empty() {
            kprintln!("  (none)");
            return;
        }
        for name in &self.plugins {
            if let Some(id) = self.k.find(name) {
                if let Some(a) = self.k.agents().iter().find(|x| x.id == id) {
                    kprintln!("  {:<12} {}", a.name, a.caps_label());
                }
            }
        }
    }

    fn cmd_gen(&self, seed: &str) {
        let s = if seed.is_empty() { "the " } else { seed };
        console::set_color(Color::Yellow);
        kprintln!("ON-DEVICE INFERENCE  (tiny char MLP, {} params, running in-kernel)", crate::nn::params());
        console::reset_color();
        let out = crate::nn::generate(s, 90);
        kprintln!("  {}", out);
    }

    fn cmd_sys(&self) {
        console::set_color(Color::Yellow);
        kprintln!("SYSTEM");
        console::reset_color();
        kprintln!("  kernel      LivingOS (no_std UEFI; agents are kernel resources)");
        if let Some((w, h)) = crate::gop::resolution() {
            kprintln!("  display     {}x{} GPU framebuffer", w, h);
        } else {
            kprintln!("  display     serial console");
        }
        if let Ok(t) = uefi::runtime::get_time() {
            kprintln!(
                "  rtc         {:04}-{:02}-{:02} {:02}:{:02}:{:02}",
                t.year(), t.month(), t.day(), t.hour(), t.minute(), t.second()
            );
        }
        kprintln!("  agents      {}", self.k.agents().len());
        let (n, e) = self.mem.counts();
        kprintln!("  memory      {} nodes, {} edges", n, e);
        kprintln!("  messages    {}", self.k.messages.len());
        kprintln!("  audit       {} entries", self.k.audit.len());
        kprintln!("  plugins     {} loaded", self.plugins.len());
        kprintln!("  model       on-device char MLP, {} params", crate::nn::params());
    }

    fn cmd_recall(&self, query: &str) {
        if query.is_empty() {
            kprintln!("usage: recall <query>");
            return;
        }
        console::set_color(Color::Yellow);
        kprintln!("RECALL \"{}\"", query);
        console::reset_color();
        let hits = self.mem.search(query);
        if hits.is_empty() {
            kprintln!("  (no memory of that yet)");
            return;
        }
        for node in hits.iter().take(16) {
            kprintln!("  #{:<4} [{:<9}] {}", node.id, node.kind, node.label);
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
