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
import time
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
    """Incident-wide readout: which scenario is loaded (id/name/is_dev),
    simulated clock, play state, speed, weather, fire health (burnt_ha,
    active_front_cells, peak_fireline_kw_m), structure damage counts
    (threatened/alight/destroyed), and household/people/unit counts by
    status. Call this first to see what is going on before drilling into any
    one agent, and again after load_scenario/set_profile_share/restart to
    confirm the change actually took."""
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
    Also carries `subtype` (the authored profile driving this household's
    decisions, id and name) and `decision` (that profile's most recent
    action/priority/prep_scale/urgency) whenever a household behaviour
    library is loaded -- both are null under the hand-written model. This is
    how to check a set_profile_share/reload_behaviour_library call actually
    changed what an agent does, not just which file it reads from.
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
    its callsign, state, current task and position. Also carries `policy`
    (which authored unit-safety profile governs it, id and name) whenever a
    suppression-unit behaviour library is loaded."""
    return _get("/agents/units")


@mcp.tool()
def list_people() -> list:
    """The separated-people roster: everyone who is not with their household
    (out when the incident started, or separated since) and is therefore an
    agent in their own right rather than part of a household's decision.
    Roughly 400 of 1,577 people on the shipped scenario. id, which household
    they belong to, status, whether they are currently away, and position.
    Each also carries `subtype` (the authored profile driving them, if a
    people-behaviour library is loaded) and `decision` (its most recent
    action/priority/urgency), the same as list_households."""
    return _get("/agents/people")


# --- scenarios -------------------------------------------------------------


@mcp.tool()
def list_scenarios() -> dict:
    """Every scenario the game can load: the four real incidents (spotorno,
    mati, pedrogao, rhodes) and the synthetic "development laboratories"
    built to exercise one ABM mechanism in isolation at a time -- abm_micro,
    congestion_funnel, fire_extreme, fire_mild, mass_evacuation, policy_lab,
    road_cutoff, suppression_access, town_scale. For each: id, name,
    description, location, is_dev, and population counts. Works even before
    any scenario has been launched, so it is a reasonable first call in a
    fresh game process."""
    return _get("/scenarios")


@mcp.tool()
def load_scenario(id: str, timeout_s: float = 20.0) -> dict:
    """Switch the running incident to a different scenario, in the same game
    process -- no need to kill and relaunch `cargo run`. Equivalent to the
    in-game Scenario -> "Load scenario..." menu item, aimed automatically at
    `id` (see list_scenarios for valid ids).

    Blocks and polls get_status until the switch has actually completed --
    the game needs to tear down the old scene, load the new scenario's
    terrain/population/roads and rebuild it, which takes real wall-clock time
    (roughly 0.2-1s locally) -- and returns the fresh status once it has, or
    raises if it has not finished within timeout_s."""
    _post("/control/load_scenario", {"id": id})
    deadline = time.monotonic() + timeout_s
    last: Optional[dict] = None
    while time.monotonic() < deadline:
        try:
            status = _get("/status")
        except RuntimeError:
            status = None
        if status is not None and status.get("scenario", {}).get("id") == id:
            return status
        last = status
        time.sleep(0.2)
    raise RuntimeError(
        f"load_scenario({id!r}) did not complete within {timeout_s}s (last status: {last})"
    )


# --- behaviour -------------------------------------------------------------


@mcp.tool()
def list_behaviour_graphs() -> list:
    """Every authored behaviour graph currently loaded, across all three
    domains: id, name, which kind of agent it is for ("household", "person"
    or "suppression_unit"), description and node count. Use with
    list_behaviour_profiles to see which graph a given profile runs."""
    return _get("/behaviour/graphs")


@mcp.tool()
def list_behaviour_profiles() -> list:
    """Every authored agent subtype (profile) currently loaded: id, name,
    description, which graph it runs, its domain, and the number that decides
    whether it is in play -- `share` of the population for households and
    separated people (0 means authored but not assigned to anyone), `enabled`
    plus which unit kinds it governs for suppression units. Change one with
    set_profile_share / set_profile_enabled."""
    return _get("/behaviour/profiles")


@mcp.tool()
def set_profile_share(id: str, share: float) -> dict:
    """Change what fraction of the population a household or separated-person
    profile is assigned (before normalisation across all profiles sharing a
    domain; see list_behaviour_profiles for current shares). Rebuilds the
    agent model and restarts the incident on the same fire, weather, seed and
    ignition list -- a controlled comparison, not a hot swap. Fails without
    changing anything if this would leave a domain with no runnable policy
    (e.g. zeroing the last household profile's share)."""
    return _post("/control/set_profile", {"id": id, "share": share})


@mcp.tool()
def set_profile_enabled(id: str, enabled: bool) -> dict:
    """Turn a suppression-unit profile on or off (households and separated
    people use `share` for the same purpose -- see set_profile_share).
    Restarts the incident the same way set_profile_share does."""
    return _post("/control/set_profile", {"id": id, "enabled": enabled})


@mcp.tool()
def reload_behaviour_library(path: Optional[str] = None) -> dict:
    """Re-read the behaviour library from disk and adopt it -- the composer's
    "Reload" followed by "Apply and restart", with no editor window needed.
    Use after hand-editing a file under data/behaviours/ or regenerating it
    (`cargo test -p behavior -- --ignored write_shipped_library`). path
    defaults to data/behaviours (or $SPOTORNO_DATA/behaviours). Lenient per
    file: one malformed graph or profile is reported in `file_errors` rather
    than failing the whole reload, unless every file fails."""
    body = {"path": path} if path is not None else {}
    return _post("/control/reload_behaviour", body)


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


@mcp.tool()
def take_screenshot(
    path: str,
    focus_x: Optional[float] = None,
    focus_y: Optional[float] = None,
    distance_m: Optional[float] = None,
    yaw_deg: Optional[float] = None,
    pitch_deg: Optional[float] = None,
    layer: Optional[str] = None,
    wait_s: float = 1.0,
) -> dict:
    """Capture the current 3D view to a PNG on disk -- the only way to
    actually *see* the incident (buildings burning or not, where the fire
    front is, whether an overlay renders sensibly) without a human at the
    keyboard. Read the resulting file with a file-reading tool afterwards;
    this call only writes it.

    focus_x/focus_y move the camera to look at a point in world metres, the
    same scenario-frame coordinates every other tool here uses (see
    place_ignition, close_road); distance_m is the orbit distance in metres;
    yaw_deg/pitch_deg the orbit angle. layer switches the fire overlay --
    one of "flames" (default), "intensity", "arrival", or "spread risk" (see
    get_status -> fire for the numbers each one visualises). Any of these
    left out keeps the camera/layer wherever it currently is.

    path must be somewhere the game process itself can write to (it is a
    native app, not sandboxed the way this MCP server might be). wait_s is
    how long to sleep before returning, since the game takes a short settle
    window plus a render-thread round trip to actually write the file --
    raise it if the read that follows still 404s."""
    body: dict = {"path": path}
    for k, v in (
        ("focus_x", focus_x),
        ("focus_y", focus_y),
        ("distance_m", distance_m),
        ("yaw_deg", yaw_deg),
        ("pitch_deg", pitch_deg),
        ("layer", layer),
    ):
        if v is not None:
            body[k] = v
    result = _post("/control/screenshot", body)
    time.sleep(wait_s)
    return result


@mcp.tool()
def close_road(x: float, y: float, radius_m: float = 300.0, minutes: float = 30.0) -> dict:
    """Close every drivable road link within radius_m of a world position
    (metres, scenario frame) for `minutes` simulated minutes -- a commander's
    order that binds civilian traffic (routing reroutes around it, or a
    household abandons a car caught on it) and nothing else: it does not stop
    the fire. Zero links closed is not an error and is worth checking -- it
    means the point was over open ground, not on the road network."""
    return _post(
        "/control/close_road",
        {"x": x, "y": y, "radius_m": radius_m, "minutes": minutes},
    )


@mcp.tool()
def reopen_roads() -> dict:
    """Lift every road closure currently in effect, immediately."""
    return _post("/control/reopen_roads", {})


@mcp.tool()
def request_boat_lift(minutes: float = 30.0, rate_per_min: float = 3.5) -> dict:
    """Request a maritime lift: capacity the road network does not have,
    arriving late (see get_status -> incident.boat_lift_min for how long
    until it is on station) and running for `minutes` simulated minutes at
    `rate_per_min` people picked up per minute from the shore. Fails if a
    lift is already active, or if this scenario's window has no shore havens
    at all (see list_behaviour_profiles for the separated-person `to-the-water`
    profile this pairs with)."""
    return _post("/control/boat_lift", {"minutes": minutes, "rate_per_min": rate_per_min})


if __name__ == "__main__":
    mcp.run()
