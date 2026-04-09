#!/usr/bin/env python3
"""Script de démarrage rapide pour Nectar-Render."""

import subprocess
import sys
import time
import webbrowser
import signal

FRONTEND_URL = "http://localhost:3000"
BACKEND_URL = "http://localhost:8000"

processes = []


def signal_handler(sig, frame):
    print("\n\nArrêt des serveurs...")
    for p in processes:
        p.terminate()
    sys.exit(0)


def start_server(cmd, name, port):
    """Démarre un serveur en arrière-plan sans afficher les logs."""
    proc = subprocess.Popen(
        cmd,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
        creationflags=subprocess.CREATE_NEW_PROCESS_GROUP
        if sys.platform == "win32"
        else 0,
    )
    processes.append(proc)
    return proc


def wait_for_server(url, timeout=10):
    """Attend qu'un serveur soit prêt."""
    import urllib.request

    start = time.time()
    while time.time() - start < timeout:
        try:
            urllib.request.urlopen(url, timeout=1)
            return True
        except Exception:
            time.sleep(0.2)
    return False


def main():
    signal.signal(signal.SIGINT, signal_handler)

    print("Démarrage des serveurs...\n")

    start_server(
        [sys.executable, "-m", "http.server", "3000", "--directory", "frontend"],
        "Frontend",
        3000,
    )

    start_server(
        [
            sys.executable,
            "-m",
            "uvicorn",
            "backend.main:app",
            "--host",
            "127.0.0.1",
            "--port",
            "8000",
            "--reload",
        ],
        "Backend",
        8000,
    )

    time.sleep(1)

    print(f"Frontend : {FRONTEND_URL}")
    print(f"Backend  : {BACKEND_URL}\n")
    print("Appuyez sur Ctrl+C pour arrêter les serveurs")

    webbrowser.open(FRONTEND_URL)

    try:
        while True:
            time.sleep(1)
    except KeyboardInterrupt:
        pass


if __name__ == "__main__":
    main()
