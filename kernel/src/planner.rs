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

fn has_any(g: &str, words: &[&str]) -> bool {
    words.iter().any(|w| g.contains(w))
}

/// Decompose a goal into an ordered, role-assigned task list.
pub fn plan(goal: &str) -> Vec<PlanTask> {
    let g = goal.to_lowercase();
    let mut tasks: Vec<PlanTask> = Vec::new();

    // Every goal starts with understanding and design.
    tasks.push(PlanTask { title: "research the problem and constraints".to_string(), role: "Researcher", priority: 6 });
    tasks.push(PlanTask { title: "design the approach".to_string(), role: "Architect", priority: 7 });

    let buildish = has_any(&g, &["build", "make", "create", "code", "implement", "program", "app", "game", "site", "website", "api", "tool", "script"]);
    if buildish {
        tasks.push(PlanTask { title: "implement the core".to_string(), role: "Coder", priority: 6 });
        tasks.push(PlanTask { title: "validate it works".to_string(), role: "Tester", priority: 5 });
    }

    if has_any(&g, &["secure", "security", "auth", "login", "password", "encrypt", "safe", "malware", "vuln"]) {
        tasks.push(PlanTask { title: "review the security model".to_string(), role: "Security", priority: 8 });
    }

    if has_any(&g, &["image", "art", "logo", "draw", "picture", "design", "poster", "icon", "ui", "visual"]) {
        tasks.push(PlanTask { title: "generate the visual asset".to_string(), role: "Designer", priority: 5 });
    }

    if has_any(&g, &["screen", "desktop", "see ", "look", "what's on", "whats on", "window"]) {
        tasks.push(PlanTask { title: "observe the screen".to_string(), role: "Eyes", priority: 7 });
    }

    // Always end with a security sanity check and a synthesis.
    if !tasks.iter().any(|t| t.role == "Security") {
        tasks.push(PlanTask { title: "sanity-check for risk".to_string(), role: "Security", priority: 8 });
    }

    tasks
}
