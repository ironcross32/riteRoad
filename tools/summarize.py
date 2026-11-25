"""Condense a capture log: drop HID identity queries, trim trailing zeros,
and show a repeated request only when its response changes.

Usage: .venv\\Scripts\\python.exe tools\\summarize.py [log] [--since HH:MM:SS] [--all]
Default log is the current session.
"""

import argparse
import pathlib
import re

ROOT = pathlib.Path(__file__).resolve().parent.parent
LINE = re.compile(r"^(\S+)\s+(TX|RX)\s+(\S+)\s+h=\S+\s+len=(\d+)\s*([0-9a-f ]*)(\s+\[.*\])?$")
IDENTITY_IOCTLS = ("0xb01a8", "0xb0193", "0xb01c2")


def trim(hexstr: str) -> str:
    parts = hexstr.split()
    while len(parts) > 3 and parts[-1] == "00":
        parts.pop()
    return " ".join(parts)


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("log", nargs="?")
    ap.add_argument("--since", help="only lines at or after this time (HH:MM:SS)")
    ap.add_argument("--all", action="store_true", help="do not suppress unchanged polls")
    args = ap.parse_args()

    path = pathlib.Path(args.log) if args.log else pathlib.Path(
        (ROOT / "capture" / "current.txt").read_text(encoding="utf-8").strip())

    last_resp: dict[str, str] = {}  # request -> last response
    pending_req = None
    pending_time = None
    for raw in path.read_text(encoding="utf-8").splitlines():
        ts = raw.split()[0] if raw.strip() else ""
        if args.since and ts < args.since:
            continue
        if "MARK:" in raw or "detached" in raw:
            print(raw)
            continue
        m = LINE.match(raw)
        if not m:
            continue
        t, d, api, _n, hexstr, extra = m.groups()
        if extra and any(i in extra for i in IDENTITY_IOCTLS):
            continue
        data = trim(hexstr)
        if d == "TX":
            if pending_req is not None:  # request with no response
                print(f"{pending_time}  TX {pending_req}   (no reply)")
            pending_req, pending_time = data, t
            continue
        # RX
        req = pending_req or "?"
        pending_req = None
        if args.all or last_resp.get(req) != data:
            changed = "" if req not in last_resp else "   <-- changed"
            print(f"{pending_time or t}  TX {req}\n{'':14}RX {data}{changed}")
        last_resp[req] = data


if __name__ == "__main__":
    main()
