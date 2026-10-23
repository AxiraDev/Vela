#!/usr/bin/env python3
"""Exercise a genuine x64 PE through Vela and check the macOS translation flag.
Use a NEW, disposable --home. No existing user prefixes are modified.
"""
import argparse
import json
import os
from pathlib import Path
import subprocess
import time

p = argparse.ArgumentParser(description=__doc__)
for name in ['core', 'engine', 'home', 'x64', 'x86', 'translation-probe']:
    p.add_argument('--' + name, required=True, type=Path)
a = p.parse_args()
if a.home.exists():
    raise SystemExit('Use a new disposable --home directory')
env = dict(os.environ, VELA_HOME=str(a.home.resolve()))

def call(**request):
    result = subprocess.run([str(a.core.resolve())], input=json.dumps(request) + '\n',
                            text=True, capture_output=True, env=env, timeout=180)
    reply = json.loads(result.stdout)
    if not reply['ok']:
        raise RuntimeError(reply['error'])
    return reply['data']

e = call(op='add_engine', path=str(a.engine.resolve()), name='Native test')
assert e['architecture'] == 'arm64' and e['backend'] == 'arm64-fex', e
assert e['supports_x86_32'] is False, 'This test expects the unrestricted 64-bit runtime'
print('PASS ARM64 loader, server and FEX registration', flush=True)
b = call(op='create', name='Native test', engine_id=e['id'], dpi=192)
print('PASS native wineboot', flush=True)
try:
    state = call(op='list')['state']
    assert state['bottles'][0]['dpi'] == 192, state
    run = call(op='launch', id=b['id'], path=str(a.home.resolve() / 'prefixes' / b['id'] / 'drive_c/windows/system32/reg.exe'),
               arguments=['query', 'HKCU\\Control Panel\\Desktop', '/v', 'LogPixels'])
    deadline = time.monotonic() + 30
    while time.monotonic() < deadline:
        text = Path(run['log']).read_text(errors='replace')
        if '0xc0' in text.lower():
            print('PASS real Windows registry: LogPixels=192 (200%)', flush=True)
            break
        time.sleep(0.2)
    else:
        raise RuntimeError('DPI registry value was not applied: ' + text[-4000:])
    run = call(op='launch', id=b['id'], path=str(a.x64.resolve()))
    deadline = time.monotonic() + 60
    while time.monotonic() < deadline:
        text = Path(run['log']).read_text(errors='replace')
        if 'VELA_WINDOWS_X64_OK' in text:
            print('PASS real AMD64 PE: VELA_WINDOWS_X64_OK', flush=True)
            check = subprocess.run([str(a.translation_probe.resolve()), str(run['pid'])],
                                   text=True, capture_output=True, check=True)
            status = json.loads(check.stdout)
            assert status['translated'] is False, status
            print('PASS macOS P_TRANSLATED=false:', check.stdout.strip(), flush=True)
            break
        time.sleep(0.2)
    else:
        raise RuntimeError('x64 fixture did not finish successfully: ' + text[-4000:])
    try:
        call(op='launch', id=b['id'], path=str(a.x86.resolve()))
    except RuntimeError as error:
        assert '32-битная' in str(error), error
        print('PASS unsupported x86 rejected before launch', flush=True)
    else:
        raise AssertionError('32-bit launch should be rejected by this runtime')
finally:
    call(op='stop', id=b['id'])
    print('PASS scoped native wineserver stop', flush=True)
