#!/usr/bin/env python3
"""Build tiny x86/x64 PE fixtures using clang and an LLVM COFF linker, without a Windows SDK."""
import argparse
from pathlib import Path
import subprocess

parser = argparse.ArgumentParser()
parser.add_argument('--linker', required=True, type=Path, help='lld-link or rust-lld')
parser.add_argument('--out', required=True, type=Path)
args = parser.parse_args()
args.out.mkdir(parents=True, exist_ok=True)
source = Path(__file__).resolve().parents[1] / 'tests/windows/probe.c'
definition = args.out / 'kernel32.def'
definition.write_text('LIBRARY KERNEL32.dll\nEXPORTS\nGetStdHandle\nWriteFile\nExitProcess\nSleep\n')
linker = [str(args.linker)]
if args.linker.name == 'rust-lld':
    linker += ['-flavor', 'link']
for machine, target in [('x64', 'x86_64-pc-windows-msvc'), ('x86', 'i686-pc-windows-msvc')]:
    library = args.out / f'kernel32-{machine}.lib'
    obj = args.out / f'probe-{machine}.obj'
    exe = args.out / f'probe-{machine}.exe'
    subprocess.run(linker + ['/lib', f'/def:{definition}', f'/machine:{machine}', f'/out:{library}'], check=True)
    subprocess.run(['/usr/bin/clang', '-target', target, '-O1', '-fno-stack-protector', '-fno-builtin', '-c', str(source), '-o', str(obj)], check=True)
    extra = []
    if machine == 'x86':
        extra = [f'/alternatename:__imp__{name}@{count}=__imp__{name}'
                 for name, count in [('GetStdHandle', 4), ('WriteFile', 20), ('ExitProcess', 4), ('Sleep', 4)]]
    subprocess.run(linker + ['/entry:mainCRTStartup', '/subsystem:console', '/nodefaultlib', f'/machine:{machine}', f'/out:{exe}', str(obj), str(library)] + extra, check=True)
    print(exe)
