//! The on-device planner. With no language model available on bare metal, the
//! kernel decomposes a goal into a task plan using a fast keyword heuristic.
//! This is deliberately a stand-in for the user-space Intelligence Router (which
//! routes to local models); the *shape* of the pipeline — goal in, role-assigned
//! tasks out, scheduled by the kernel — is identical, so swapping the heuristic
//! for real model output later changes nothing downstream.

use alloc::string::{String, ToString};
use alloc::vec::Vec;

pub struct PlanTask {
    pub title: String,
    pub role: &'static str,
    pub priority: u8,
}

/// Whole-word / prefix keyword match. Tokenises the goal and tests each token
/// against each stem with `starts_with`, so short stems like "ui" can't match
/// *inside* unrelated words (e.g. "b-ui-ld").
fn has_word(goal: &str, stems: &[&str]) -> bool {
    goal.split(|c: char| !c.is_alphanumeric())
        .filter(|t| !t.is_empty())
        .any(|tok| stems.iter().any(|s| tok.starts_with(s)))
}

/// Decompose a goal into an ordered, role-assigned task list.
pub fn plan(goal: &str) -> Vec<PlanTask> {
    let g = goal.to_lowercase();
    let mut tasks: Vec<PlanTask> = Vec::new();

    // Every goal starts with understanding and design.
    tasks.push(PlanTask { title: "research the problem and constraints".to_string(), role: "Researcher", priority: 6 });
    tasks.push(PlanTask { title: "design the approach".to_string(), role: "Architect", priority: 7 });

    if has_word(&g, &["build", "make", "creat", "code", "implement", "program", "app", "game", "site", "web", "api", "tool", "script", "develop"]) {
        tasks.push(PlanTask { title: "implement the core".to_string(), role: "Coder", priority: 6 });
        tasks.push(PlanTask { title: "validate it works".to_string(), role: "Tester", priority: 5 });
    }

    let secure = has_word(&g, &["secur", "auth", "login", "password", "encrypt", "safe", "malware", "vuln", "threat"]);
    if secure {
        tasks.push(PlanTask { title: "review the security model".to_string(), role: "Security", priority: 8 });
    }

    if has_word(&g, &["image", "art", "logo", "draw", "pictur", "design", "poster", "icon", "visual", "graphic", "ui"]) {
        tasks.push(PlanTask { title: "generate the visual asset".to_string(), role: "Designer", priority: 5 });
    }

    if has_word(&g, &["screen", "desktop", "window", "see", "look", "monitor", "display"]) {
        tasks.push(PlanTask { title: "observe the screen".to_string(), role: "Eyes", priority: 7 });
    }

    // Always end with a security sanity check if one wasn't already added.
    if !secure {
        tasks.push(PlanTask { title: "sanity-check for risk".to_string(), role: "Security", priority: 8 });
    }

    tasks
}
