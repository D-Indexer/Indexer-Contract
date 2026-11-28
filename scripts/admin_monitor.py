#!/usr/bin/env python3
"""Poll the Folder contract's events and alert on privileged actions.

Run every 15 minutes by .github/workflows/admin-monitor.yml. Reads events since
a stored cursor with RPC `getEvents`, decodes privileged ones (admin changes,
template registration, treasury/price changes, and future upgrade/pause
events), and posts alerts to a webhook and/or a GitHub issue. Actors missing
from the expected-admin allowlist raise a high-priority alert. Standard
library only.

Environment:
  RPC_URL, CONTRACT_ID, NETWORK   default to deployments/testnet.json
  EXPECTED_ADMINS                 comma-separated G.../C... allowlist
                                  (default: the deployment's `admin`)
  STATE_FILE                      cursor file (default .admin-monitor/state.json)
  ALERT_WEBHOOK_URL               optional; receives a JSON POST per alert
  GITHUB_TOKEN, GITHUB_REPOSITORY optional; opens one issue per alert
  DRY_RUN=1                       print alerts instead of posting them
"""
import base64
import json
import os
import pathlib
import struct
import sys
import urllib.request

ROOT = pathlib.Path(__file__).resolve().parent.parent

PRIVILEGED = {"init", "adm_add", "adm_rm", "tmpl_reg", "treas_set", "tmpl_price",
              "upgrade", "pause", "unpause"}
# Event data is a vec of the event struct's fields (see src/events.rs).
FIELDS = {
    "init": ["admin"],
    "adm_add": ["admin", "new_admin"],
    "adm_rm": ["admin", "target"],
    "tmpl_reg": ["template_id", "metadata_uri", "creator"],
    "treas_set": ["admin", "treasury"],
    "tmpl_price": ["admin", "template_id", "price"],
}
ACTOR_FIELD = {"tmpl_reg": "creator"}
PAGE_LIMIT = 200
# First run without a cursor: look back ~1 hour (5 s ledgers).
FIRST_RUN_LOOKBACK = 720

# ── Minimal XDR ScVal decoder ────────────────────────────────────────────────


def _crc16_xmodem(data):
    crc = 0
    for byte in data:
        crc ^= byte << 8
        for _ in range(8):
            crc = ((crc << 1) ^ 0x1021) if crc & 0x8000 else crc << 1
            crc &= 0xFFFF
    return crc


def strkey(version, payload):
    body = bytes([version]) + payload
    return base64.b32encode(body + struct.pack("<H", _crc16_xmodem(body))).decode().rstrip("=")


class _Reader:
    def __init__(self, data):
        self.data, self.pos = data, 0

    def take(self, n):
        if self.pos + n > len(self.data):
            raise ValueError("truncated XDR")
        out = self.data[self.pos:self.pos + n]
        self.pos += n
        return out

    def u32(self):
        return struct.unpack(">I", self.take(4))[0]

    def opaque(self):
        n = self.u32()
        out = self.take(n)
        self.take((4 - n % 4) % 4)
        return out


def _address(r):
    kind = r.u32()
    if kind == 0:  # account: PublicKey (ed25519)
        r.u32()
        return strkey(6 << 3, r.take(32))
    if kind == 1:  # contract
        return strkey(2 << 3, r.take(32))
    if kind == 2:  # muxed account: id then ed25519
        mux_id = r.take(8)
        return strkey(12 << 3, r.take(32) + mux_id)
    if kind == 3:  # claimable balance
        return strkey(1 << 3, bytes([r.u32()]) + r.take(32))
    if kind == 4:  # liquidity pool
        return strkey(11 << 3, r.take(32))
    raise ValueError(f"unknown ScAddress type {kind}")


class Address(str):
    """Marks decoded addresses so rules can tell them apart from strings."""


def _scval(r):
    t = r.u32()
    if t == 0:
        return bool(r.u32())
    if t == 1:
        return None
    if t in (3, 4):
        return struct.unpack(">I" if t == 3 else ">i", r.take(4))[0]
    if t in (5, 6):
        return struct.unpack(">Q" if t == 5 else ">q", r.take(8))[0]
    if t in (9, 10):
        hi = struct.unpack(">Q" if t == 9 else ">q", r.take(8))[0]
        return (hi << 64) | struct.unpack(">Q", r.take(8))[0]
    if t == 13:
        return r.opaque().hex()
    if t in (14, 15):
        return r.opaque().decode("utf-8", "replace")
    if t in (16, 17):
        if not r.u32():
            return None
        n = r.u32()
        if t == 16:
            return [_scval(r) for _ in range(n)]
        return {str(_scval(r)): _scval(r) for _ in range(n)}
    if t == 18:
        return Address(_address(r))
    raise ValueError(f"unsupported ScVal type {t}")


def decode_scval(b64):
    return _scval(_Reader(base64.b64decode(b64)))


# ── Rules ────────────────────────────────────────────────────────────────────


def decode_event(event):
    """Returns a decoded privileged event, or None for other events."""
    try:
        name = decode_scval(event["topic"][0])
    except (ValueError, IndexError, KeyError):
        return None
    if name not in PRIVILEGED:
        return None
    try:
        data = decode_scval(event["value"])
    except ValueError as err:
        data = f"<undecodable: {err}>"
    fields = FIELDS.get(name)
    if fields and isinstance(data, list) and len(data) >= len(fields):
        details = dict(zip(fields, data))
    else:
        details = {"data": data}
    actor = details.get(ACTOR_FIELD.get(name, "admin"))
    if actor is None and isinstance(data, list):  # future events: first address
        actor = next((v for v in data if isinstance(v, Address)), None)
    return {
        "event": name,
        "actor": actor,
        "details": details,
        "ledger": event.get("ledger"),
        "closed_at": event.get("ledgerClosedAt"),
        "tx_hash": event.get("txHash"),
        "id": event.get("id"),
    }


def evaluate(decoded, expected_admins):
    """Attaches `priority` and `reasons` to a decoded privileged event."""
    reasons = []
    if decoded["actor"] not in expected_admins:
        reasons.append(f"actor {decoded['actor']} is not an expected admin")
    new_admin = decoded["details"].get("new_admin")
    if decoded["event"] == "adm_add" and new_admin not in expected_admins:
        reasons.append(f"unexpected address {new_admin} was granted admin")
    return {**decoded, "priority": "high" if reasons else "normal", "reasons": reasons}


def explorer(network, kind, value):
    return f"https://stellar.expert/explorer/{network}/{kind}/{value}"


def render_alert(alert, network, contract_id):
    title = f"[{alert['priority'].upper()}] privileged `{alert['event']}` on Folder contract ({network})"
    lines = [
        f"**Event:** `{alert['event']}` at ledger {alert['ledger']} ({alert['closed_at']})",
        f"**Actor:** `{alert['actor']}`",
    ]
    lines += [f"- ⚠️ {reason}" for reason in alert["reasons"]]
    lines += ["", "**Details:**"]
    lines += [f"- {key}: `{value}`" for key, value in alert["details"].items()]
    lines += [
        "",
        f"- Transaction: {explorer(network, 'tx', alert['tx_hash'])}",
        f"- Contract: {explorer(network, 'contract', contract_id)}",
    ]
    if isinstance(alert["actor"], str) and alert["actor"].startswith("G"):
        lines.append(f"- Actor account: {explorer(network, 'account', alert['actor'])}")
    return title, "\n".join(lines)


def gap_alert(missed_from, oldest):
    return {
        "event": "cursor_gap", "actor": None, "ledger": oldest, "closed_at": None,
        "tx_hash": None, "id": None, "priority": "high",
        "details": {"missed_ledgers": f"{missed_from}..{oldest - 1}"},
        "reasons": ["stored cursor is older than the RPC retention window; events "
                    "in the missed range were not checked"],
    }


# ── RPC, state and delivery ──────────────────────────────────────────────────


def rpc(url, method, params=None):
    body = json.dumps({"jsonrpc": "2.0", "id": 1, "method": method, "params": params or {}})
    request = urllib.request.Request(
        url, body.encode(), {"Content-Type": "application/json", "User-Agent": "admin-monitor"})
    with urllib.request.urlopen(request, timeout=30) as response:
        reply = json.load(response)
    if "error" in reply:
        raise RuntimeError(f"{method}: {reply['error']}")
    return reply["result"]


def fetch_events(url, contract_id, state):
    """Returns (events, new_state, gap) reading from the stored cursor."""
    health = rpc(url, "getHealth")
    oldest, latest = health["oldestLedger"], health["latestLedger"]
    gap = None
    pagination = {"limit": PAGE_LIMIT}
    params = {"filters": [{"type": "contract", "contractIds": [contract_id]}]}
    if state.get("cursor") and state.get("ledger", 0) >= oldest:
        pagination["cursor"] = state["cursor"]
    else:
        if state.get("ledger"):
            gap = (state["ledger"] + 1, oldest)
            params["startLedger"] = oldest
        else:
            params["startLedger"] = max(oldest, latest - FIRST_RUN_LOOKBACK)
    events = []
    while True:
        result = rpc(url, "getEvents", {**params, "pagination": dict(pagination)})
        events += result["events"]
        cursor = result.get("cursor") or (result["events"][-1]["id"] if result["events"] else None)
        if cursor:
            pagination["cursor"] = cursor
        params.pop("startLedger", None)
        if len(result["events"]) < PAGE_LIMIT:
            break
    new_state = {"cursor": pagination.get("cursor", state.get("cursor")),
                 "ledger": result.get("latestLedger", latest)}
    return events, new_state, gap


def post_json(url, payload, headers=None):
    request = urllib.request.Request(
        url, json.dumps(payload).encode(),
        {"Content-Type": "application/json", "User-Agent": "admin-monitor", **(headers or {})})
    with urllib.request.urlopen(request, timeout=30):
        pass


def deliver(alert, network, contract_id):
    title, body = render_alert(alert, network, contract_id)
    if os.environ.get("DRY_RUN") == "1":
        print(f"{title}\n{body}\n")
        return
    if os.environ.get("ALERT_WEBHOOK_URL"):
        post_json(os.environ["ALERT_WEBHOOK_URL"], {"title": title, "text": body, "alert": alert})
    if os.environ.get("GITHUB_TOKEN") and os.environ.get("GITHUB_REPOSITORY"):
        post_json(
            f"https://api.github.com/repos/{os.environ['GITHUB_REPOSITORY']}/issues",
            {"title": title, "body": body},
            {"Authorization": f"Bearer {os.environ['GITHUB_TOKEN']}",
             "Accept": "application/vnd.github+json"})
    print(title)


def main():
    deployment = json.loads((ROOT / "deployments" / "testnet.json").read_text())
    url = os.environ.get("RPC_URL") or deployment["rpc_url"]
    contract_id = os.environ.get("CONTRACT_ID") or deployment["contract_id"]
    network = os.environ.get("NETWORK") or deployment["network"]
    expected = {a.strip() for a in os.environ.get("EXPECTED_ADMINS", deployment["admin"]).split(",")
                if a.strip()}
    state_file = pathlib.Path(os.environ.get("STATE_FILE", ROOT / ".admin-monitor" / "state.json"))
    state = json.loads(state_file.read_text()) if state_file.exists() else {}

    events, new_state, gap = fetch_events(url, contract_id, state)
    alerts = [gap_alert(*gap)] if gap else []
    for event in events:
        decoded = decode_event(event)
        if decoded:
            alerts.append(evaluate(decoded, expected))
    for alert in alerts:
        deliver(alert, network, contract_id)

    # Save the cursor only after every alert was delivered, so a failed post
    # is retried on the next run instead of being lost.
    state_file.parent.mkdir(parents=True, exist_ok=True)
    state_file.write_text(json.dumps(new_state))
    print(f"checked {len(events)} events up to ledger {new_state['ledger']}; {len(alerts)} alert(s)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
