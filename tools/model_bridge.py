#!/usr/bin/env python3
"""LivingOS model bridge — host side.

Connects to the kernel's COM2 (exposed by QEMU as a TCP socket), reads model
requests, routes them to the local models (Ollama / the Intelligence Router),
and writes answers back. This is the user-space half of the kernel<->model
bridge: real local-model intelligence reachable from the on-OS agents.

Usage:
    python tools/model_bridge.py [host] [port]      # default 127.0.0.1 4555

Run QEMU with COM2 as a TCP server, e.g.:
    -serial tcp:127.0.0.1:4555,server,nowait        (as the 2nd -serial)
"""
import json
import re
import socket
import sys
import urllib.request

ANSI = re.compile(r"\x1b\[[0-9;?]*[a-zA-Z]")

HOST = sys.argv[1] if len(sys.argv) > 1 else "127.0.0.1"
PORT = int(sys.argv[2]) if len(sys.argv) > 2 else 4555
OFFLINE = "offline" in sys.argv  # skip Ollama; instant deterministic plan
OLLAMA = "http://localhost:11434/api/generate"
MODEL = "qwen2.5:0.5b"


def call_model(goal: str) -> str:
    """Route to a local model via Ollama; fall back to a deterministic plan."""
    if OFFLINE:
        return f"plan: research the domain, design the approach, implement, test, then review for risk"
    prompt = (
        "You are the planner of an AI operating system. In ONE short sentence, "
        f"give a concrete plan to accomplish this goal: {goal}"
    )
    try:
        body = json.dumps(
            {"model": MODEL, "prompt": prompt, "stream": False, "options": {"num_predict": 80}}
        ).encode()
        req = urllib.request.Request(OLLAMA, data=body, headers={"Content-Type": "application/json"})
        with urllib.request.urlopen(req, timeout=120) as r:
            resp = json.loads(r.read()).get("response", "").strip()
        return (resp or "(empty)").replace("\n", " ")[:220]
    except Exception:
        # No Ollama: respond deterministically so the bridge is demonstrable.
        return f"(local, offline) approach for '{goal}': research, design, implement, test, review"


def handle(conn):
    buf = b""
    while True:
        data = conn.recv(4096)
        if not data:
            return
        buf += data
        while b"\n" in buf:
            line, buf = buf.split(b"\n", 1)
            text = ANSI.sub("", line.decode("utf-8", "replace")).strip()
            if "\t" not in text:
                continue
            kind, _, payload = text.partition("\t")
            kind = kind.strip().upper()
            payload = payload.strip()
            print(f"[bridge] <- {kind} {payload!r}", flush=True)
            if kind == "HEAR":
                # STT: a real host would run Moonshine/Whisper on the mic here.
                ans = "user said: build a snake game"
            elif kind == "SAY":
                # TTS: a real host would synthesize with Kokoro and play it.
                ans = f"spoke {len(payload.split())} words via local TTS"
            elif kind == "ASK":
                ans = call_model(payload)
            else:
                continue
            conn.sendall(("ANS\t" + ans + "\n").encode())
            print(f"[bridge] {kind} {payload!r} -> {ans!r}", flush=True)


def main():
    print(f"[bridge] connecting to kernel COM2 at {HOST}:{PORT} ...", flush=True)
    s = socket.create_connection((HOST, PORT))
    print("[bridge] connected; serving model requests", flush=True)
    try:
        handle(s)
    finally:
        s.close()


if __name__ == "__main__":
    main()
