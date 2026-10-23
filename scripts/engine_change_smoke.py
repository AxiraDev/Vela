#!/usr/bin/env python3
"""Exercise Intel/native engine changes in a NEW disposable Vela library."""
import argparse
import json
import os
from pathlib import Path
import shutil
import subprocess
import time

p = argparse.ArgumentParser(description=__doc__)
p.add_argument('--intel', required=True, type=Path)
p.add_argument('--native', required=True, type=Path)
p.add_argument('--probe-x64', required=True, type=Path)
p.add_argument('--home', required=True, type=Path)
p.add_argument('--core', type=Path, default=Path(__file__).resolve().parents[1] / 'Vela.app/Contents/MacOS/vela-core')
a = p.parse_args()
root = a.home.resolve()
if root.exists():
    p.error('Use a new disposable --home; existing libraries are not modified.')
env = dict(os.environ, VELA_HOME=str(root), VELA_LANGUAGE='en')

def call(op, **fields):
    process = subprocess.run([str(a.core.resolve())], input=json.dumps(dict(op=op, **fields)) + '\n',
                             text=True, capture_output=True, env=env, timeout=1200)
    reply = json.loads(process.stdout)
    if not reply['ok']:
        raise RuntimeError(reply['error'])
    return reply['data']

def bottle():
    return call('list')['state']['bottles'][0]

def wait_marker(marker):
    deadline = time.monotonic() + 30
    while time.monotonic() < deadline:
        text = call('read_log', id=bid)['text']
        if marker in text:
            return
        time.sleep(0.25)
    raise RuntimeError(f'Missing marker {marker}:\n{text}')

intel = call('add_engine', path=str(a.intel.absolute()), name='Intel test engine')
native = call('add_engine', path=str(a.native.absolute()), name='Native test engine')
created = call('create', name='Disposable engine change test', engine_id=intel['id'], dpi=192)
bid = created['id']
prefix = root / 'prefixes' / bid
try:
    sentinel = prefix / 'drive_c' / 'save.dat'
    sentinel.write_text('before change')
    outside = root / 'host-sentinel.txt'
    outside.write_text('host file')
    (prefix / 'drive_c' / 'host-link').symlink_to(outside)
    probe = prefix / 'drive_c' / 'VelaProbe.exe'
    shutil.copyfile(a.probe_x64, probe)
    shortcut = call('add_shortcut', id=bid, name='Original x64 app', path=str(probe), arguments=[])
    call('save_settings', id=bid, name='Disposable engine change test', environment={'MY_APP_SETTING': 'keep'})
    call('change_engine', id=bid, engine_id=native['id'])
    b = bottle()
    assert b['id'] == bid and b['engine_id'] == native['id'] and b['status'] == 'ready'
    assert b['dpi'] == 192 and b['environment']['MY_APP_SETTING'] == 'keep'
    assert '"LogPixels"=dword:000000c0' in (prefix / 'user.reg').read_text(errors='replace')
    assert b['shortcuts'][0]['id'] == shortcut['id'] and sentinel.read_text() == 'before change'
    call('run_shortcut', id=bid, shortcut_id=shortcut['id'])
    wait_marker('VELA_WINDOWS_X64_OK')
    print('PASS Intel -> native: original x64 app, prefix ID, data, shortcuts, variables and DPI retained', flush=True)
    call('change_engine', id=bid, engine_id=intel['id'])
    assert bottle()['engine_id'] == intel['id']
    assert '"LogPixels"=dword:000000c0' in (prefix / 'user.reg').read_text(errors='replace')
    call('run_shortcut', id=bid, shortcut_id=shortcut['id'])
    wait_marker('VELA_WINDOWS_X64_OK')
    cmd = call('add_shortcut', id=bid, name='32-bit check', path=str(prefix / 'drive_c/windows/syswow64/cmd.exe'),
               arguments=['/d', '/c', 'echo VELA_SWITCH_WIN32_OK'])
    call('run_shortcut', id=bid, shortcut_id=cmd['id'])
    wait_marker('VELA_SWITCH_WIN32_OK')
    print('PASS native -> Intel: original x64 app and 32-bit Windows command execution', flush=True)
    sentinel.write_text('after change')
    call('restore_engine_backup', id=bid)
    b = bottle()
    assert b['engine_id'] == native['id'] and b['status'] == 'ready' and b['pending_engine'] is None
    assert sentinel.read_text() == 'before change'
    assert len(b['shortcuts']) == 1 and b['shortcuts'][0]['id'] == shortcut['id']
    assert outside.read_text() == 'host file'
    backup = root / 'engine-backups' / bid / b['engine_backup']['id']
    assert any((item / 'drive_c/save.dat').read_text() == 'after change' for item in backup.glob('replaced-*'))
    call('run_shortcut', id=bid, shortcut_id=shortcut['id'])
    wait_marker('VELA_WINDOWS_X64_OK')
    print('PASS backup restore: previous engine and prefix state restored; post-switch files retained separately', flush=True)
finally:
    call('stop', id=bid)
    print('PASS scoped stop of the disposable test environment', flush=True)
