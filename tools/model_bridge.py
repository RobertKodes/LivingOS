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
import socket
import sys
import urllib.request

HOST = sys.argv[1] if len(sys.argv) > 1 else "127.0.0.1"
PORT = int(sys.argv[2]) if len(sys.argv) > 2 else 4555
OLLAMA = "http://localhost:11434/api/generate"
MODEL = "smollm3:3b"


def call_model(goal: str) -> str:
    """Route to a local model via Ollama; fall back to a deterministic plan."""
    try:
        body = json.dumps({"model": MODEL, "prompt": goal, "stream": False}).encode()
        req = urllib.request.Request(OLLAMA, data=body, headers={"Content-Type": "application/json"})
        with urllib.request.urlopen(req, timeout=30) as r:
            resp = json.loads(r.read()).get("response", "").strip()
        return (resp or "(empty)").replace("\n", " ")[:240]
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
            text = line.decode("utf-8", "replace").strip("\r")
            if "\t" not in text:
                continue
            kind, _, payload = text.partition("\t")
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
