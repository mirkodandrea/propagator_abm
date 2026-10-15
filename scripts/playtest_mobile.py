#!/usr/bin/env python3
"""Touch regression: mobile DPR, portrait/landscape, tutorial, orders and restart.
Build with scripts/build_web.sh first. Uses an isolated browser profile.
"""
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


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--browser-executable')
    parser.add_argument('--output', type=Path, default=ROOT / 'out/playtest/mobile')
    args = parser.parse_args()
    args.output.mkdir(parents=True, exist_ok=True)
    class Quiet(http.server.SimpleHTTPRequestHandler):
        def log_message(self, *_): pass
    server = http.server.ThreadingHTTPServer(('127.0.0.1', 0), functools.partial(Quiet, directory=str(ROOT / 'target/web')))
    threading.Thread(target=server.serve_forever, daemon=True).start()
    errors, samples, actions = [], [], []
    started = time.monotonic()
    passed = False
    try:
        with sync_playwright() as pw:
            gpu = ['--enable-gpu'] + (['--use-angle=metal'] if sys.platform == 'darwin' else [])
            browser = pw.chromium.launch(headless=True, executable_path=args.browser_executable, args=gpu)
            page = browser.new_page(viewport={'width': 390, 'height': 844}, device_scale_factor=3, is_mobile=True, has_touch=True)
            page.set_default_timeout(120000)
            page.on('pageerror', lambda e: errors.append(str(e)) if 'Using exceptions for control flow' not in str(e) else None)
            page.on('console', lambda m: errors.append(m.text) if m.type == 'error' else None)
            cdp = page.context.new_cdp_session(page)
            def state(): return page.evaluate('window.roccaPlaytest.state')
            def frames(n=4):
                frame = page.evaluate('window.roccaPlaytest.frame')
                page.wait_for_function('f => window.roccaPlaytest.frame >= f', arg=frame + n)
            def control(label):
                frames()
                matches = [c for c in state()['controls'] if c['label'] == label and c['enabled']]
                assert matches, f'Missing control: {label}'
                c = matches[0]
                w, h = page.viewport_size.values()
                assert 0 <= c['x'] <= w and 0 <= c['y'] <= h, (label, c)
                return c
            def touch(kind, points):
                cdp.send('Input.dispatchTouchEvent', {'type': kind, 'touchPoints': [dict(x=x, y=y, id=i, radiusX=5, radiusY=5, force=1) for i, x, y in points]})
                frames(2)
            def tap(label):
                c = control(label)
                touch('touchStart', [(0, c['x'], c['y'])]); touch('touchEnd', [])
                actions.append(label)
            def drag(start, end, screenshot=None):
                touch('touchStart', [(0, *start)])
                for i in range(1, 13):
                    p = [start[j] + (end[j] - start[j]) * i / 12 for j in range(2)]
                    touch('touchMove', [(0, *p)])
                if screenshot: snap(screenshot)
                touch('touchEnd', [])
            def snap(name):
                frames()
                s = state(); samples.append({'name': name, 'memory_bytes': s['wasm_memory_bytes'], 'phase': s['phase']})
                page.screenshot(path=str(args.output / f'{name}.png'), scale='css')
                (args.output / f'{name}.json').write_text(json.dumps(s, ensure_ascii=False, indent=2))
                print(name, s['phase'], s['expect'], round(s['wasm_memory_bytes'] / 1048576, 1), 'MiB', flush=True)
            def rotate(w, h):
                page.set_viewport_size({'width': w, 'height': h}); frames(12)
            def advance(seconds):
                page.evaluate('s => window.roccaPlaytest.advance_s = s', seconds)
                page.wait_for_function('window.roccaPlaytest.advance_s === 0'); frames()
            try:
                page.goto(f'http://127.0.0.1:{server.server_port}/?playtest=1')
                page.wait_for_function('window.roccaPlaytest?.state && !document.querySelector("#loading")')
                frames(); assert state()['camera']['pixels'] == [390, 844]
                snap('01-title-portrait')
                tap('Gioca'); tap('Nome')
                page.locator('#player-name').fill('Prova telefono')
                page.get_by_role('button', name='Continua', exact=True).click(); frames()
                assert state()['player'] == 'Prova telefono'
                snap('02-intro-portrait')
                tap('Inizia'); frames(90); snap('03-tutorial-portrait')
                tap('Avanti'); tap('Avanti'); frames(120)
                target = state()['tutorial_target']; assert target
                team = state()['teams'][target['team']]['name']
                # Tap-select then tap the ground: this never depended on a mouse.
                tap(team)
                target = state()['tutorial_target']
                touch('touchStart', [(0, target['x'], target['y'])]); touch('touchEnd', [])
                assert state()['step'] == 3, state()
                rotate(844, 390); frames(120); snap('04-tutorial-landscape')
                target = state()['tutorial_target']; team = state()['teams'][target['team']]['name']
                c = control(team)
                # Releasing over the HUD cancels; the tutorial must stay put.
                drag((c['x'], c['y']), (100, 40))
                assert state()['step'] == 3 and state()['teams'][target['team']]['hex'] is None
                snap('04b-canceled-drop')
                c = control(team)
                touch('touchStart', [(0, c['x'], c['y'])])
                touch('touchMove', [(0, c['x'] + 14, c['y'] - 14)])
                touch('touchCancel', [])
                assert state()['step'] == 3 and state()['teams'][target['team']]['hex'] is None
                c = control(team)
                target = state()['tutorial_target']
                before = state()['camera']['focus']
                drag((c['x'], c['y']), (target['x'], target['y']), '05-drag-preview')
                assert state()['step'] == 4, state()
                assert state()['teams'][target['team']]['hex'] is not None
                # A fresh camera flight can start after the valid drop, so
                # ownership is checked in the saved in-drag observation.
                in_drag = json.loads((args.output / '05-drag-preview.json').read_text())
                assert sum(abs(a-b) for a,b in zip(before,in_drag['camera']['focus'])) < 10
                tap('Riprendi'); advance(1800); frames(120)
                snap('06-evacuation-instruction')
                tap('Le Ghiande'); snap('07-evacuation-card'); tap('Evacua')
                assert state()['step'] == 7, state()
                tap('Riprendi')
                # Keep each batch short enough to stop at the tutorial boundary.
                for _ in range(40):
                    if state()['phase'] != 'Tutorial': break
                    advance(120)
                assert state()['phase'] == 'Giorno2', state()
                snap('08-day-two'); tap('Inizia giorno 2'); frames(40)
                rotate(390, 844); snap('09-game-portrait')
                # Zoom, two-finger pan/rotation, and cancellation must not issue orders.
                teams = state()['teams']
                dist = state()['camera']['distance']; tap('+')
                assert state()['camera']['distance'] < dist
                touch('touchStart', [(0, 80, 420), (1, 260, 420)])
                touch('touchMove', [(0, 60, 430), (1, 290, 400)])
                touch('touchEnd', [])
                assert [t['hex'] for t in state()['teams']] == [t['hex'] for t in teams]
                tap('Isola'); frames(10)
                snap('10-game-home')
                tap('Riprendi'); advance(18000)
                assert state()['phase'] == 'Fine', state()
                page.wait_for_function('window.roccaPlaytest.state.baseline_ready')
                snap('11-end-portrait')
                rotate(844, 390); snap('12-end-landscape')
                tap('Nuova partita'); snap('13-restart')
                assert state()['phase'] == 'Intro'
                rotate(320, 568); snap('14-small-phone'); tap('Gioca'); snap('15-small-intro')
                assert not errors, errors
                passed = True
            finally:
                if not passed:
                    try: snap('failure')
                    except Exception: pass
                browser.close()
    finally:
        server.shutdown()
        (args.output / 'report.json').write_text(json.dumps({'passed': passed, 'seconds': time.monotonic()-started, 'errors': errors, 'samples': samples, 'actions': actions}, ensure_ascii=False, indent=2))

if __name__ == '__main__': main()
