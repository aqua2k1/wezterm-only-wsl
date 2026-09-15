"""Run with: python3 -m unittest discover -s tools/perf -p 'test_*.py'."""
import base64
import errno
import json
import os
from pathlib import Path
import pty
import re
import select
import subprocess
import tempfile
import time
import unittest


class WorkerTest(unittest.TestCase):
    def test_workloads_with_terminal_response(self):
        for mode in ('idle', 'ascii', 'ansi', 'unicode', 'kitty'):
            with self.subTest(mode=mode), tempfile.TemporaryDirectory() as folder:
                result = Path(folder) / 'result.json'
                master, slave = pty.openpty()
                process = subprocess.Popen([
                    'python3', str(Path(__file__).with_name('wsl-workload.py')),
                    '--mode', mode, '--mib', '1', '--idle-seconds', '0.01',
                    '--result', str(result),
                ], stdin=slave, stdout=slave, stderr=subprocess.PIPE)
                os.close(slave)
                tail = b''
                queries = 0
                deadline = time.monotonic() + 20
                try:
                    while not result.exists():
                        if time.monotonic() > deadline:
                            self.fail(f'worker timed out in {mode}')
                        if Path(str(result) + '.ready').exists():
                            Path(str(result) + '.go').touch()
                        if select.select([master], [], [], 0.02)[0]:
                            try:
                                data = os.read(master, 65536)
                            except OSError as error:
                                if error.errno != errno.EIO:
                                    raise
                                self.fail(process.stderr.read().decode())
                            joined = tail + data
                            matches = list(re.finditer(
                                rb'\x1b\]1337;SetUserVar=PERF_FENCE=([^\x07]+)\x07', joined))
                            queries += len(matches)
                            for match in matches:
                                token = base64.b64decode(match.group(1))
                                os.write(master, b'PERF_ACK:' + token + b':END')
                            kitty = list(re.finditer(
                                rb'\x1b_Ga=q,f=100,i=9001;[^\x1b]+\x1b\\', joined))
                            for _ in kitty:
                                os.write(master, b'\x1b_Gi=9001;OK\x1b\\')
                            ends = [match.end() for match in matches + kitty]
                            tail = joined[max(ends):] if ends else joined
                            tail = tail[-4096:]
                    record = json.loads(result.read_text())
                    self.assertNotIn('error', record)
                    self.assertEqual(record['mode'], mode)
                    self.assertGreater(record['seconds'], 0)
                    self.assertGreaterEqual(queries, 2)
                    if mode in ('ascii', 'ansi', 'unicode'):
                        self.assertGreaterEqual(record['bytes'], 1024 * 1024)
                    if mode == 'kitty':
                        self.assertEqual(record['images'], 120)
                        self.assertTrue(record['kitty_query_verified'])
                        self.assertEqual(queries, 2)
                    Path(str(result) + '.done').touch()
                    self.assertEqual(process.wait(timeout=5), 0, process.stderr.read())
                finally:
                    if process.poll() is None:
                        process.kill()
                        process.wait()
                    process.stderr.close()
                    os.close(master)


if __name__ == '__main__':
    unittest.main()
