#!/usr/bin/env python3
"""
Submit tockloader-rs project for hardware testing via Hilltop broker.

Zips the project directory (entrypoint.sh at archive root), uploads as source,
schedules a job on nrf52840 hardware, waits for completion, and reports pass/fail.
"""

import argparse
import asyncio
import io
import json
import os
import sys

import requests
import websockets
import zipfile

BASE_URL = "https://tw.semaka.ro:2053"
WS_URL = "wss://tw.semaka.ro:2053/ws"

RUNNER_SLUG = "admin-runner"
USER_EMAIL = "admin@tw.semaka.ro"

PASS_SENTINEL = "-=-= End test pipeline =-=-"


def load_api_key(api_key_file: str | None) -> str:
    if api_key_file:
        with open(api_key_file) as f:
            return f.read().strip()
    key = os.environ.get("CLIENT_API_KEY")
    if not key:
        print("Error: set CLIENT_API_KEY or pass --api-key-file", file=sys.stderr)
        sys.exit(1)
    return key

EXCLUDE_DIRS = {".git", "target", "__pycache__", ".venv"}


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
    )
    resp.raise_for_status()
    token = resp.json()["token"]
    print("[REST] Got client JWT")
    return token


def rest_upload_source(client_jwt: str, zip_bytes: bytes) -> str:
    resp = requests.post(
        f"{BASE_URL}/client_api/source",
        headers={"Authorization": f"Bearer {client_jwt}"},
        params={"RunnerSlug": RUNNER_SLUG},
        files={"sourceFile": ("project.zip", zip_bytes, "application/zip")},
    )
    resp.raise_for_status()
    result = resp.json()
    print(f"[REST] Source uploaded -> sourceId={result['sourceId']}")
    return result["sourceId"]


def rest_get_job(client_jwt: str, job_id: str) -> dict:
    resp = requests.get(
        f"{BASE_URL}/client_api/job/{job_id}",
        headers={"Authorization": f"Bearer {client_jwt}"},
    )
    resp.raise_for_status()
    return resp.json()


def rest_download_artifact(client_jwt: str, artifact_id: str) -> bytes:
    resp = requests.get(
        f"{BASE_URL}/client_api/artifact/{artifact_id}/download",
        headers={"Authorization": f"Bearer {client_jwt}"},
    )
    resp.raise_for_status()
    return resp.content


async def ws_send_recv(ws, payload: dict) -> dict:
    await ws.send(json.dumps(payload))
    raw = await ws.recv()
    return json.loads(raw)


async def run_client_session(source_id: str, client_jwt: str, job_description: dict, client_api_key: str) -> dict:
    async with websockets.connect(WS_URL) as ws:
        resp = await ws_send_recv(ws, {
            "command": "HELLO_CLIENT",
            "socket_protocol_version": "1.0.0",
        })
        print(f"[Client WS] HELLO_CLIENT -> {resp}")

        resp = await ws_send_recv(ws, {
            "command": "CONFIG_CLIENT",
            "user_identifier": USER_EMAIL,
            "api_key": client_api_key,
            "runner_slug": RUNNER_SLUG,
        })
        print(f"[Client WS] CONFIG_CLIENT -> {resp}")

        resp = await ws_send_recv(ws, {
            "command": "SCHEDULE_JOB",
            "job_data_identifier": source_id,
            "job_description": job_description,
        })
        print(f"[Client WS] SCHEDULE_JOB -> {resp}")

        artifacts_by_name = {}

        if resp.get("response") == "ACK" and resp.get("data"):
            job_id = resp["data"].get("job_identifier")
            print(f"[Client WS] Job scheduled -> jobId={job_id}")
            print("[Client WS] Waiting for COMPLETED or FAILED notification...")

            while True:
                try:
                    raw = await ws.recv()
                    msg = json.loads(raw)

                    if msg.get("command") == "NOTIFY":
                        await ws.send(json.dumps({"response": "NOTIFY_ACK"}))
                        status = msg.get("job_status")
                        print(f"[Client WS] NOTIFY job_status={status} message={msg.get('job_status_message', '')!r}")
                        if status in ("COMPLETED", "FAILED"):
                            loop = asyncio.get_event_loop()
                            job_info = await loop.run_in_executor(None, rest_get_job, client_jwt, job_id)
                            for artifact in job_info.get("artifacts", []):
                                content = await loop.run_in_executor(
                                    None, rest_download_artifact, client_jwt, artifact["id"]
                                )
                                artifacts_by_name[artifact["displayIdentifier"]] = content.decode(errors="replace")
                            break
                    else:
                        print(f"[Client WS] Received: {msg}")
                except websockets.ConnectionClosed:
                    print("[Client WS] Connection closed while waiting for job completion")
                    break

        resp = await ws_send_recv(ws, {"command": "GOODBYE"})
        print(f"[Client WS] GOODBYE -> {resp}")

    return artifacts_by_name


async def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--api-key-file", help="Path to file containing the client API key (plaintext)")
    args = parser.parse_args()

    client_api_key = load_api_key(args.api_key_file)

    script_dir = os.path.dirname(os.path.abspath(__file__))
    project_dir = os.path.dirname(script_dir)
    job_description_path = os.path.join(script_dir, "job.json")

    with open(job_description_path) as f:
        job_description = json.load(f)

    print(f"[Config] Project dir: {project_dir}")
    print(f"[Config] Job description: {job_description_path}")

    print("[Zip] Creating project archive...")
    zip_bytes = create_project_zip(project_dir)
    print(f"[Zip] Archive size: {len(zip_bytes):,} bytes")

    client_jwt = rest_get_client_jwt(client_api_key)
    source_id = rest_upload_source(client_jwt, zip_bytes)

    artifacts = await run_client_session(source_id, client_jwt, job_description, client_api_key)

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
    asyncio.run(main())
