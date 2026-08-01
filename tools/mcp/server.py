"""MCP server for the Spotorno wildfire game.

Wraps the running game's local control/inspection API
(`crates/game/src/api.rs`, `http://127.0.0.1:8731` by default) as MCP tools:
read the agent event history, read the incident roster, and drive the
simulation -- play/pause, speed, step, restart, evacuation orders, ignitions,
weather, unit tasking.

This process is a thin HTTP client and nothing else. The game itself has to
already be running (`cargo run --release -p game`) for any tool here to
return anything but a connection error -- there is no simulation inside this
process, only a translation from MCP tool calls to requests against the one
that is playing.

Run directly for local testing:
    python3 tools/mcp/server.py
Or point an MCP client at it with stdio transport; see `.mcp.json` at the
repo root for the entry Claude Code uses.
"""

from __future__ import annotations

import json
import os
import urllib.error
import urllib.request
from typing import Any, Optional

from mcp.server.fastmcp import FastMCP

BASE_URL = os.environ.get("SPOTORNO_API_URL", "http://127.0.0.1:8731")

mcp = FastMCP("spotorno")


def _request(method: str, path: str, body: Optional[dict] = None) -> Any:
    url = f"{BASE_URL}{path}"
    data = json.dumps(body).encode("utf-8") if body is not None else None
    req = urllib.request.Request(url, data=data, method=method)
    if data is not None:
        req.add_header("Content-Type", "application/json")
    try:
        with urllib.request.urlopen(req, timeout=5) as resp:
            return json.loads(resp.read().decode("utf-8"))
    except urllib.error.HTTPError as e:
        # The game's API always answers with a JSON body, errors included --
        # surface that message rather than a bare status code.
        try:
            detail = json.loads(e.read().decode("utf-8"))
        except Exception:
            detail = {"error": str(e)}
        raise RuntimeError(f"{path} -> HTTP {e.code}: {detail.get('error', detail)}") from e
    except urllib.error.URLError as e:
        raise RuntimeError(
            f"could not reach the game at {BASE_URL} ({e.reason}). "
            "Is `cargo run --release -p game` running?"
        ) from e


def _get(path: str) -> Any:
    return _request("GET", path)


def _post(path: str, body: dict) -> Any:
    return _request("POST", path, body)


# --- inspection ---------------------------------------------------------


@mcp.tool()
def get_status() -> dict:
    """Incident-wide readout: simulated clock, play state, speed, weather,
    and household/people/unit counts by status. Call this first to see what
    is going on before drilling into any one agent."""
    return _get("/status")


@mcp.tool()
def get_recent_history(limit: int = 50) -> list:
    """The most recent simulation events across every agent, newest first --
    status changes, decisions, evacuation orders, travel state, unit tasking,
    ignitions, weather changes. An incident-wide activity feed."""
    return _get(f"/history/recent?limit={int(limit)}")


@mcp.tool()
def get_agent_history(kind: str, id: Optional[int] = None) -> list:
    """The full recorded history of one agent, oldest first -- what an
    "interview this agent" answer would be built from.

    kind: one of "household", "person", "traveller", "unit", "command".
    id: the agent's index (see list_households / list_units for the ids in
        play); omit only for kind="command", the incident-wide log.
    """
    q = f"kind={kind}"
    if id is not None:
        q += f"&id={int(id)}"
    return _get(f"/history/subject?{q}")


@mcp.tool()
def list_households(status: Optional[str] = None, ordered: Optional[bool] = None) -> list:
    """The household roster: id, status, whether an evacuation order has
    reached them, their stated intent, household size and home position.
    Optionally filtered by status (e.g. "preparing", "evacuated / safe",
    "defending") or by whether an order has been issued."""
    rows = _get("/agents/households")
    if status is not None:
        rows = [r for r in rows if r["status"] == status]
    if ordered is not None:
        rows = [r for r in rows if r["ordered"] == ordered]
    return rows


@mcp.tool()
def list_units() -> list:
    """The suppression roster: every engine, hand crew and air tanker, with
    its callsign, state, current task and position."""
    return _get("/agents/units")


# --- control -------------------------------------------------------------


@mcp.tool()
def set_playing(playing: bool) -> dict:
    """Play or pause the simulation clock."""
    return _post("/control/play", {"playing": playing})


@mcp.tool()
def set_speed(speed: float) -> dict:
    """Set simulated seconds per wall-clock second (clamped to the game's
    own min/max, roughly 1x-512x)."""
    return _post("/control/speed", {"speed": speed})


@mcp.tool()
def step() -> dict:
    """Advance exactly one decision tick (~5 simulated seconds, rounded up
    to the fire model's own quantum), whether or not the sim is playing."""
    return _post("/control/step", {})


@mcp.tool()
def restart() -> dict:
    """Rebuild the fire and every agent from scratch and replay the
    ignition list -- the same fire and seed, a genuinely clean run. Also
    clears the event history, since a restart discards everything that
    happened in the previous run."""
    return _post("/control/restart", {})


@mcp.tool()
def order_evacuation(
    all: bool = True,
    x: Optional[float] = None,
    y: Optional[float] = None,
    radius_m: float = 2000.0,
) -> dict:
    """Issue an evacuation order. With all=True (the default) it reaches
    every household regardless of distance; pass x/y (world metres) and a
    radius_m to target one area instead. The order still has to arrive over
    each household's own warning channel and be acted on -- this does not
    teleport anyone."""
    body: dict = {}
    if not all and x is not None and y is not None:
        body = {"x": x, "y": y, "radius_m": radius_m}
    return _post("/control/order_evacuation", body)


@mcp.tool()
def place_ignition(x: float, y: float, radius_m: float = 120.0) -> dict:
    """Light a new fire patch at a world position (metres, scenario frame).
    Fails if there is no burnable fuel there -- try a point in vegetation,
    not on a road or a building footprint. radius_m is clamped to the
    game's own placeable range (roughly 60-600 m)."""
    return _post("/control/ignite", {"x": x, "y": y, "radius_m": radius_m})


@mcp.tool()
def set_weather(
    wind_dir_deg: Optional[float] = None,
    wind_speed_kmh: Optional[float] = None,
    moisture_pct: Optional[float] = None,
) -> dict:
    """Change the fire's weather boundary condition, from now on -- this
    changes what the front does next without rewriting what it has already
    burned. Any field left out keeps its current value. wind_dir_deg is the
    meteorological bearing the wind blows FROM."""
    body = {}
    if wind_dir_deg is not None:
        body["wind_dir_deg"] = wind_dir_deg
    if wind_speed_kmh is not None:
        body["wind_speed_kmh"] = wind_speed_kmh
    if moisture_pct is not None:
        body["moisture_pct"] = moisture_pct
    return _post("/control/weather", body)


@mcp.tool()
def assign_unit_task(
    id: int,
    task: str,
    x: Optional[float] = None,
    y: Optional[float] = None,
    from_x: Optional[float] = None,
    from_y: Optional[float] = None,
    to_x: Optional[float] = None,
    to_y: Optional[float] = None,
) -> dict:
    """Order one suppression unit (see list_units for ids). task is one of:

    - "hold": stand by where it is.
    - "return": go back to staging.
    - "attack": work the fire edge nearest (x, y) -- needs x/y.
    - "drop": aircraft only, one load on (x, y) -- needs x/y.
    - "line": hand crews only, cut a break from (from_x, from_y) to
      (to_x, to_y) -- needs all four.

    Fails with the same refusal the in-game panel would show (wrong unit
    kind for the task, no road within hose reach, and so on)."""
    body: dict = {"id": id, "task": task}
    for k, v in (
        ("x", x),
        ("y", y),
        ("from_x", from_x),
        ("from_y", from_y),
        ("to_x", to_x),
        ("to_y", to_y),
    ):
        if v is not None:
            body[k] = v
    return _post("/control/unit_task", body)


if __name__ == "__main__":
    mcp.run()
