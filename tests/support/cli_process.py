# SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me>
# SPDX-License-Identifier: GPL-3.0-or-later

import os
import pathlib
import pty
import select
import subprocess
import sys
import tempfile
import termios
import time
import unittest

BINARY = pathlib.Path(sys.argv.pop()).resolve(strict=True)


class CliProcessTest(unittest.TestCase):
    def wait_for_prompt(self, child, descriptor):
        exited = os.pidfd_open(child.pid)
        transcript = bytearray()
        try:
            deadline = time.monotonic() + 20
            while b"Choose" not in transcript:
                remaining = deadline - time.monotonic()
                self.assertGreater(remaining, 0, f"startup timed out: {transcript!r}")
                ready, _, _ = select.select([descriptor, exited], [], [], remaining)
                self.assertTrue(ready, "startup produced no prompt")
                if descriptor in ready:
                    transcript.extend(os.read(descriptor, 65536))
                self.assertIsNone(child.poll(), f"exited before startup: {transcript!r}")
            return transcript
        finally:
            os.close(exited)

    # @internal — assertions remain active under optimized Python.
    def test_native_terminal_and_pipe_eof(self):
        self.assertEqual(BINARY.name, "vauchi", "only the Cargo-built vauchi binary may be executed")
        with tempfile.TemporaryDirectory() as directory:
            environment = dict(os.environ, XDG_DATA_HOME=directory, TERM="xterm-256color")
            environment.pop("VAUCHI_TEST_SEED", None)
            master, slave = pty.openpty()
            original = termios.tcgetattr(slave)
            child = None
            try:
                child = subprocess.Popen(
                    ["./vauchi"], cwd=BINARY.parent, stdin=slave, stdout=slave, stderr=slave,
                    start_new_session=True, env=environment,
                )
                transcript = self.wait_for_prompt(child, master)
                self.assertFalse(termios.tcgetattr(slave)[3] & termios.ECHO, "native input still echoes")
                os.write(master, b"\x03")
                self.assertEqual(child.wait(timeout=10), 0, f"Ctrl-C failed: {transcript!r}")
                self.assertEqual(termios.tcgetattr(slave), original, "terminal attributes were not restored")
            finally:
                if child is not None:
                    if child.poll() is None:
                        child.kill()
                    child.wait(timeout=5)
                os.close(master)
                os.close(slave)
            completed = subprocess.run(
                ["./vauchi"], cwd=BINARY.parent, input=b"", capture_output=True,
                env=environment, timeout=20, check=False,
            )
            self.assertEqual(completed.returncode, 0, completed.stderr.decode(errors="replace"))
            self.assertIn(b"Choose", completed.stdout, "pipe mode did not render")
            master, slave = pty.openpty()
            original = termios.tcgetattr(slave)
            child = None
            try:
                child = subprocess.Popen(
                    ["./vauchi"], cwd=BINARY.parent, stdin=slave, stdout=subprocess.PIPE,
                    stderr=subprocess.PIPE, start_new_session=True, env=environment,
                )
                self.wait_for_prompt(child, child.stdout.fileno())
                child.stdout.close()
                os.write(master, b"x")
                self.assertEqual(child.wait(timeout=20), 1, "broken stdout did not fail")
                self.assertEqual(termios.tcgetattr(slave), original, "I/O failure left terminal raw")
            finally:
                if child is not None:
                    if child.poll() is None:
                        child.kill()
                    child.wait(timeout=5)
                    if child.stderr is not None:
                        child.stderr.close()
                os.close(master)
                os.close(slave)
        print("CLI startup; quit and EOF clean; terminal restored")


if __name__ == "__main__":
    unittest.main()
