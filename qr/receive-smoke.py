"""Exercise receive startup, the copied LAN URL, and phone uploads without a phone."""
import ctypes
import http.client
import json
import os
from pathlib import Path
import re
import subprocess
import tempfile
import threading
import time
import urllib.parse

root = Path(__file__).resolve().parent.parent
exe = Path(os.environ.get("LOCALSEND_QR_EXE", root / "target/release" / ("localsend-qr.exe" if os.name == "nt" else "localsend-qr")))
preferred_port = os.environ.get("LOCALSEND_QR_TEST_PORT", "0")


def windows_clipboard():
    user = ctypes.windll.user32
    kernel = ctypes.windll.kernel32
    user.GetClipboardData.restype = ctypes.c_void_p
    kernel.GlobalLock.argtypes = [ctypes.c_void_p]
    kernel.GlobalLock.restype = ctypes.c_void_p
    kernel.GlobalUnlock.argtypes = [ctypes.c_void_p]
    if not user.OpenClipboard(None):
        return ""
    try:
        handle = user.GetClipboardData(13)
        if not handle:
            return ""
        pointer = kernel.GlobalLock(handle)
        if not pointer:
            return ""
        try:
            return ctypes.wstring_at(pointer)
        finally:
            kernel.GlobalUnlock(handle)
    finally:
        user.CloseClipboard()


with tempfile.TemporaryDirectory() as work:
    work = Path(work)
    destination = work / "中文 receive folder"
    destination.mkdir()
    env = dict(os.environ)
    clipboard = work / "clipboard"
    if os.name != "nt":
        mock = work / "bin"
        mock.mkdir()
        helper = mock / "wl-copy"
        helper.write_text('#!/bin/sh\ncat > "$QR_TEST_CLIPBOARD"\n')
        helper.chmod(0o755)
        env.update(PATH=str(mock) + os.pathsep + env["PATH"], WAYLAND_DISPLAY="qr-test", QR_TEST_CLIPBOARD=str(clipboard))
    previous_clipboard = windows_clipboard() if os.name == "nt" else ""
    process = subprocess.Popen([str(exe), "receive", "--port", preferred_port, "--expires", "1", "--", str(destination)],
                               env=env, stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
                               text=True, encoding="utf-8", errors="replace")
    output = []

    def read_output():
        output.extend(process.stdout)

    reader = threading.Thread(target=read_output, daemon=True)
    reader.start()
    process.stdin.write("1\n")
    process.stdin.flush()
    try:
        deadline = time.monotonic() + 30
        match = None
        while time.monotonic() < deadline and process.poll() is None:
            text = windows_clipboard() if os.name == "nt" else (clipboard.read_text() if clipboard.exists() else "")
            match = re.fullmatch(r"http://[0-9.]+:(\d+)/#key=([a-f0-9]{32})", text)
            if match and text != previous_clipboard:
                break
            time.sleep(0.1)
        assert match, "LAN URL was not copied:\n" + "".join(output)
        port, key = match.groups()
        connection = http.client.HTTPConnection("127.0.0.1", int(port), timeout=20)
        connection.request("GET", "/")
        response = connection.getresponse()
        assert response.status == 200
        assert "XMLHttpRequest" in response.read().decode()
        files = {"one": {"id": "one", "fileName": "手机 文件.txt", "size": 5, "fileType": "text/plain"},
                 "two": {"id": "two", "fileName": "empty.txt", "size": 0, "fileType": "text/plain"}}
        body = json.dumps({"info": {"alias": "phone", "version": "2.2", "fingerprint": "browser", "port": 0, "protocol": "http"}, "files": files})
        connection.request("POST", "/api/localsend/v2/prepare-upload?pin=" + key, body, {"Content-Type": "application/json"})
        response = connection.getresponse()
        assert response.status == 200, response.read()
        session = json.loads(response.read())
        for file_id, data in [("one", b"hello"), ("two", b"")]:
            query = urllib.parse.urlencode({"sessionId": session["sessionId"], "fileId": file_id, "token": session["files"][file_id]})
            connection.request("POST", "/api/localsend/v2/upload?" + query, data)
            response = connection.getresponse()
            assert response.status == 200, response.read()
            response.read()
        assert (destination / "手机 文件.txt").read_bytes() == b"hello"
        assert (destination / "empty.txt").read_bytes() == b""
        assert not list(destination.glob("*.part"))
        assert "http://" not in "".join(output), "Default window should not print a long LAN URL"
        qr_lines = [line for line in output if any(c in line for c in "█▀▄")]
        assert len(qr_lines) <= 21, len(qr_lines)
        connection.close()
        print("PASS: automatically copied LAN URL, compact QR window, phone multi-file upload, Unicode destination")
    finally:
        process.terminate()
        process.wait(timeout=10)
