import asyncio
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parents[1]))

import main as api


def test_scanner_status_starts_false_and_expires(monkeypatch):
    now = [100.0]
    monkeypatch.setattr(api, "monotonic", lambda: now[0])
    monkeypatch.setattr(api, "scanner_ready", False)
    monkeypatch.setattr(api, "scanner_heartbeat", None)

    assert asyncio.run(api.get_scanner_status()).model_dump() == {"ready": False}

    asyncio.run(api.mod_scanner_status(api.ScannerStatus(ready=True)))
    assert api.scanner_ready is True
    assert api.scanner_heartbeat == 100.0
    assert asyncio.run(api.get_scanner_status()).model_dump() == {"ready": True}

    now[0] = 115.0
    assert asyncio.run(api.get_scanner_status()).model_dump() == {"ready": True}

    now[0] = 115.01
    assert asyncio.run(api.get_scanner_status()).model_dump() == {"ready": False}


def test_scanner_status_accepts_busy_heartbeat(monkeypatch):
    monkeypatch.setattr(api, "monotonic", lambda: 200.0)
    monkeypatch.setattr(api, "scanner_ready", True)
    monkeypatch.setattr(api, "scanner_heartbeat", 150.0)

    asyncio.run(api.mod_scanner_status(api.ScannerStatus(ready=False)))

    assert api.scanner_ready is False
    assert api.scanner_heartbeat == 200.0
