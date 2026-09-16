import sys
from pathlib import Path

from fastapi.testclient import TestClient

sys.path.insert(0, str(Path(__file__).parents[1]))

import main as api


def test_scanner_status_starts_false_and_expires(monkeypatch):
    now = [100.0]
    monkeypatch.setattr(api, "monotonic", lambda: now[0])
    monkeypatch.setattr(api, "scanner_ready", False)
    monkeypatch.setattr(api, "scanner_heartbeat", None)
    client = TestClient(api.app)

    assert client.get("/scanner/status").json() == {"ready": False}

    response = client.post("/scanner/status", json={"ready": True})
    assert response.status_code == 200
    assert api.scanner_ready is True
    assert api.scanner_heartbeat == 100.0
    assert client.get("/scanner/status").json() == {"ready": True}

    now[0] = 115.0
    assert client.get("/scanner/status").json() == {"ready": True}

    now[0] = 115.01
    assert client.get("/scanner/status").json() == {"ready": False}


def test_scanner_status_accepts_busy_heartbeat(monkeypatch):
    monkeypatch.setattr(api, "monotonic", lambda: 200.0)
    client = TestClient(api.app)

    response = client.post("/scanner/status", json={"ready": False})

    assert response.status_code == 200
    assert api.scanner_ready is False
    assert api.scanner_heartbeat == 200.0
