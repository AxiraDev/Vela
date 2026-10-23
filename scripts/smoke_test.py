#!/usr/bin/env python3
"""Integration test using a real, separately installed Wine engine.

Run in an explicitly selected test data directory, not an existing library.
The test leaves its prefix and logs for inspection and stops that prefix only.
"""
import argparse
import json
import os
from pathlib import Path
import subprocess
import time


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--engine", required=True, type=Path)
    parser.add_argument("--home", required=True, type=Path)
    parser.add_argument("--core", type=Path, default=Path(__file__).resolve().parents[1] / "Vela.app/Contents/MacOS/vela-core")
    args = parser.parse_args()
    home = args.home.resolve()
    if (home / "state.json").exists():
        parser.error("Use a new test data directory; existing libraries are not modified.")
    env = os.environ.copy()
    env["VELA_HOME"] = str(home)

    def call(op, **fields):
        request = {"op": op, **fields}
        process = subprocess.run([str(args.core.resolve())], input=json.dumps(request),
                                 text=True, capture_output=True, env=env, timeout=145)
        try:
            reply = json.loads(process.stdout)
        except ValueError as exc:
            raise RuntimeError(f"Invalid core response: {process.stdout!r} {process.stderr!r}") from exc
        if not reply["ok"]:
            raise RuntimeError(reply["error"])
        return reply["data"]

    engine = call("add_engine", path=str(args.engine.resolve()))
    print(f"PASS engine: {engine['version']}", flush=True)
    bottle = call("create", name="Integration test", engine_id=engine["id"])
    bid = bottle["id"]
    prefix = home / "prefixes" / bid
    print(f"PASS wineboot: {prefix}", flush=True)
    try:
        call("save_settings", id=bid, name="Integration test", environment={"APP_TEST_TOKEN": "VELA_ENV_OK"})
        for windows_dir, marker in [("system32", "VELA_WIN64_OK"), ("syswow64", "VELA_WOW64_OK")]:
            executable = prefix / "drive_c/windows" / windows_dir / "cmd.exe"
            if not executable.is_file():
                raise RuntimeError(f"Missing Windows command processor: {executable}")
            shortcut = call("add_shortcut", id=bid, name=marker, path=str(executable),
                            arguments=["/d", "/c", f"echo {marker} %APP_TEST_TOKEN%"],
                            directory=str(prefix / "drive_c"))
            call("run_shortcut", id=bid, shortcut_id=shortcut["id"])
            deadline = time.monotonic() + 20
            while time.monotonic() < deadline:
                log = call("read_log", id=bid)
                if f"{marker} VELA_ENV_OK" in log["text"]:
                    print(f"PASS {windows_dir}: Windows EXE, shortcut, arguments, cwd, environment, log", flush=True)
                    break
                time.sleep(0.2)
            else:
                raise RuntimeError(f"Windows program did not emit the expected marker.\n{log['text']}")
    finally:
        call("stop", id=bid)
        print("PASS scoped wineserver stop", flush=True)
    print("All real-Wine integration checks passed.", flush=True)


if __name__ == "__main__":
    main()
