//! living — the Living Shell.
//!
//! You don't open applications. You express goals. This CLI is the entry point
//! to the LivingOS agent society.
//!
//!   living goal "build a snake game in python"   run the society on a goal
//!   living see  "what's on my screen?"            the Eyes agent looks
//!   living design "a neon koi fish, dark water"   the Designer makes an image
//!   living ps                                     list agents (the society)
//!   living models                                 show the fleet + what's pulled
//!   living mem [query]                            browse Living Memory
//!   living log [n]                                the transparent audit trail
//!   living doctor                                 check Ollama + models + image server
//!   living init                                   write default config & data dirs

use los_runtime::LivingOs;
use std::path::PathBuf;

fn root() -> PathBuf {
    std::env::var_os("LIVINGOS_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")))
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let cmd = args.first().map(|s| s.as_str()).unwrap_or("help");
    let rest = args.get(1..).map(|s| s.join(" ")).unwrap_or_default();

    let code = match cmd {
        "goal" | "g" => cmd_goal(&rest),
        "see" | "s" => cmd_see(&rest),
        "design" | "d" => cmd_design(&rest),
        "ps" | "agents" => cmd_ps(),
        "models" | "fleet" => cmd_models(),
        "mem" | "memory" => cmd_mem(&rest),
        "log" | "audit" => cmd_log(&rest),
        "doctor" | "health" => cmd_doctor(),
        "init" => cmd_init(),
        "help" | "-h" | "--help" => {
            help();
            0
        }
        other => {
            eprintln!("unknown command: {other}\n");
            help();
            2
        }
    };
    std::process::exit(code);
}

fn boot() -> LivingOs {
    match LivingOs::boot(root()) {
        Ok(os) => os,
        Err(e) => {
            eprintln!("failed to boot LivingOS: {e}");
            std::process::exit(1);
        }
    }
}

fn emitter() -> impl FnMut(&str) {
    |line: &str| println!("{line}")
}

fn cmd_goal(goal: &str) -> i32 {
    if goal.is_empty() {
        eprintln!("usage: living goal \"<what you want>\"");
        return 2;
    }
    let mut os = boot();
    let mut emit = emitter();
    match os.run_goal(goal, &mut emit) {
        Ok(answer) => {
            println!("\n──────── RESULT ────────\n{answer}");
            0
        }
        Err(e) => {
            eprintln!("\n✗ {e}\n  (run `living doctor` to check the model fleet)");
            1
        }
    }
}

fn cmd_see(question: &str) -> i32 {
    let q = if question.is_empty() { "Describe what is on screen." } else { question };
    let mut os = boot();
    let mut emit = emitter();
    match os.see(q, &mut emit) {
        Ok(answer) => {
            println!("\n──────── EYES ────────\n{answer}");
            0
        }
        Err(e) => {
            eprintln!("\n✗ {e}\n  (run `living doctor`; vision needs the `vision` model pulled in Ollama)");
            1
        }
    }
}

fn cmd_design(request: &str) -> i32 {
    if request.is_empty() {
        eprintln!("usage: living design \"<what to draw>\"");
        return 2;
    }
    let mut os = boot();
    let mut emit = emitter();
    match os.design(request, &mut emit) {
        Ok(path) => {
            println!("\n──────── IMAGE ────────\n{}", path.display());
            0
        }
        Err(e) => {
            eprintln!("\n✗ {e}\n  (image generation needs a local SD server — see `living doctor`)");
            1
        }
    }
}

fn cmd_ps() -> i32 {
    let os = boot();
    println!("{:<3} {:<10} {:<9} {:<5} {:<8} {}", "ID", "ROLE", "STATE", "REP", "DONE/FAIL", "CAPABILITIES");
    println!("{}", "─".repeat(78));
    for a in os.kernel.agents() {
        let caps: Vec<String> = a.caps.iter().map(|c| c.label().to_string()).collect();
        println!(
            "{:<3} {:<10} {:<9} {:<5.1} {:<8} {}",
            a.id,
            a.role,
            format!("{:?}", a.state),
            a.reputation,
            format!("{}/{}", a.tasks_done, a.tasks_failed),
            caps.join(", ")
        );
    }
    0
}

fn cmd_models() -> i32 {
    let os = boot();
    let cfg = os.router.config();
    let installed = os.router.installed_models().unwrap_or_default();
    let is_ready = |tag: &str| installed.iter().any(|m| m == tag || m.starts_with(&format!("{}:", tag.split(':').next().unwrap_or(tag))));

    println!("Intelligence Router — local fleet  (ollama: {})", cfg.ollama_url);
    println!("{}", "─".repeat(60));
    let mut roles: Vec<(&String, &String)> = cfg.models.iter().collect();
    roles.sort_by(|a, b| a.0.cmp(b.0));
    for (role, model) in roles {
        let mark = if is_ready(model) { "✓" } else { "·" };
        println!("  {mark} {:<14} → {}", role, model);
    }
    println!("\nImage generation (separate local server)");
    println!("  · image_gen      → {}  @ {}", cfg.image_model, cfg.image_url);
    if installed.is_empty() {
        println!("\n(no models reported by Ollama — is it installed and running?)");
    }
    0
}

fn cmd_mem(query: &str) -> i32 {
    let os = boot();
    let (n, e) = os.memory.stats();
    println!("Living Memory — {n} nodes, {e} edges");
    println!("{}", "─".repeat(60));
    let nodes = if query.is_empty() { os.memory.recent(20) } else { os.memory.search(query) };
    if nodes.is_empty() {
        println!("(nothing yet — try `living goal \"...\"`)");
    }
    for node in nodes {
        let label = truncate(&node.label, 60);
        println!("  #{:<4} [{:<11}] {}", node.id, node.kind, label);
    }
    0
}

fn cmd_log(arg: &str) -> i32 {
    let n: usize = arg.trim().parse().unwrap_or(30);
    let os = boot();
    let entries = os.kernel.audit_tail(n);
    if entries.is_empty() {
        println!("(audit trail empty — actions are logged as agents work)");
        return 0;
    }
    println!("Transparent audit trail (last {})", entries.len());
    println!("{}", "─".repeat(70));
    for e in entries {
        let mark = if e.allowed { "✓" } else { "✗ DENIED" };
        println!("  {:<8} {:<9} {:<22} {}", mark, e.agent, e.action, truncate(&e.detail, 34));
    }
    0
}

fn cmd_doctor() -> i32 {
    let os = boot();
    let cfg = os.router.config();
    println!("LivingOS doctor");
    println!("{}", "─".repeat(60));

    match os.router.installed_models() {
        Ok(installed) => {
            println!("✓ Ollama reachable at {} ({} models)", cfg.ollama_url, installed.len());
            let base = |t: &str| t.split(':').next().unwrap_or(t).to_string();
            let ready = |tag: &str| installed.iter().any(|m| m == tag || base(m) == base(tag));
            let mut missing = Vec::new();
            let mut roles: Vec<(&String, &String)> = cfg.models.iter().collect();
            roles.sort_by(|a, b| a.0.cmp(b.0));
            for (role, model) in roles {
                if ready(model) {
                    println!("  ✓ {:<14} {}", role, model);
                } else {
                    println!("  ✗ {:<14} {}  (missing)", role, model);
                    missing.push(model.clone());
                }
            }
            if !missing.is_empty() {
                println!("\nPull the missing specialists:");
                for m in missing {
                    println!("    ollama pull {m}");
                }
            }
        }
        Err(e) => {
            println!("✗ Ollama not reachable: {e}");
            println!("\n  Install Ollama (https://ollama.com/download), start it, then:");
            for (_role, model) in &cfg.models {
                println!("    ollama pull {model}");
            }
        }
    }

    println!("\nImage server (Designer): {} ({})", cfg.image_url, cfg.image_model);
    println!("  Run a local Stable-Diffusion server with an A1111-compatible");
    println!("  /sdapi/v1/txt2img endpoint. Recommended checkpoint: {} (or FLUX.2 Klein).", cfg.image_model);
    println!("\nData dir: {}", os.data_dir().display());
    0
}

fn cmd_init() -> i32 {
    let os = boot(); // boot() writes default config + data dirs as a side effect
    println!("✓ LivingOS initialized");
    println!("  config: {}", root().join("config").join("fleet.json").display());
    println!("  data:   {}", os.data_dir().display());
    println!("\nNext: `living doctor` to set up the local model fleet.");
    0
}

fn help() {
    println!(
        "LivingOS — an AI-native OS where agents are first-class, kernel-managed resources.\n\n\
         USAGE\n\
         \x20 living goal   \"<goal>\"     run the agent society on a goal\n\
         \x20 living see    \"<question>\" the Eyes agent looks at your desktop\n\
         \x20 living design \"<prompt>\"   the Designer generates a local image\n\
         \x20 living ps                  list agents (the society)\n\
         \x20 living models              show the local model fleet\n\
         \x20 living mem    [query]      browse Living Memory\n\
         \x20 living log    [n]          transparent audit trail\n\
         \x20 living doctor              check Ollama, models, image server\n\
         \x20 living init                write default config & data dirs\n"
    );
}

fn truncate(s: &str, max: usize) -> String {
    let s = s.replace('\n', " ");
    if s.chars().count() <= max {
        s
    } else {
        let cut: String = s.chars().take(max.saturating_sub(1)).collect();
        format!("{cut}…")
    }
}
