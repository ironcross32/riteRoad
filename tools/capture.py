"""Attach Frida to RODE Central and log its HID traffic with the dongle.

Usage (from the repo root):
    .venv\\Scripts\\python.exe tools\\capture.py            # attach to running app
    .venv\\Scripts\\python.exe tools\\capture.py --spawn    # launch app (catches startup)

Log lines go to capture\\session-<timestamp>.log (and stdout).
Add a marker line from another shell with:
    .venv\\Scripts\\python.exe tools\\capture.py --mark "TX1 gain 0 -> +3"
"""

import argparse
import datetime as dt
import pathlib
import sys
import threading

import frida

ROOT = pathlib.Path(__file__).resolve().parent.parent
CAPTURE_DIR = ROOT / "capture"
CURRENT = CAPTURE_DIR / "current.txt"
APP_EXE = r"C:\Program Files\RØDE Microphones\RODE Central\RODE Central.exe"
APP_NAME = "RODE Central.exe"


def now() -> str:
    return dt.datetime.now().strftime("%H:%M:%S.%f")[:-3]


def mark(text: str) -> None:
    log_path = pathlib.Path(CURRENT.read_text(encoding="utf-8").strip())
    with log_path.open("a", encoding="utf-8") as f:
        f.write(f"{now()}  ===== MARK: {text} =====\n")
    print(f"marked {log_path.name}: {text}")


def find_pid(device) -> int | None:
    for p in device.enumerate_processes():
        if p.name.lower() == APP_NAME.lower():
            return p.pid
    return None


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--spawn", action="store_true", help="launch RODE Central under Frida")
    ap.add_argument("--mark", help="append a marker line to the current log and exit")
    args = ap.parse_args()

    if args.mark:
        mark(args.mark)
        return

    CAPTURE_DIR.mkdir(exist_ok=True)
    log_path = CAPTURE_DIR / f"session-{dt.datetime.now():%Y%m%d-%H%M%S}.log"
    CURRENT.write_text(str(log_path), encoding="utf-8")
    logf = log_path.open("a", encoding="utf-8", buffering=1)

    def write(line: str) -> None:
        logf.write(line + "\n")
        print(line, flush=True)

    device = frida.get_local_device()
    if args.spawn:
        pid = device.spawn([APP_EXE])
    else:
        pid = find_pid(device)
        if pid is None:
            sys.exit(f"{APP_NAME} is not running. Start it, or use --spawn.")
    session = device.attach(pid)

    def on_message(msg, _data):
        if msg["type"] == "send":
            p = msg["payload"]
            if p["dir"] == "info":
                write(f"{now()}  info  {p['api']}: {p['extra']}")
            else:
                extra = f"  [{p['extra']}]" if p["extra"] else ""
                write(f"{now()}  {p['dir']}  {p['api']:<20} h={p['handle']} "
                      f"len={p['len']:<3} {p['hex']}{extra}")
        else:
            write(f"{now()}  ERROR  {msg.get('stack') or msg}")

    done = threading.Event()
    session.on("detached", lambda reason, *_: (write(f"{now()}  detached: {reason}"), done.set()))

    script = session.create_script((ROOT / "tools" / "hid_hook.js").read_text(encoding="utf-8"))
    script.on("message", on_message)
    script.load()
    if args.spawn:
        device.resume(pid)
    write(f"{now()}  capturing pid {pid} -> {log_path}")

    try:
        done.wait()
    except KeyboardInterrupt:
        pass
    finally:
        logf.close()


if __name__ == "__main__":
    main()
