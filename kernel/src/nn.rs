//! On-metal neural-network inference.
//!
//! A real (tiny) char-level MLP language model runs *in the kernel*: embeddings
//! → linear → tanh → linear → argmax, with weights trained offline and embedded
//! (`nn_weights.rs`, from `tools/train_nn.py`). This is genuine on-device
//! inference — actual floating-point tensor math on bare metal, no host, no
//! model server. It is deliberately tiny; the small *local* specialist models
//! from the PRD run in user space and reach the kernel via the model bridge.

use crate::nn_weights as w;
use alloc::string::String;
use alloc::vec::Vec;

/// tanh without libm: a clamped Padé approximation, accurate enough for argmax.
fn tanh_approx(x: f32) -> f32 {
    if x > 4.0 {
        return 1.0;
    }
    if x < -4.0 {
        return -1.0;
    }
    let x2 = x * x;
    x * (27.0 + x2) / (27.0 + 9.0 * x2)
}

fn vocab() -> Vec<char> {
    w::VOCAB.chars().collect()
}

fn space_idx(chars: &[char]) -> usize {
    chars.iter().position(|&c| c == ' ').unwrap_or(0)
}

/// One forward pass over a context window; returns the argmax next-token index.
fn forward(ctx: &[usize; w::C]) -> usize {
    let mut emb = [0f32; w::C * w::D];
    for (j, &ci) in ctx.iter().enumerate() {
        for d in 0..w::D {
            emb[j * w::D + d] = w::E[ci * w::D + d];
        }
    }
    let mut h = [0f32; w::H];
    for (k, hk) in h.iter_mut().enumerate() {
        let mut s = w::B1[k];
        for (i, &ei) in emb.iter().enumerate() {
            s += ei * w::W1[i * w::H + k];
        }
        *hk = tanh_approx(s);
    }
    let mut best = 0usize;
    let mut best_v = f32::MIN;
    for v in 0..w::V {
        let mut s = w::B2[v];
        for (k, &hk) in h.iter().enumerate() {
            s += hk * w::W2[k * w::V + v];
        }
        if s > best_v {
            best_v = s;
            best = v;
        }
    }
    best
}

/// Greedy text generation from a seed. Real inference, deterministic.
pub fn generate(seed: &str, n: usize) -> String {
    let chars = vocab();
    let sp = space_idx(&chars);
    let mut ctx = [sp; w::C];

    // Prime the context from the seed's trailing characters.
    let seed_lower: String = seed.to_lowercase();
    for c in seed_lower.chars() {
        let idx = chars.iter().position(|&v| v == c).unwrap_or(sp);
        for j in 0..w::C - 1 {
            ctx[j] = ctx[j + 1];
        }
        ctx[w::C - 1] = idx;
    }

    let mut out = String::from(seed_lower.as_str());
    for _ in 0..n {
        let nx = forward(&ctx);
        out.push(chars[nx]);
        for j in 0..w::C - 1 {
            ctx[j] = ctx[j + 1];
        }
        ctx[w::C - 1] = nx;
    }
    out
}

/// Model size, for `sys`/`about`.
pub fn params() -> usize {
    w::E.len() + w::W1.len() + w::B1.len() + w::W2.len() + w::B2.len()
}
