"""Verify Nautilus registration and argv quoting in an isolated XDG directory."""
import os
from pathlib import Path
import shutil
import subprocess
import tempfile

root = Path(__file__).resolve().parent.parent
with tempfile.TemporaryDirectory() as scratch:
    scratch = Path(scratch)
    mock = scratch / "mock-bin"
    mock.mkdir()
    terminal = mock / "gnome-terminal"
    terminal.write_text('#!/usr/bin/env bash\nprintf "%s\\0" "$@" > "$QR_TEST_ARGS"\n')
    terminal.chmod(0o755)
    env = dict(os.environ, XDG_DATA_HOME=str(scratch / "xdg"),
               PATH=str(mock) + os.pathsep + os.environ["PATH"],
               QR_TEST_ARGS=str(scratch / "argv"))
    selected = ["/tmp/中文 folder/a 'quoted'.txt", "/tmp/space & tab\t/file"]
    env["NAUTILUS_SCRIPT_SELECTED_FILE_PATHS"] = "\n".join(selected) + "\n"
    for name in ["first folder", "moved 'folder' & [x]"]:
        folder = scratch / name
        folder.mkdir()
        core = folder / "localsend-qr"
        shutil.copy2(root / "target/release/localsend-qr", core)
        subprocess.run(["bash", str(root / "qr/portable/linux/register-context-menu.sh"), str(core)], env=env, check=True)
        installed = Path(env["XDG_DATA_HOME"]) / "nautilus/scripts/通过二维码传送"
        subprocess.run(["bash", str(installed)], env=env, check=True)
        argv = (scratch / "argv").read_bytes().split(b"\0")[:-1]
        assert [part.decode() for part in argv] == ["--", str(core), "share", "--pause-on-error", "--", *selected]
    subprocess.run(["bash", str(root / "qr/portable/linux/remove-context-menu.sh")], env=env, check=True)
    assert not installed.exists()
print("PASS: Nautilus selection argv, special path quoting, relocation and removal")
