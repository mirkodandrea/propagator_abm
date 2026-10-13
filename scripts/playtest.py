#!/usr/bin/env python3
"""Headless real-UI playtest, or a persistent JSON-lines session for an agent."""
import argparse
import functools
import http.server
import json
from pathlib import Path
import sys
import threading
import time

from playwright.sync_api import sync_playwright

ROOT = Path(__file__).resolve().parents[1]


class Playtest:
    def __init__(self, page, output):
        self.page = page
        self.output = output
        self.events = []
        self.errors = []
        self.log = output / "browser.log"
        self.log.write_text("")
        def log(kind, message):
            with self.log.open("a") as stream:
                stream.write(f"{kind}: {message}\n")
            if kind == "error":
                self.errors.append(message)
        page.on("pageerror", lambda error: log("error", str(error)))
        page.on("console", lambda msg: log(msg.type, msg.text))

    def frames(self, count=3):
        frame = self.page.evaluate("window.roccaPlaytest.frame")
        self.page.wait_for_function("n => window.roccaPlaytest.frame >= n", arg=frame + count)

    def state(self):
        state = self.page.evaluate("window.roccaPlaytest.state")
        if state["error"]:
            raise RuntimeError(state["error"])
        return state

    def click(self, label=None, x=None, y=None, index=0):
        self.frames()
        if label is not None:
            matches = [c for c in self.state()["controls"] if c["label"] == label and c["enabled"]]
            if not matches or index >= len(matches):
                raise ValueError(f"No enabled control {label!r} at index {index}; see state.controls")
            x, y = matches[index]["x"], matches[index]["y"]
        self.page.mouse.move(x, y)
        self.frames(1)
        self.page.mouse.click(x, y)
        self.frames()

    def advance(self, seconds=10800):
        state = self.state()
        if state["phase"] != "Esegui" or state["confirm_evac"] is not None or state["tour"] is not None:
            raise ValueError("Advance requires running gameplay with no pending decision; use UI or elapse")
        self.page.evaluate("s => window.roccaPlaytest.advance_s = s", seconds)
        self.page.wait_for_function("window.roccaPlaytest.advance_s === 0")
        self.frames()

    def screenshot(self, name):
        # Avoid mixing the previous phase's egui output/camera with this state.
        self.frames()
        path = self.output / name
        if path.suffix != ".png":
            path = path.with_suffix(".png")
        if not path.resolve().is_relative_to(self.output.resolve()):
            raise ValueError("Screenshot name must stay inside the output directory")
        path.parent.mkdir(parents=True, exist_ok=True)
        self.page.screenshot(path=str(path))
        path.with_suffix(".json").write_text(json.dumps(self.state(), ensure_ascii=False, indent=2))
        return str(path)

    def command(self, command):
        op = command["op"]
        result = None
        if op == "click":
            self.click(**{k: v for k, v in command.items() if k != "op"})
        elif op == "type":
            self.page.keyboard.type(command["text"])
            self.frames()
        elif op == "key":
            self.page.keyboard.press(command["key"])
            self.frames()
        elif op == "advance":
            self.advance(command.get("seconds", 10800))
        elif op == "elapse":
            self.page.evaluate("s => window.roccaPlaytest.elapse_s = s", command["seconds"])
            self.frames()
        elif op == "drag":
            self.page.mouse.move(*command["from"])
            self.page.mouse.down()
            self.frames(2)
            self.page.mouse.move(*command["to"], steps=command.get("steps", 12))
            self.frames(2)
            self.page.mouse.up()
            self.frames()
        elif op == "wheel":
            self.page.mouse.wheel(command.get("x", 0), command["y"])
            self.frames()
        elif op == "screenshot":
            result = self.screenshot(command["name"])
        elif op != "state":
            raise ValueError(f"Unknown operation {op!r}")
        state = self.state()
        self.events.append({"command": command, "state": state, "result": result})
        return {"state": state, "result": result}

    def smoke(self, level, player):
        self.screenshot("00-title")
        self.command({"op": "click", "label": "Gioca"})
        self.command({"op": "type", "text": player})
        self.screenshot("01-levels")
        self.command({"op": "click", "label": level})
        if self.state()["tour"] is not None:
            self.command({"op": "key", "key": "Escape"})
        self.screenshot("02-plan")
        # Exercise a civil order through the actual district card.
        district = self.state()["districts"][0]["name"]
        self.command({"op": "click", "label": f"Mappa:{district}"})
        self.command({"op": "click", "label": "Preallerta"})
        if self.state()["phase"] == "Pianifica":
            self.command({"op": "click", "label": "Avvia"})
        for i in range(50):
            state = self.state()
            if state["phase"] == "Fine":
                break
            if state["phase"] == "Crisi":
                self.screenshot(f"crisis-{i:02}")
                labels = {c["label"] for c in state["controls"] if c["enabled"]}
                self.command({"op": "click", "label": "Conferma" if "Conferma" in labels else "Continua"})
            elif state["phase"] == "Esegui":
                self.command({"op": "advance"})
            else:
                raise AssertionError(f"Unexpected phase: {state['phase']}")
        else:
            raise AssertionError("Game did not finish within 50 decisions")
        self.page.wait_for_function("window.roccaPlaytest.state.baseline_ready")
        self.frames()
        final = self.state()
        assert final["time_s"] >= final["end_s"]
        self.screenshot("99-end")
        self.command({"op": "click", "label": "Riprova"})
        assert self.state()["phase"] == "intro", "Retry must reopen level selection"
        self.screenshot("100-retry")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--software-rendering", action="store_true", help="Use SwiftShader instead of headless GPU rendering")
    parser.add_argument("--browser-executable", help="Optional existing Chromium/headless-shell executable")
    parser.add_argument("--url", help="Existing web build URL; otherwise serves target/web on a random local port")
    parser.add_argument("--output", type=Path, default=ROOT / "out/playtest")
    parser.add_argument("--level", choices=["Facile", "Medio", "Esperto"], default="Facile")
    parser.add_argument("--player", default="Agente")
    parser.add_argument("--size", default="1600x1000")
    parser.add_argument("--timeout", type=float, default=180, help="Timeout per operation, in seconds")
    parser.add_argument("--interactive", action="store_true", help="Read JSON commands from stdin, return state JSON on stdout")
    parser.add_argument("--script", type=Path, help="JSON array of commands instead of the default end-to-end smoke")
    args = parser.parse_args()
    args.output.mkdir(parents=True, exist_ok=True)
    server = None
    if not args.url:
        if not (ROOT / "target/web/index.html").exists():
            parser.error("Run scripts/build_web.sh first")
        class QuietHandler(http.server.SimpleHTTPRequestHandler):
            def log_message(self, *_):
                pass
        handler = functools.partial(QuietHandler, directory=str(ROOT / "target/web"))
        server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), handler)
        threading.Thread(target=server.serve_forever, daemon=True).start()
        args.url = f"http://127.0.0.1:{server.server_port}/"
    started = time.monotonic()
    test = None
    passed = False
    try:
        with sync_playwright() as pw:
            # Headless Chromium has no desktop window and cannot take OS focus.
            gpu_args = ["--enable-unsafe-swiftshader"] if args.software_rendering else ["--enable-gpu"]
            if sys.platform == "darwin" and not args.software_rendering:
                gpu_args.append("--use-angle=metal")
            browser = pw.chromium.launch(headless=True, executable_path=args.browser_executable, args=gpu_args)
            width, height = map(int, args.size.split("x"))
            page = browser.new_page(viewport={"width": width, "height": height}, device_scale_factor=1)
            page.set_default_timeout(args.timeout * 1000)
            test = Playtest(page, args.output)
            sep = "&" if "?" in args.url else "?"
            try:
                page.goto(args.url + sep + "playtest=1", wait_until="domcontentloaded")
                page.wait_for_function("window.roccaPlaytest?.state != null && !document.querySelector('#loading')")
                test.frames()
                if args.interactive:
                    print(json.dumps({"ready": True, "state": test.state()}, ensure_ascii=False), flush=True)
                    for line in sys.stdin:
                        try:
                            cmd = json.loads(line)
                            if cmd["op"] == "quit":
                                break
                            print(json.dumps(test.command(cmd), ensure_ascii=False), flush=True)
                        except Exception as error:
                            print(json.dumps({"error": str(error)}, ensure_ascii=False), flush=True)
                elif args.script:
                    for cmd in json.loads(args.script.read_text()):
                        test.command(cmd)
                else:
                    test.smoke(args.level, args.player)
                if test.errors:
                    raise RuntimeError("Browser errors: " + "; ".join(test.errors))
                passed = True
            except Exception:
                try:
                    page.screenshot(path=str(args.output / "failure.png"), timeout=5000)
                    (args.output / "failure.json").write_text(json.dumps(test.state(), ensure_ascii=False, indent=2))
                except Exception:
                    pass
                raise
            finally:
                browser.close()
    finally:
        elapsed = time.monotonic() - started
        (args.output / "report.json").write_text(json.dumps({
            "passed": passed, "elapsed_s": elapsed, "browser_errors": test.errors if test else [],
            "events": test.events if test else [],
        }, ensure_ascii=False, indent=2))
        if server:
            server.shutdown()
    if not args.interactive:
        print(f"Playtest passed in {elapsed:.1f}s; screenshots and report: {args.output}")


if __name__ == "__main__":
    main()
