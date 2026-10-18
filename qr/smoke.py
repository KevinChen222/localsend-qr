"""Exercise the built CLI over HTTP; this never needs a real phone."""
import hashlib
import http.client
import json
import os
from pathlib import Path
import re
import signal
import subprocess
import tempfile
import threading
import time
import urllib.parse
import zipfile

root = Path(__file__).resolve().parent.parent
exe = root / "target/release" / ("localsend-qr.exe" if os.name == "nt" else "localsend-qr")
with tempfile.TemporaryDirectory() as work:
    folder = Path(work) / "测试 folder"
    folder.mkdir()
    (folder / "empty").mkdir()
    data = bytes(range(256)) * 4000
    (folder / "payload.bin").write_bytes(data)
    output = []
    process = subprocess.Popen([str(exe), "share", "--port", "0", "--expires", "1", "--", str(folder)],
                               stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
                               text=True, encoding="utf-8", errors="replace")
    def read_output():
        for line in process.stdout:
            output.append(line)
    reader = threading.Thread(target=read_output, daemon=True)
    reader.start()
    # If CI exposes multiple interfaces, select the first; HTTP test uses loopback.
    process.stdin.write("1\n")
    process.stdin.flush()
    try:
        deadline = time.monotonic() + 30
        match = None
        while time.monotonic() < deadline and process.poll() is None:
            match = re.search(r"http://[0-9.]+:(\d+)/#key=([a-f0-9]+)", "".join(output))
            if match:
                break
            time.sleep(0.1)
        assert match, "CLI did not print QR link:\n" + "".join(output)
        port, key = match.groups()
        connection = http.client.HTTPConnection("127.0.0.1", int(port), timeout=20)
        connection.request("POST", "/api/localsend/v2/prepare-download?pin=" + key)
        response = connection.getresponse()
        assert response.status == 200, response.status
        files = json.loads(response.read())
        file_id = next(iter(files["files"]))
        query = urllib.parse.urlencode({"sessionId": files["sessionId"], "fileId": file_id})
        connection.request("GET", "/api/localsend/v2/download?" + query)
        response = connection.getresponse()
        assert response.status == 200
        downloaded = Path(work) / "received.zip"
        downloaded.write_bytes(response.read())
        with zipfile.ZipFile(downloaded) as archive:
            assert archive.read("测试 folder/payload.bin") == data
            assert "测试 folder/empty/" in archive.namelist()
        connection.close()
        print("PASS: CLI QR URL, keyed browser session, ZIP bytes, Unicode paths, empty directories")
    finally:
        process.terminate()
        process.wait(timeout=10)
