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
    parser.add_argument('--browser', choices=['chromium', 'webkit'], default='chromium')
    parser.add_argument('--smoke', action='store_true', help='Test native taps through title, name and tutorial, in both orientations')
    parser.add_argument('--edge-pan', action='store_true', help='Test dragging at each map edge, stopping and cancellation')
    parser.add_argument('--output', type=Path, default=ROOT / 'out/playtest/mobile')
    args = parser.parse_args()
    if args.browser == 'webkit' and not args.smoke:
        parser.error('WebKit supports --smoke; full gestures use Chromium CDP')
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
            browser = getattr(pw, args.browser).launch(headless=True, executable_path=args.browser_executable, args=gpu if args.browser == 'chromium' else [])
            page = browser.new_page(viewport={'width': 390, 'height': 844}, device_scale_factor=3, is_mobile=True, has_touch=True)
            page.set_default_timeout(120000)
            page.on('pageerror', lambda e: errors.append(str(e)) if 'Using exceptions for control flow' not in str(e) else None)
            page.on('console', lambda m: errors.append(m.text) if m.type == 'error' else None)
            cdp = page.context.new_cdp_session(page) if args.browser == 'chromium' else None
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
                if args.smoke:
                    # Native taps include browser event ordering and no artificial
                    # frame delay between press/release (especially on Safari).
                    page.touchscreen.tap(c['x'], c['y']); frames()
                else:
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
                # The tutorial reserves part of the camera viewport for its
                # sidebar; check the entire canvas backing buffer instead.
                size = page.locator('#rocca').evaluate('c => [c.width, c.height]')
                assert size == [w, h], size
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
                if args.smoke:
                    tap('Avanti'); assert state()['step'] == 1
                    rotate(844, 390); snap('04-tutorial-landscape')
                    tap('Avanti'); assert state()['step'] == 2
                    assert not errors, errors
                    passed = True
                    return
                tap('Avanti'); tap('Avanti'); frames(120)
                target = state()['tutorial_target']; assert target
                team = state()['teams'][target['team']]['name']
                if args.edge_pan:
                    c = control(team)
                    touch('touchStart', [(0, c['x'], c['y'])])
                    touch('touchMove', [(0, c['x'] + 14, c['y'] - 14)])
                    center = (195, 350)
                    for x, y, axis, sign in [(382, 350, 0, 1), (8, 350, 0, -1), (195, 124, 1, -1), (195, 590, 1, 1)]:
                        touch('touchMove', [(0, *center)]); frames(8)
                        before = state()['camera']['focus']
                        touch('touchMove', [(0, x, y)])
                        page.wait_for_timeout(250); frames(4)
                        after = state()['camera']['focus']
                        assert (after[axis] - before[axis]) * sign > 10, (before, after, x, y)
                    snap('edge-pan-held')
                    # A stationary finger in the center must stop the motion.
                    touch('touchMove', [(0, *center)]); frames(8)
                    before = state()['camera']['focus']; frames(20)
                    assert sum(abs(a-b) for a,b in zip(before, state()['camera']['focus'])) < 1
                    # Panels do not pan, and cancel must leave no queued motion/order.
                    touch('touchMove', [(0, 100, 40)]); frames(8)
                    before = state()['camera']['focus']; frames(20)
                    assert sum(abs(a-b) for a,b in zip(before, state()['camera']['focus'])) < 1
                    touch('touchCancel', []); frames(20)
                    assert sum(abs(a-b) for a,b in zip(before, state()['camera']['focus'])) < 1
                    assert state()['step'] == 2 and state()['teams'][target['team']]['hex'] is None
                    snap('edge-pan-canceled')
                    c = control(team)
                    touch('touchStart', [(0, c['x'], c['y'])])
                    touch('touchMove', [(0, c['x'] + 14, c['y'] - 14)])
                    touch('touchMove', [(0, 382, 350)])
                    page.wait_for_timeout(150); frames(4)
                    # Release on the HUD cancels the order and stops panning.
                    touch('touchMove', [(0, 100, 40)]); touch('touchEnd', [])
                    before = state()['camera']['focus']; frames(20)
                    assert sum(abs(a-b) for a,b in zip(before, state()['camera']['focus'])) < 1
                    assert state()['step'] == 2 and state()['teams'][target['team']]['hex'] is None
                    assert not errors, errors
                    passed = True
                    return
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
                pinch_before = state()['camera']['distance']
                touch('touchStart', [(0, 80, 420), (1, 260, 420)])
                touch('touchMove', [(0, 60, 430), (1, 290, 400)])
                touch('touchEnd', [])
                assert state()['camera']['distance'] < pinch_before * 0.9
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
                tap('Inizia'); frames(90)
                rotate(568, 320); frames(90); snap('16-small-landscape')
                tap('Avanti'); assert state()['step'] == 1
                # Also smoke-test mouse input on a desktop Retina viewport.
                page.set_viewport_size({'width': 1280, 'height': 800}); frames(12)
                c = control('Avanti'); page.mouse.click(c['x'], c['y']); frames()
                assert state()['step'] == 2
                snap('17-desktop-mouse')
                assert not errors, errors
                assert max(s['memory_bytes'] for s in samples) <= 320 * 1048576, samples
                passed = True
            finally:
                if not passed:
                    try: snap('failure')
                    except Exception: pass
                browser.close()
    finally:
        server.shutdown()
        (args.output / 'report.json').write_text(json.dumps({'browser': args.browser, 'smoke': args.smoke, 'passed': passed, 'seconds': time.monotonic()-started, 'errors': errors, 'samples': samples, 'actions': actions}, ensure_ascii=False, indent=2))

if __name__ == '__main__': main()
