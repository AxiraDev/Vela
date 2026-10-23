#!/usr/bin/env python3
"""Check stop, recoverable deletion, restore, and purge in a NEW test library."""
import argparse
import json
import os
from pathlib import Path
import subprocess

p = argparse.ArgumentParser(description=__doc__)
p.add_argument('--engine', required=True, type=Path)
p.add_argument('--home', required=True, type=Path)
p.add_argument('--core', type=Path, default=Path(__file__).resolve().parents[1] / 'Vela.app/Contents/MacOS/vela-core')
a = p.parse_args()
root = a.home.resolve()
if root.exists():
    p.error('Use a new disposable --home; existing libraries are not modified.')
env = dict(os.environ, VELA_HOME=str(root), VELA_LANGUAGE='en')

def call(op, **fields):
    process = subprocess.run([str(a.core.resolve())], input=json.dumps(dict(op=op, **fields)) + '\n',
                             text=True, capture_output=True, env=env, timeout=180)
    reply = json.loads(process.stdout)
    if not reply['ok']:
        raise RuntimeError(reply['error'])
    return reply['data']

engine = call('add_engine', path=str(a.engine.absolute()), name='Removal smoke test')
bottle = call('create', name='Disposable deletion test', engine_id=engine['id'], dpi=192)
bid = bottle['id']
prefix = root / 'prefixes' / bid
sentinel = prefix / 'drive_c' / 'saved-data.txt'
sentinel.write_text('restore me')
outside = root / 'host-sentinel.txt'
outside.write_text('keep me')
(prefix / 'drive_c' / 'host-link').symlink_to(outside)
call('delete_environment', id=bid)
state = call('list')['state']
assert not state['bottles'] and state['deleted_bottles'][0]['id'] == bid
assert sentinel.read_text() == 'restore me'
print('PASS scoped Wine stop and recoverable environment deletion', flush=True)
call('restore_environment', id=bid)
state = call('list')['state']
assert state['bottles'][0]['id'] == bid and not state['deleted_bottles']
assert sentinel.read_text() == 'restore me'
print('PASS restore preserves Windows files, ID and settings', flush=True)
call('delete_environment', id=bid)
call('purge_environment', id=bid)
assert not prefix.exists() and outside.read_text() == 'keep me'
assert not call('list')['state']['deleted_bottles']
print('PASS permanent removal preserves files outside the environment', flush=True)
