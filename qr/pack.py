"""Package only this fork's portable CLI. Run on its target platform in CI."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import tarfile
import zipfile

parser = argparse.ArgumentParser()
parser.add_argument("platform", choices=["windows-x64", "ubuntu24-x64"])
args = parser.parse_args()
root = Path(__file__).resolve().parent.parent
version = "0.1.0"
name = f"LocalSendQR-{version}-{args.platform}"
out = root / "dist"
folder = out / name
folder.mkdir(parents=True, exist_ok=False)
windows = args.platform == "windows-x64"
exe = "localsend-qr.exe" if windows else "localsend-qr"
shutil.copy2(root / "target" / "release" / exe, folder / exe)
for source in (root / "qr" / "portable" / ("windows" if windows else "linux")).iterdir():
    shutil.copy2(source, folder / source.name)
for source in [root / "LICENSE", root / "qr/portable/README.txt", root / "qr/portable/settings.json"]:
    shutil.copy2(source, folder / source.name)
for notice in [root / "NOTICE", root / "NOTICE.txt"]:
    if notice.exists():
        shutil.copy2(notice, folder / notice.name)
if windows:
    # PowerShell 5.1 needs a BOM to read non-ASCII script source reliably.
    for source in folder.glob("*.ps1"):
        source.write_text(source.read_text(encoding="utf-8"), encoding="utf-8-sig", newline="\r\n")
else:
    for source in [folder / exe, *folder.glob("*.sh")]:
        source.chmod(0o755)
manifest = {p.name: hashlib.sha256(p.read_bytes()).hexdigest() for p in sorted(folder.iterdir())}
(folder / "SHA256SUMS.json").write_text(json.dumps(manifest, indent=2) + "\n", encoding="utf-8")
if windows:
    archive = out / (name + ".zip")
    with zipfile.ZipFile(archive, "w", zipfile.ZIP_DEFLATED) as bundle:
        for source in sorted(folder.iterdir()):
            bundle.write(source, f"{name}/{source.name}")
else:
    archive = out / (name + ".tar.gz")
    with tarfile.open(archive, "w:gz") as bundle:
        bundle.add(folder, arcname=name)
print(archive)
print("SHA256:", hashlib.sha256(archive.read_bytes()).hexdigest())
