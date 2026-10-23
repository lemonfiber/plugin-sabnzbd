#!/usr/bin/env python3
"""Record every conformance case of the contract the adapter speaks.

**This is CI harness, not plugin content.** Nothing under `.github/` is
installed, and lemonfiber never runs any of it (`F3-R6`).

Each recording under `conformance/<capability>@<major>/<case>.json` is the
adapter's answer to one case of that contract's `conformance.json` (ARCH-R217),
asked with the case's own `asked`, in the order the contract lists them. It is
taken against a throwaway SABnzbd at the digest `plugin.toml` pins for the
plugin's own service, started from the starting files under `config/sabnzbd/`.
The adapter runs from this repository's Dockerfile, as lemonfiber runs it: the
caller's uid, read-only, no capabilities, the key and the upstream credential
owner-only in its configuration directory. This script stands where lemonfiber
stands: it reads the key SABnzbd wrote to `sabnzbd.ini` on its first start,
writes it to the adapter's `upstream.json` beside `lemonfiber.key`, and asks.

    python3 .github/record.py            write conformance/
    python3 .github/record.py --check    record again and compare

Needs Docker. Exit 0 = recorded (or matched), 1 = it did not.
"""

from __future__ import annotations

import argparse
import json
import os
import pathlib
import re
import secrets
import shutil
import subprocess
import sys
import tempfile
import time
import tomllib
import urllib.error
import urllib.parse
import urllib.request
import uuid

ROOT = pathlib.Path(__file__).resolve().parents[1]
RECORDINGS = ROOT / "conformance"
CONTRACTS = "https://raw.githubusercontent.com/lemonfiber/lemonfiber/{rev}/contract/capabilities/{capability}/v{major}/conformance.json"
JSON = "application/json"
STARTUP_S = 180
ANSWER_S = 30
ADAPTER_PORT = 8080
SEGMENT = re.compile(r"[a-z][a-z0-9_.]*")
MAJOR = re.compile(r"[1-9][0-9]*")
API_KEY = "api_key"
NOTES = {
    "refuses-without-the-key": "Asked without the plugin's key: the kit refuses it before the adapter is reached.",
    "transfers-answers": "What a fresh SABnzbd is downloading: nothing.",
    "pulling-answers": "Whether a fresh SABnzbd is fetching: it is, with nothing to fetch.",
    "stop-answers": "The downloader paused, and the queue read back to confirm it stopped.",
    "resume-answers": "The downloader resumed, and the queue read back to confirm it fetches again.",
    "throttled-answers": "The rates a fresh SABnzbd is held to: none, no upload to hold, and no schedule.",
    "restrain-answers": "Held to the active rate around the clock with no window, and read back.",
    "moving-answers": "How fast it is moving with nothing queued.",
    "moved-answers": "What it pulled in the month asked about, with no account configured.",
    "filed-answers": "Every category the starting files and SABnzbd's own defaults hold, and the folder each completes into.",
    "file_under-answers": "A category filed under the folder the case names.",
    "accounts-answers": "The Usenet accounts of a SABnzbd nobody has given one.",
}


def docker(*args: str, check: bool = True) -> str:
    done = subprocess.run(["docker", *args], capture_output=True, text=True, check=False)
    if check and done.returncode != 0:
        raise RuntimeError(f"docker {' '.join(args)}: {done.stderr.strip()}")
    return done.stdout.strip()


def manifest() -> tuple[str, str, int]:
    """The plugin's own service: its id, its pinned image and the port it listens on."""
    read = tomllib.loads((ROOT / "plugin.toml").read_text(encoding="utf-8"))
    plugin = read["plugin"]["id"]
    upstream = next(one for one in read["service"] if one["id"] == plugin)
    return plugin, f"{upstream['image']}@{upstream['digest']}", upstream["listens"]


def core_rev() -> str:
    read = tomllib.loads((ROOT / "Cargo.toml").read_text(encoding="utf-8"))
    return read["dependencies"]["lemonfiber-adapter-kit"]["rev"]


def published_cases(spoken: str) -> list[dict]:
    capability, major = spoken.split("@")
    url = CONTRACTS.format(rev=core_rev(), capability=capability, major=major)
    with urllib.request.urlopen(url, timeout=ANSWER_S) as answer:
        return json.load(answer)["cases"]


def asked(base: str, path: str, key: str | None, body: dict | None = None) -> tuple[int, str, str]:
    request = urllib.request.Request(
        f"{base}{path}",
        data=None if body is None else json.dumps(body).encode(),
        method="GET" if body is None else "POST",
        headers={"Content-Type": JSON, "Accept": JSON} | ({"Authorization": f"Bearer {key}"} if key else {}),
    )
    try:
        with urllib.request.urlopen(request, timeout=ANSWER_S) as answer:
            return answer.status, answer.headers.get("content-type", ""), answer.read().decode()
    except urllib.error.HTTPError as refused:
        return refused.code, refused.headers.get("content-type", ""), refused.read().decode()


def recording(pinned: str, path: str, status: int, kind: str, body: str, note: str) -> dict:
    response: dict = {"status": status}
    if kind:
        response["headers"] = {"content-type": kind}
    if body.strip() == "null":
        response["body_starts_with"] = "null"
    elif body.strip():
        response["json"] = json.loads(body)
    return {
        "recorded_from": pinned,
        "note": note,
        "request": {"method": "POST", "path": path, "accept": JSON},
        "response": response,
    }


def generated_key(ini: pathlib.Path) -> str | None:
    """The key SABnzbd wrote to `sabnzbd.ini` on its first start, as lemonfiber reads it."""
    if not ini.is_file():
        return None
    for line in ini.read_text(encoding="utf-8", errors="replace").splitlines():
        name, _, value = line.partition("=")
        if name.strip() == API_KEY and value.strip():
            return value.strip()
    return None


def owner_only(directory: pathlib.Path) -> None:
    for file in directory.iterdir():
        file.chmod(0o600)
    directory.chmod(0o700)


def waited(what: str, ready) -> object:
    deadline = time.monotonic() + STARTUP_S
    while time.monotonic() < deadline:
        found = ready()
        if found:
            return found
        time.sleep(2)
    raise RuntimeError(f"{what} within {STARTUP_S}s")


def recorded_live(plugin: str, pinned: str, listens: int) -> tuple[list[str], dict[tuple[str, str], dict]]:
    """What the adapter speaks, and every case asked of it in front of a throwaway SABnzbd."""
    name = f"{plugin}-record-{uuid.uuid4().hex[:8]}"
    network = f"{name}-net"
    key = secrets.token_hex(24)
    with tempfile.TemporaryDirectory() as held:
        held = pathlib.Path(held)
        upstream, data, adapter = held / "upstream", held / "data", held / "adapter"
        shutil.copytree(ROOT / "config" / plugin, upstream)
        data.mkdir()
        adapter.mkdir()
        try:
            docker("network", "create", network)
            docker("run", "-d", "--name", f"{name}-upstream", "--network", network,
                   "--network-alias", plugin, "-e", f"PUID={os.getuid()}", "-e", f"PGID={os.getgid()}",
                   "-e", "TZ=Etc/UTC", "-v", f"{upstream}:/config", "-v", f"{data}:/data", pinned)
            api_key = waited("SABnzbd wrote no key", lambda: generated_key(upstream / "sabnzbd.ini"))
            waited("SABnzbd did not answer its version", lambda: docker(
                "exec", f"{name}-upstream", "curl", "-fsS",
                f"http://localhost:{listens}/api?mode=version&output=json", check=False))
            (adapter / "lemonfiber.key").write_text(key, encoding="utf-8")
            (adapter / "upstream.json").write_text(json.dumps({API_KEY: api_key}), encoding="utf-8")
            owner_only(adapter)
            docker("build", "-q", "-t", f"{name}:adapter", str(ROOT))
            docker("run", "-d", "--name", f"{name}-adapter", "--network", network,
                   "--user", f"{os.getuid()}:{os.getgid()}", "--read-only", "--cap-drop", "ALL",
                   "--security-opt", "no-new-privileges", "-v", f"{adapter}:/config:ro",
                   "-p", f"127.0.0.1::{ADAPTER_PORT}", f"{name}:adapter")
            port = docker("port", f"{name}-adapter", str(ADAPTER_PORT)).splitlines()[0].rsplit(":", 1)[1]
            base = f"http://127.0.0.1:{port}"
            waited("the adapter was not ready", lambda: answering(base, key))
            speaks = json.loads(asked(base, "/lemonfiber/adapter/v1/about", key)[2])["speaks"]
            said = recorded(base, key, pinned, speaks)
            leaked = [case for case, one in said.items()
                      if api_key in json.dumps(one) or key in json.dumps(one)]
            if leaked:
                raise RuntimeError(f"an answer holds a credential, so nothing is written: {leaked}")
            return speaks, said
        finally:
            for container in (f"{name}-adapter", f"{name}-upstream"):
                docker("rm", "-fv", container, check=False)
            docker("rmi", f"{name}:adapter", check=False)
            docker("network", "rm", network, check=False)
            docker("run", "--rm", "-v", f"{held}:/held", "--entrypoint", "chown", pinned,
                   "-R", f"{os.getuid()}:{os.getgid()}", "/held", check=False)


def answering(base: str, key: str) -> bool:
    try:
        status, _, body = asked(base, "/lemonfiber/adapter/v1/ready", key)
    except (urllib.error.URLError, ConnectionError):
        return False
    return status == 200 and body.strip() == "true"


def recorded(base: str, key: str, pinned: str, speaks: list[str]) -> dict[tuple[str, str], dict]:
    """Every case of every contract spoken, asked in the order each contract lists them."""
    said: dict[tuple[str, str], dict] = {}
    for spoken in speaks:
        capability, major = spoken.split("@")
        for case in published_cases(spoken):
            path = operation_path(capability, major, case["operation"])
            status, kind, body = asked(base, path, key if case["keyed"] else None, case["asked"])
            note = NOTES.get(case["case"], "")
            if not note:
                raise RuntimeError(f"{spoken}/{case['case']} has no note to record it with")
            said[(spoken, case["case"])] = recording(pinned, path, status, kind, body, note)
    return said


def operation_path(capability: str, major: str, operation: str) -> str:
    """Where an operation is asked, each segment held to the shape the contract writes it in."""
    for segment, shape in ((capability, SEGMENT), (major, MAJOR), (operation, SEGMENT)):
        if not shape.fullmatch(segment):
            raise RuntimeError(f"{segment!r} is not a segment a contract names")
    quoted = [urllib.parse.quote(segment, safe="") for segment in (capability, major, operation)]
    return "/lemonfiber/{}/v{}/{}".format(*quoted)


def filed(said: dict[tuple[str, str], dict], spoken: str, case: str, checking: bool) -> str | None:
    """One case written to conformance/, or compared with what is there; the fault, if any."""
    got = said.get((spoken, case))
    path = RECORDINGS / spoken / f"{case}.json"
    if got is None:
        return f"{spoken}/{case} was not asked"
    if checking:
        held = json.loads(path.read_text(encoding="utf-8")) if path.is_file() else None
        return None if held == got else f"{path.relative_to(ROOT)} is not what SABnzbd answers now"
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(got, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")
    return None


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--check", action="store_true", help="record again and compare with conformance/")
    checking = parser.parse_args().check

    plugin, pinned, listens = manifest()
    speaks, said = recorded_live(plugin, pinned, listens)
    faults = [fault for spoken in speaks for case in published_cases(spoken)
              if (fault := filed(said, spoken, case["case"], checking)) is not None]
    for fault in faults:
        print(f"::error::{fault}")
    print(f"{len(said)} cases asked against {pinned}; {len(faults)} fault(s).")
    return 1 if faults else 0


if __name__ == "__main__":
    sys.exit(main())
