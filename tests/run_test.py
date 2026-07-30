#!/usr/bin/env python3
"""
Submit tockloader-rs project for hardware testing via Hilltop broker.

Zips the project directory (entrypoint.sh at archive root), uploads as source,
schedules a job on nrf52840 hardware, waits for completion, and reports pass/fail.
"""
#needed for the "str | None" union syntax to work on older Python
from __future__ import annotations
import argparse
import io
import json
import os
import sys
import requests
import zipfile
from pathlib import Path
from typing import Any #needed for dict[str, Any]
from websockets import ConnectionClosed
from websockets.sync.client import ClientConnection, connect #needed o type the 'ws' parameter 

BASE_URL = "https://tw.semaka.ro:2053"
WS_URL = "wss://tw.semaka.ro:2053/ws"

RUNNER_SLUG = "admin-runner"
USER_EMAIL = "admin@tw.semaka.ro"

PASS_SENTINEL = "-=-= End test pipeline =-=-"

EXCLUDE_DIRS = {".git", "target", "__pycache__", ".venv"}

def load_api_key(api_key_file: str | None) -> str:
    if api_key_file:
        #explicit encoding to avoid inconsistency across OS
        with open(api_key_file, encoding="utf-8") as f:
            return f.read().strip()
    key = os.environ.get("CLIENT_API_KEY")
    if not key:
        print("Error: set CLIENT_API_KEY or pass --api-key-file", file=sys.stderr)
        sys.exit(1)
    return key


def create_project_zip(project_dir: str) -> bytes:
    """Zip the project directory with entrypoint.sh at the archive root."""
    buf = io.BytesIO()
    with zipfile.ZipFile(buf, "w", zipfile.ZIP_DEFLATED) as zf:
        for root, dirs, files in os.walk(project_dir):
            dirs[:] = [d for d in dirs if d not in EXCLUDE_DIRS and not d.startswith(".")]
            for filename in files:
                abs_path = os.path.join(root, filename)
                arcname = os.path.relpath(abs_path, project_dir)
                zf.write(abs_path, arcname)
    return buf.getvalue()


def rest_get_client_jwt(client_api_key: str) -> str:
    resp = requests.post(
        f"{BASE_URL}/api/auth/newUserSession",
        json={"userEmail": USER_EMAIL, "clientApiKey": client_api_key},
        timeout=15,
    )
    resp.raise_for_status()
    # explicit 'str' so mypy --strict doesn't flag an implicit Any leaking out str declared
    # return type
    token: str = resp.json()["token"]
    print("[REST] Got client JWT")
    return token


def rest_upload_source(client_jwt: str, zip_bytes: bytes) -> str:
    resp = requests.post(
        f"{BASE_URL}/client_api/source",
        headers={"Authorization": f"Bearer {client_jwt}"},
        params={"RunnerSlug": RUNNER_SLUG},
        files={"sourceFile": ("project.zip", zip_bytes, "application/zip")},
        timeout=15,
    )
    resp.raise_for_status()
    result = resp.json()
    source_id: str = result["sourceId"]
    print(f"[REST] Source uploaded -> sourceId={source_id}")
    return source_id


def rest_get_job(client_jwt: str, job_id: str) -> dict[str, Any]:
    resp = requests.get(
        f"{BASE_URL}/client_api/job/{job_id}",
        headers={"Authorization": f"Bearer {client_jwt}"},
        timeout=15,
    )
    resp.raise_for_status()
    result: dict[str, Any] = resp.json()
    return result


def rest_download_artifact(client_jwt: str, artifact_id: str) -> bytes:
    resp = requests.get(
        f"{BASE_URL}/client_api/artifact/{artifact_id}/download",
        headers={"Authorization": f"Bearer {client_jwt}"},
        timeout=15,
    )
    resp.raise_for_status()
    return resp.content


def ws_send_recv(ws: ClientConnection, payload: dict[str, Any]) -> dict[str, Any]:
    ws.send(json.dumps(payload))
    raw = ws.recv(timeout=15)
    result: dict[str, Any] = json.loads(raw)
    return result

#Extracted try/except loop from run_client_sesson() making each piece independently readable
def wait_for_job_result(ws: ClientConnection, client_jwt: str, job_id: str) -> dict[str, str]:
    print("Waiting results...")
    while True:
        try:
            raw = ws.recv(timeout=600)
            msg: dict[str, Any] = json.loads(raw)

            if msg.get("command") != "NOTIFY":
                continue

            ws.send(json.dumps({"response": "NOTIFY_ACK"}))
            status = msg.get("job_status")
            if status in ("COMPLETED", "FAILED"):
                job_info = rest_get_job(client_jwt, job_id)
                return {
                    artifact["displayIdentifier"]: rest_download_artifact(client_jwt, artifact["id"]).decode(errors="replace")
                    for artifact in job_info.get("artifacts", [])
                }
        except ConnectionClosed:
            print("[Client WS] Connection closed while waiting for job completion")
            return{}
        except TimeoutError:
            print("Hardware runner timed out")
            return{}


def run_client_session(source_id: str, client_jwt: str, job_description: dict[str, Any], client_api_key: str) -> dict[str, str]:
    with connect(WS_URL) as ws:
        
        ws_send_recv(ws, {
            "command": "HELLO_CLIENT",
            "socket_protocol_version": "1.0.0",
        })
            
        ws_send_recv(ws, {
            "command": "CONFIG_CLIENT",
            "user_identifier": USER_EMAIL,
            "api_key": client_api_key,
            "runner_slug": RUNNER_SLUG,
        })
        print("Scheduling job...")

        resp = ws_send_recv(ws, {
            "command": "SCHEDULE_JOB",
            "job_data_identifier": source_id,
            "job_description": job_description,
        })
        print(f"[Client WS] SCHEDULE_JOB -> {resp}")

        artifacts_by_name: dict[str, str] = {}

        if resp.get("response") == "ACK" and resp.get("data"):
            job_id = resp["data"].get("job_identifier")
            artifacts_by_name = wait_for_job_result(ws, client_jwt, job_id)

        ws_send_recv(ws, {"command": "GOODBYE"})

    return artifacts_by_name


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--api-key-file", help="Path to file containing the client API key (plaintext)")
    args = parser.parse_args()
    #replaced os.path.dirname/abspath/join with pathlib.Path
    # easier to read/chain
    client_api_key = load_api_key(args.api_key_file)
    script_dir = Path(__file__).resolve().parent
    project_dir = script_dir.parent
    job_description_path = script_dir / "job.json"
    job_description: dict[str, Any] = json.loads(job_description_path.read_text(encoding="utf-8"))
    zip_bytes = create_project_zip(str(project_dir))

    client_jwt = rest_get_client_jwt(client_api_key)
    print("Uploading source...")
    source_id = rest_upload_source(client_jwt, zip_bytes)

    artifacts = run_client_session(source_id, client_jwt, job_description, client_api_key)
    print(f"[Zip] Archive size: {len(zip_bytes):,} bytes")
    
    stdout_content = artifacts.get("stdout_artifact", "<not found>")
    stderr_content = artifacts.get("stderr_artifact", "<not found>")

    print("\n" + "=" * 60)
    print("STDOUT:")
    print("=" * 60)
    print(stdout_content)

    print("\n" + "=" * 60)
    print("STDERR:")
    print("=" * 60)
    print(stderr_content)

    for name, content in artifacts.items():
        if name not in ("stdout_artifact", "stderr_artifact"):
            print("\n" + "=" * 60)
            print(f"ARTIFACT: {name}")
            print("=" * 60)
            print(content)

    print("\n" + "=" * 60)
    if PASS_SENTINEL in stdout_content:
        print("RESULT: PASS")
        print("=" * 60)
    else:
        print("RESULT: FAIL")
        print("=" * 60)
        sys.exit(1)

if __name__ == "__main__":
    main()
