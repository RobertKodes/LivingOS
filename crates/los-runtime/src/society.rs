//! The Agent Society — the cast of specialist agents and the capabilities and
//! model-role each one is granted at spawn time.

use los_kernel::Capability;
use los_router::Role;

pub struct AgentSpec {
    /// Stable role name, also used as the kernel role + scheduler routing key.
    pub role: &'static str,
    pub blurb: &'static str,
    pub system: &'static str,
    /// Which model specialist this agent thinks with.
    pub model_role: Role,
    pub caps: &'static [Capability],
    pub priority: u8,
}

use Capability::*;

/// The full society. Each agent is a first-class kernel resource with a scoped
/// capability set — note the Eyes agent is the only one that may capture the
/// screen, and the Designer the only one that may generate images.
pub fn society() -> Vec<AgentSpec> {
    vec![
        AgentSpec {
            role: "Planner",
            blurb: "Decomposes a goal into a task plan",
            system: "You are the Planner of an AI operating system. Given a user GOAL, break it into \
                     a short ordered list of concrete tasks. Respond with ONLY a JSON array, no prose. \
                     Each item: {\"title\": string, \"role\": one of \
                     [\"Researcher\",\"Architect\",\"Coder\",\"Tester\",\"Security\",\"Designer\"], \
                     \"detail\": string}. Keep it to 3-6 tasks.",
            model_role: Role::Planning,
            caps: &[ModelInference, Memory],
            priority: 9,
        },
        AgentSpec {
            role: "Architect",
            blurb: "Designs the structure and approach",
            system: "You are the Architect. Design a clear, pragmatic technical approach for the task. \
                     Be concrete about structure, components, and trade-offs. Keep it tight.",
            model_role: Role::Planning,
            caps: &[ModelInference, Memory],
            priority: 7,
        },
        AgentSpec {
            role: "Researcher",
            blurb: "Gathers knowledge and context",
            system: "You are the Researcher. Provide the key facts, options, and considerations needed \
                     for the task, drawing on what you know. Be specific and cite reasoning.",
            model_role: Role::Conversation,
            caps: &[ModelInference, Memory, Internet],
            priority: 6,
        },
        AgentSpec {
            role: "Coder",
            blurb: "Writes the implementation",
            system: "You are the Coder. Produce correct, idiomatic code for the task. Explain briefly, \
                     then give the code in fenced blocks. Prefer minimal, working solutions.",
            model_role: Role::Coding,
            caps: &[ModelInference, WriteFiles, Compiler, Git],
            priority: 6,
        },
        AgentSpec {
            role: "Tester",
            blurb: "Validates the work",
            system: "You are the Tester. Given the task and prior work, describe how to verify it and \
                     point out the most likely failure modes and edge cases.",
            model_role: Role::Coding,
            caps: &[ModelInference, Terminal],
            priority: 5,
        },
        AgentSpec {
            role: "Security",
            blurb: "Reviews for risk",
            system: "You are the Security reviewer. Identify concrete risks in the proposed work \
                     (capabilities, data, untrusted input) and give specific mitigations. Be terse.",
            model_role: Role::Planning,
            caps: &[ModelInference],
            priority: 8,
        },
        AgentSpec {
            role: "Designer",
            blurb: "Generates images and visual assets",
            system: "You are the Designer. Turn the request into a single vivid, concrete image-\
                     generation prompt (subject, style, composition, lighting). Respond with ONLY the \
                     prompt text, one line, no preamble.",
            model_role: Role::Conversation,
            caps: &[ModelInference, ImageGen],
            priority: 5,
        },
        AgentSpec {
            role: "Eyes",
            blurb: "Sees the desktop via the vision model",
            system: "You are the Eyes of the operating system. You are shown a screenshot of the user's \
                     desktop. Answer the user's question about what is on screen precisely and concisely. \
                     Describe windows, apps, text, and state you can actually see.",
            model_role: Role::Vision,
            caps: &[ScreenCapture, ModelInference],
            priority: 7,
        },
        AgentSpec {
            role: "Observer",
            blurb: "Summarizes and documents outcomes",
            system: "You are the Observer. Synthesize the work of the other agents into a clear, useful \
                     final answer for the user. Be direct; lead with the result.",
            model_role: Role::Conversation,
            caps: &[ModelInference, Memory],
            priority: 4,
        },
    ]
}
