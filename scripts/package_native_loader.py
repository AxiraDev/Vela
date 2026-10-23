#!/usr/bin/env python3
"""Give a locally built ARM64 Wine loader its macOS application identity.
Only the unrestricted custom-x18 entitlement is used. No provisioning profile.
"""
import argparse
from pathlib import Path
import plistlib
import shutil
import subprocess
import tempfile

p = argparse.ArgumentParser(description=__doc__)
p.add_argument('--runtime', required=True, type=Path)
a = p.parse_args()
runtime = a.runtime.resolve()
unix = runtime / 'lib/wine/aarch64-unix'
loader = unix / 'wine'
if not loader.is_file() or loader.is_symlink():
    raise SystemExit('Expected an unpackaged native Wine installation')
if subprocess.check_output(['lipo', '-archs', str(loader)], text=True).strip() != 'arm64':
    raise SystemExit('Expected an ARM64 loader')
app = unix / 'VelaWine.app'
if app.exists():
    raise SystemExit('Loader bundle already exists')
with tempfile.TemporaryDirectory(prefix='vela-wine-loader-') as tmp:
    stage = Path(tmp) / app.name
    macos = stage / 'Contents/MacOS'
    macos.mkdir(parents=True)
    shutil.copyfile(loader, macos / 'wine')
    (macos / 'wine').chmod(0o755)
    info = {'CFBundleIdentifier': 'org.vela.engine.wine', 'CFBundleName': 'Vela Wine',
            'CFBundleExecutable': 'wine', 'CFBundlePackageType': 'APPL',
            'CFBundleShortVersionString': '11.18', 'CFBundleVersion': '1',
            'LSMinimumSystemVersion': '26.5', 'NSPrincipalClass': 'WineApplication',
            'LSUIElement': True, 'NSHighResolutionCapable': True}
    (stage / 'Contents/Info.plist').write_bytes(plistlib.dumps(info))
    entitlements = Path(tmp) / 'loader.entitlements'
    entitlements.write_bytes(plistlib.dumps({'com.apple.security.custom-x18-abi-toggle': True}))
    subprocess.run(['codesign', '--force', '--sign', '-', '--entitlements', str(entitlements), str(stage)], check=True)
    subprocess.run(['codesign', '--verify', '--strict', str(stage)], check=True)
    subprocess.run(['ditto', '--norsrc', '--noextattr', str(stage), str(app)], check=True)
loader.unlink()
loader.symlink_to('VelaWine.app/Contents/MacOS/wine')
bin_loader = runtime / 'bin/wine'
bin_loader.unlink()
bin_loader.symlink_to('../lib/wine/aarch64-unix/VelaWine.app/Contents/MacOS/wine')
print(app)
