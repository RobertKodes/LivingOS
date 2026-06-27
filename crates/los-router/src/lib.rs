//! los-router — the Intelligence Router.
//!
//! LivingOS does not use one model. It uses a *fleet* of small local
//! specialists, and the router maps each agent **role** to the best specialist
//! for that role, then dispatches the work to a local backend.
//!
//!   * Text / vision / embeddings  -> Ollama  (HTTP, localhost:11434)
//!   * Image generation            -> a local SD server (ComfyUI / A1111-style)
//!
//! Nothing here is cloud. Everything runs on the user's machine. The role ->
//! model mapping lives in config and is fully swappable ("model agnostic /
//! composable" from the PRD).

use base64::Engine as _;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;

pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

/// The capabilities the router can route to. Each maps to one specialist model.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    Conversation,
    Planning,
    Coding,
    ToolCalling,
    Vision,
    Ocr,
    Embedding,
    Stt,
    Tts,
    ImageGen,
}

impl Role {
    pub fn key(&self) -> &'static str {
        match self {
            Role::Conversation => "conversation",
            Role::Planning => "planning",
            Role::Coding => "coding",
            Role::ToolCalling => "tool_calling",
            Role::Vision => "vision",
            Role::Ocr => "ocr",
            Role::Embedding => "embedding",
            Role::Stt => "stt",
            Role::Tts => "tts",
            Role::ImageGen => "image_gen",
        }
    }
    pub fn all() -> &'static [Role] {
        &[
            Role::Conversation,
            Role::Planning,
            Role::Coding,
            Role::ToolCalling,
            Role::Vision,
            Role::Ocr,
            Role::Embedding,
            Role::Stt,
            Role::Tts,
            Role::ImageGen,
        ]
    }
}

/// Router configuration — the fleet. Serialized to config/fleet.json.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FleetConfig {
    pub ollama_url: String,
    /// role key -> ollama model tag
    pub models: HashMap<String, String>,
    /// local Stable-Diffusion-style server base url (ComfyUI / A1111)
    pub image_url: String,
    /// recommended checkpoint loaded in that server, e.g. "z-image-turbo"
    pub image_model: String,
    pub image_steps: u32,
    pub image_width: u32,
    pub image_height: u32,
    #[serde(default = "default_timeout")]
    pub timeout_secs: u64,
}

fn default_timeout() -> u64 {
    600
}

impl Default for FleetConfig {
    /// The newest small local specialists, mid-2026. Every tag is a local model;
    /// edit freely — the router is model-agnostic.
    fn default() -> Self {
        let mut models = HashMap::new();
        models.insert("conversation".into(), "smollm3:3b".into());
        models.insert("planning".into(), "gemma4:4b".into());
        models.insert("coding".into(), "qwen3.5:4b".into());
        models.insert("tool_calling".into(), "qwen3.5:4b".into());
        models.insert("vision".into(), "qwen3-vl:2b".into());
        models.insert("ocr".into(), "glm-ocr:0.9b".into());
        models.insert("embedding".into(), "embeddinggemma".into());
        models.insert("stt".into(), "moonshine".into());
        models.insert("tts".into(), "kokoro".into());
        FleetConfig {
            ollama_url: "http://localhost:11434".into(),
            models,
            image_url: "http://localhost:7860".into(),
            image_model: "z-image-turbo".into(),
            image_steps: 8,
            image_width: 1024,
            image_height: 1024,
            timeout_secs: default_timeout(),
        }
    }
}

pub struct Router {
    cfg: FleetConfig,
    agent: ureq::Agent,
}

impl Router {
    pub fn new(cfg: FleetConfig) -> Self {
        let agent = ureq::AgentBuilder::new()
            .timeout(Duration::from_secs(cfg.timeout_secs))
            .build();
        Router { cfg, agent }
    }

    pub fn config(&self) -> &FleetConfig {
        &self.cfg
    }

    pub fn model_for(&self, role: Role) -> String {
        self.cfg
            .models
            .get(role.key())
            .cloned()
            .unwrap_or_else(|| "qwen3.5:4b".into())
    }

    // ---- text -------------------------------------------------------------

    /// Route a chat completion to the specialist for `role`.
    pub fn chat(&self, role: Role, system: &str, user: &str) -> Result<String> {
        let model = self.model_for(role);
        let body = serde_json::json!({
            "model": model,
            "stream": false,
            "messages": [
                {"role": "system", "content": system},
                {"role": "user", "content": user}
            ]
        });
        let v: serde_json::Value = self
            .agent
            .post(&format!("{}/api/chat", self.cfg.ollama_url))
            .send_json(body)
            .map_err(|e| format!("ollama chat ({model}) failed: {e}"))?
            .into_json()?;
        Ok(v["message"]["content"].as_str().unwrap_or("").trim().to_string())
    }

    // ---- vision -----------------------------------------------------------

    /// Route an image-grounded question to the vision specialist. `images` are
    /// raw PNG/JPEG bytes; they are base64-encoded for Ollama.
    pub fn vision(&self, role: Role, system: &str, user: &str, images: &[Vec<u8>]) -> Result<String> {
        let model = self.model_for(role);
        let b64: Vec<String> = images
            .iter()
            .map(|b| base64::engine::general_purpose::STANDARD.encode(b))
            .collect();
        let body = serde_json::json!({
            "model": model,
            "stream": false,
            "messages": [
                {"role": "system", "content": system},
                {"role": "user", "content": user, "images": b64}
            ]
        });
        let v: serde_json::Value = self
            .agent
            .post(&format!("{}/api/chat", self.cfg.ollama_url))
            .send_json(body)
            .map_err(|e| format!("ollama vision ({model}) failed: {e}"))?
            .into_json()?;
        Ok(v["message"]["content"].as_str().unwrap_or("").trim().to_string())
    }

    // ---- embeddings -------------------------------------------------------

    pub fn embed(&self, text: &str) -> Result<Vec<f32>> {
        let model = self.model_for(Role::Embedding);
        let body = serde_json::json!({ "model": model, "prompt": text });
        let v: serde_json::Value = self
            .agent
            .post(&format!("{}/api/embeddings", self.cfg.ollama_url))
            .send_json(body)
            .map_err(|e| format!("ollama embed ({model}) failed: {e}"))?
            .into_json()?;
        let arr = v["embedding"].as_array().cloned().unwrap_or_default();
        Ok(arr.iter().filter_map(|x| x.as_f64().map(|f| f as f32)).collect())
    }

    // ---- image generation -------------------------------------------------

    /// Generate an image locally via an Automatic1111-compatible txt2img
    /// endpoint. Returns PNG bytes. (ComfyUI users can point image_url at an
    /// A1111-compat shim.)
    pub fn generate_image(&self, prompt: &str) -> Result<Vec<u8>> {
        let body = serde_json::json!({
            "prompt": prompt,
            "steps": self.cfg.image_steps,
            "width": self.cfg.image_width,
            "height": self.cfg.image_height,
        });
        let v: serde_json::Value = self
            .agent
            .post(&format!("{}/sdapi/v1/txt2img", self.cfg.image_url))
            .send_json(body)
            .map_err(|e| format!("image server ({}) failed: {e}", self.cfg.image_model))?
            .into_json()?;
        let b64 = v["images"][0]
            .as_str()
            .ok_or("image server returned no image")?;
        // A1111 sometimes prefixes a data URL; strip it.
        let b64 = b64.split(',').last().unwrap_or(b64);
        let bytes = base64::engine::general_purpose::STANDARD.decode(b64)?;
        Ok(bytes)
    }

    // ---- health -----------------------------------------------------------

    /// Models currently installed in Ollama (from /api/tags).
    pub fn installed_models(&self) -> Result<Vec<String>> {
        let v: serde_json::Value = self
            .agent
            .get(&format!("{}/api/tags", self.cfg.ollama_url))
            .call()
            .map_err(|e| format!("ollama not reachable at {}: {e}", self.cfg.ollama_url))?
            .into_json()?;
        let models = v["models"]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|m| m["name"].as_str().map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default();
        Ok(models)
    }
}
