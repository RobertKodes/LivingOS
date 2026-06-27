//! los-runtime — the Agent Runtime and the live Agent Society.
//!
//! Boots a [`LivingOs`] instance: it spawns every agent in the society as a
//! first-class kernel resource, opens Living Memory, and wires up the
//! Intelligence Router. The high-level verbs the Living Shell exposes —
//! `goal`, `see`, `design` — are implemented here as collaborations between
//! agents, mediated by the kernel's capability gate and recorded in memory.

mod society;

pub use society::{society, AgentSpec};

use los_kernel::{AgentId, AgentState, Capability, Kernel, Scheduler};
use los_memory::Memory;
use los_router::{FleetConfig, Role, Router};
use serde::Deserialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

/// A live progress sink so the shell can stream what the society is doing.
pub type Progress<'a> = &'a mut dyn FnMut(&str);

pub struct LivingOs {
    pub kernel: Kernel,
    pub memory: Memory,
    pub router: Router,
    pub scheduler: Scheduler,
    specs: Vec<AgentSpec>,
    roles: HashMap<String, AgentId>,
    data_dir: PathBuf,
    gallery_dir: PathBuf,
}

#[derive(Deserialize)]
struct PlanItem {
    title: String,
    role: String,
    detail: String,
}

impl LivingOs {
    /// Boot the operating system: load config, open memory, spawn the society.
    pub fn boot(root: impl AsRef<Path>) -> Result<Self> {
        let root = root.as_ref();
        let config_path = root.join("config").join("fleet.json");
        let data_dir = root.join("data");
        let gallery_dir = data_dir.join("gallery");
        std::fs::create_dir_all(&gallery_dir)?;

        let cfg = load_or_init_config(&config_path)?;
        let router = Router::new(cfg);
        let memory = Memory::open(data_dir.join("memory.json"))?;
        let kernel = Kernel::new(Some(data_dir.join("audit.jsonl")));

        let specs = society();
        let mut roles = HashMap::new();
        for s in &specs {
            let id = kernel.spawn(s.role, s.role, s.caps.to_vec(), s.priority);
            roles.insert(s.role.to_string(), id);
        }

        Ok(LivingOs {
            kernel,
            memory,
            router,
            scheduler: Scheduler::new(),
            specs,
            roles,
            data_dir,
            gallery_dir,
        })
    }

    fn spec_for(&self, role: &str) -> Option<&AgentSpec> {
        self.specs.iter().find(|s| s.role.eq_ignore_ascii_case(role))
    }

    fn agent_for(&self, role: &str) -> Option<AgentId> {
        self.roles.get(role).copied()
    }

    // ----------------------------------------------------------------------
    // Verb: goal — the full society collaborating on a user goal.
    // ----------------------------------------------------------------------

    pub fn run_goal(&mut self, goal: &str, emit: Progress) -> Result<String> {
        emit(&format!("◆ GOAL: {goal}"));
        let goal_id = self.memory.add_node("Goal", goal, serde_json::json!({"source": "user"}));

        // 1. Planner decomposes the goal.
        let planner = self.agent_for("Planner").ok_or("no Planner agent")?;
        let planner_sys = self.spec_for("Planner").unwrap().system;
        self.kernel.authorize(planner, Capability::ModelInference, "decompose goal")?;
        self.kernel.set_state(planner, AgentState::Running);
        emit("  ▸ Planner is decomposing the goal…");
        let plan = match self.router.chat(Role::Planning, planner_sys, goal) {
            Ok(raw) => {
                self.kernel.record_result(planner, true, "planned");
                parse_plan(&raw)
            }
            Err(e) => {
                self.kernel.record_result(planner, false, &e.to_string());
                return Err(e);
            }
        };
        let plan = if plan.is_empty() {
            vec![PlanItem { title: goal.to_string(), role: "Researcher".into(), detail: goal.to_string() }]
        } else {
            plan
        };

        let plan_id = self.memory.add_node(
            "Plan",
            format!("{} tasks", plan.len()),
            serde_json::json!({"tasks": plan.iter().map(|p| &p.title).collect::<Vec<_>>()}),
        );
        self.memory.link(goal_id, plan_id, "decomposed_into");
        emit(&format!("  ✓ Plan: {} tasks", plan.len()));

        // 2. Schedule each task to its specialist.
        for item in &plan {
            let agent = self.agent_for(&item.role).or_else(|| self.agent_for("Researcher"));
            let prio = self.spec_for(&item.role).map(|s| s.priority).unwrap_or(5);
            self.scheduler.submit(item.title.clone(), item.role.clone(), item.detail.clone(), prio, agent);
        }

        // 3. Run the queue, highest priority first. Each result feeds the next.
        let mut context = String::new();
        while let Some(task) = self.scheduler.next() {
            let role = if self.spec_for(&task.role).is_some() { task.role.clone() } else { "Researcher".to_string() };
            let spec_sys = self.spec_for(&role).unwrap().system;
            let model_role = self.spec_for(&role).unwrap().model_role;
            let agent = self.agent_for(&role).unwrap();

            self.kernel.set_state(agent, AgentState::Running);
            emit(&format!("  ▸ {} → {}", role, task.title));

            if role.eq_ignore_ascii_case("Designer") {
                match self.run_designer(agent, &task.title, &task.detail) {
                    Ok((prompt, path)) => {
                        let k = self.memory.add_node("Image", &task.title, serde_json::json!({"prompt": prompt, "path": path.display().to_string()}));
                        self.memory.link(goal_id, k, "produced");
                        context.push_str(&format!("\n[{}] image saved: {}\n", role, path.display()));
                        emit(&format!("    ✓ image → {}", path.display()));
                    }
                    Err(e) => emit(&format!("    ✗ Designer failed: {e}")),
                }
                continue;
            }

            self.kernel.authorize(agent, Capability::ModelInference, &task.title)?;
            let prompt = format!(
                "GOAL: {goal}\nTASK: {}\nDETAIL: {}\n\nWork so far:\n{}\n\nDo your part.",
                task.title,
                task.detail,
                truncate(&context, 4000)
            );
            match self.router.chat(model_role, spec_sys, &prompt) {
                Ok(out) => {
                    self.kernel.record_result(agent, true, &task.title);
                    let k = self.memory.add_node("Knowledge", &task.title, serde_json::json!({"role": role, "text": out}));
                    self.memory.link(goal_id, k, "produced");
                    context.push_str(&format!("\n## {role}: {}\n{}\n", task.title, out));
                    emit(&format!("    ✓ {} done", role));
                }
                Err(e) => {
                    self.kernel.record_result(agent, false, &e.to_string());
                    emit(&format!("    ✗ {role} failed: {e}"));
                }
            }
        }

        // 4. Observer synthesizes the final answer.
        let observer = self.agent_for("Observer").ok_or("no Observer agent")?;
        let obs_sys = self.spec_for("Observer").unwrap().system;
        self.kernel.authorize(observer, Capability::ModelInference, "synthesize")?;
        self.kernel.set_state(observer, AgentState::Running);
        emit("  ▸ Observer is synthesizing the result…");
        let final_prompt = format!("GOAL: {goal}\n\nAgent outputs:\n{}\n\nWrite the final answer for the user.", truncate(&context, 8000));
        let answer = match self.router.chat(Role::Conversation, obs_sys, &final_prompt) {
            Ok(a) => {
                self.kernel.record_result(observer, true, "synthesized");
                a
            }
            Err(e) => {
                self.kernel.record_result(observer, false, &e.to_string());
                return Err(e);
            }
        };
        let ans_id = self.memory.add_node("Answer", "final answer", serde_json::json!({"text": answer}));
        self.memory.link(goal_id, ans_id, "answered_by");
        self.memory.save()?;
        Ok(answer)
    }

    fn run_designer(&mut self, agent: AgentId, title: &str, detail: &str) -> Result<(String, PathBuf)> {
        let sys = self.spec_for("Designer").unwrap().system;
        self.kernel.authorize(agent, Capability::ModelInference, "compose image prompt")?;
        let prompt = self
            .router
            .chat(Role::Conversation, sys, &format!("{title}\n{detail}"))
            .unwrap_or_default();
        let prompt = if prompt.trim().is_empty() { format!("{title}. {detail}") } else { prompt };

        self.kernel.authorize(agent, Capability::ImageGen, &prompt)?;
        let bytes = self.router.generate_image(&prompt)?;
        let path = self.next_gallery_path();
        std::fs::write(&path, bytes)?;
        self.kernel.record_result(agent, true, "image generated");
        Ok((prompt, path))
    }

    // ----------------------------------------------------------------------
    // Verb: design — generate an image directly via the Designer agent.
    // ----------------------------------------------------------------------

    pub fn design(&mut self, request: &str, emit: Progress) -> Result<PathBuf> {
        let agent = self.agent_for("Designer").ok_or("no Designer agent")?;
        emit(&format!("◆ DESIGN: {request}"));
        self.kernel.set_state(agent, AgentState::Running);
        emit("  ▸ Designer is composing a prompt…");
        let (prompt, path) = self.run_designer(agent, request, "")?;
        emit(&format!("  ▸ generating with image model…\n    prompt: {prompt}"));
        let id = self.memory.add_node("Image", request, serde_json::json!({"prompt": prompt, "path": path.display().to_string()}));
        let _ = id;
        self.memory.save()?;
        emit(&format!("  ✓ saved → {}", path.display()));
        Ok(path)
    }

    // ----------------------------------------------------------------------
    // Verb: see — the Eyes agent captures and interprets the desktop.
    // ----------------------------------------------------------------------

    pub fn see(&mut self, question: &str, emit: Progress) -> Result<String> {
        let eyes = self.agent_for("Eyes").ok_or("no Eyes agent")?;
        let sys = self.spec_for("Eyes").unwrap().system;
        emit(&format!("◆ SEE: {question}"));

        // Capability gate: the OS may only look at the screen when granted.
        self.kernel.authorize(eyes, Capability::ScreenCapture, "capture desktop to answer user")?;
        self.kernel.set_state(eyes, AgentState::Running);
        emit("  ▸ Eyes is capturing the desktop…");
        let frame = los_perception::capture_primary()?;
        emit(&format!("  ✓ captured {} ({}×{})", frame.monitor, frame.width, frame.height));

        // Keep a copy of what it saw for transparency.
        let snap = self.gallery_dir.join("last-capture.png");
        let _ = std::fs::write(&snap, &frame.png);

        self.kernel.authorize(eyes, Capability::ModelInference, "interpret screenshot")?;
        emit("  ▸ vision model is interpreting…");
        let answer = match self.router.vision(Role::Vision, sys, question, &[frame.png]) {
            Ok(a) => {
                self.kernel.record_result(eyes, true, "saw desktop");
                a
            }
            Err(e) => {
                self.kernel.record_result(eyes, false, &e.to_string());
                return Err(e);
            }
        };
        let id = self.memory.add_node("Observation", question, serde_json::json!({"monitor": frame.monitor, "answer": answer, "snapshot": snap.display().to_string()}));
        let _ = id;
        self.memory.save()?;
        Ok(answer)
    }

    fn next_gallery_path(&self) -> PathBuf {
        let n = std::fs::read_dir(&self.gallery_dir)
            .map(|rd| rd.filter(|e| e.as_ref().map(|e| e.path().extension().map(|x| x == "png").unwrap_or(false)).unwrap_or(false)).count())
            .unwrap_or(0);
        self.gallery_dir.join(format!("img-{:04}.png", n + 1))
    }

    pub fn data_dir(&self) -> &Path {
        &self.data_dir
    }
}

// ---- helpers ----------------------------------------------------------------

pub fn load_or_init_config(path: &Path) -> Result<FleetConfig> {
    if path.exists() {
        let bytes = std::fs::read(path)?;
        Ok(serde_json::from_slice(&bytes).unwrap_or_default())
    } else {
        let cfg = FleetConfig::default();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, serde_json::to_vec_pretty(&cfg)?)?;
        Ok(cfg)
    }
}

/// Extract a JSON array of plan items from a model response, tolerating prose
/// or code fences around it.
fn parse_plan(raw: &str) -> Vec<PlanItem> {
    let start = raw.find('[');
    let end = raw.rfind(']');
    if let (Some(s), Some(e)) = (start, end) {
        if e > s {
            if let Ok(items) = serde_json::from_str::<Vec<PlanItem>>(&raw[s..=e]) {
                return items;
            }
        }
    }
    Vec::new()
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("…{}", &s[s.len() - max..])
    }
}
