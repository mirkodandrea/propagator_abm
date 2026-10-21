#!/usr/bin/env python3
"""Measure Wasm memory through a game and restart, for each web graphics profile.

Build with scripts/build_web.sh first. Reports linear memory capacity, excluding
JS, browser and GPU allocations. Uses fresh browser contexts and real UI clicks.
"""
import argparse
import functools
import http.server
import json
from pathlib import Path
import sys
import threading

from playwright.sync_api import sync_playwright

ROOT = Path(__file__).resolve().parents[1]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--site-dir', type=Path, default=ROOT / 'target/web')
    parser.add_argument('--output', type=Path, default=ROOT / 'out/playtest/memory')
    parser.add_argument('--browser-executable')
    args = parser.parse_args()
    args.output.mkdir(parents=True, exist_ok=True)

    class Quiet(http.server.SimpleHTTPRequestHandler):
        def log_message(self, *_):
            pass

    server = http.server.ThreadingHTTPServer(
        ('127.0.0.1', 0), functools.partial(Quiet, directory=str(args.site_dir.resolve())))
    threading.Thread(target=server.serve_forever, daemon=True).start()
    results = []
    try:
        with sync_playwright() as pw:
            gpu = ['--enable-gpu'] + (['--use-angle=metal'] if sys.platform == 'darwin' else [])
            browser = pw.chromium.launch(headless=True, executable_path=args.browser_executable, args=gpu)
            try:
                for profile in ('bassa', 'media', 'auto'):
                    # Fresh context: no saved adaptive rung or previous heap.
                    context = browser.new_context(viewport={'width': 1280, 'height': 800}, locale='it-IT')
                    page = context.new_page()
                    page.set_default_timeout(180000)
                    errors = []
                    page.on('pageerror', lambda e: errors.append(str(e)) if 'Using exceptions for control flow' not in str(e) else None)
                    page.on('console', lambda m: errors.append(m.text) if m.type == 'error' else None)

                    def state():
                        return page.evaluate('window.roccaPlaytest.state')

                    def frames(n=6):
                        frame = page.evaluate('window.roccaPlaytest.frame')
                        page.wait_for_function('f => window.roccaPlaytest.frame > f', arg=frame + n)

                    def click(label):
                        frames()
                        c = next(c for c in state()['controls'] if c['label'] == label and c['enabled'])
                        page.mouse.click(c['x'], c['y'])
                        frames()

                    def memory():
                        return state()['wasm_memory_bytes'] / 1048576

                    suffix = f'&q={profile}' if profile != 'auto' else ''
                    page.goto(f'http://127.0.0.1:{server.server_port}/?playtest=1{suffix}')
                    page.wait_for_function('window.roccaPlaytest?.state && !document.querySelector("#loading")')
                    frames(30)
                    result = {'profile': profile, 'wasm_mib': memory(), 'errors': errors}
                    page.screenshot(path=str(args.output / f'{profile}.png'))
                    click('Gioca')
                    click('Nome')
                    page.keyboard.type('Memoria')
                    click('Inizia')
                    click('Parti')
                    frames(30)
                    result.update(case=state()['case'], game_start_mib=memory())
                    if any(c['label'] == 'Riprendi' and c['enabled'] for c in state()['controls']):
                        click('Riprendi')
                    for _ in range(40):
                        if state()['phase'] == 'Fine':
                            break
                        page.evaluate('window.roccaPlaytest.advance_s = 600')
                        page.wait_for_function('window.roccaPlaytest.advance_s === 0')
                        frames()
                    assert state()['phase'] == 'Fine', state()
                    page.wait_for_function('window.roccaPlaytest.state.baseline_ready')
                    frames()
                    result['end_mib'] = memory()
                    page.screenshot(path=str(args.output / f'{profile}-end.png'))
                    click('Nuova partita')
                    frames()
                    result['restart_mib'] = memory()
                    assert state()['phase'] == 'Intro', state()
                    assert not errors, errors
                    results.append(result)
                    (args.output / 'memory.json').write_text(json.dumps(results, indent=2))
                    print(json.dumps(result), flush=True)
                    context.close()
            finally:
                browser.close()
    finally:
        server.shutdown()
        server.server_close()


if __name__ == '__main__':
    main()
