//! The Agent Society, defined at the kernel level: which agents exist and the
//! capabilities each is born with. Note that only the Eyes agent may capture
//! the screen and only the Designer may generate images — the kernel will deny
//! anyone else who tries.

use crate::sched::Capability;
use crate::sched::Capability::*;
use alloc::vec;
use alloc::vec::Vec;

pub struct Spec {
    pub name: &'static str,
    pub blurb: &'static str,
    pub caps: Vec<Capability>,
    pub priority: u8,
}

pub fn society() -> Vec<Spec> {
    vec![
        Spec { name: "Planner", blurb: "decomposes a goal into a plan", caps: vec![ModelInference, Memory], priority: 9 },
        Spec { name: "Architect", blurb: "designs the approach", caps: vec![ModelInference, Memory], priority: 7 },
        Spec { name: "Researcher", blurb: "gathers knowledge", caps: vec![ModelInference, Memory, Internet], priority: 6 },
        Spec { name: "Coder", blurb: "writes the implementation", caps: vec![ModelInference, WriteFiles, Compiler, Git], priority: 6 },
        Spec { name: "Tester", blurb: "validates the work", caps: vec![ModelInference, Terminal], priority: 5 },
        Spec { name: "Security", blurb: "reviews for risk", caps: vec![ModelInference], priority: 8 },
        Spec { name: "Designer", blurb: "generates images", caps: vec![ModelInference, ImageGen], priority: 5 },
        Spec { name: "Eyes", blurb: "sees the desktop", caps: vec![ScreenCapture, ModelInference], priority: 7 },
        Spec { name: "Observer", blurb: "synthesizes the result", caps: vec![ModelInference, Memory], priority: 4 },
    ]
}
