# SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me>
# SPDX-License-Identifier: GPL-3.0-or-later

import fcntl
import os
import pathlib
import pty
import resource
import select
import signal
import struct
import subprocess
import sys
import tempfile
import termios
import time
import unittest

BINARY = pathlib.Path(sys.argv.pop()).resolve(strict=True)


class ShutdownSignalsTest(unittest.TestCase):
    # @internal — signal tests own a temporary terminal and isolated profile.
    def test_external_shutdown_restores_terminal(self):
        self.assertIn(BINARY.name, ("vauchi", "vauchi-tui"), "only Cargo-built terminal binaries may be executed")
        resource.setrlimit(resource.RLIMIT_CORE, (0, resource.getrlimit(resource.RLIMIT_CORE)[1]))
        for shutdown in (signal.SIGTERM, signal.SIGINT, signal.SIGHUP, signal.SIGQUIT):
            self.exercise_signal(shutdown)
        print("external shutdown signals restore terminal")

    def exercise_signal(self, shutdown):
        with tempfile.TemporaryDirectory() as directory:
            master, slave = pty.openpty()
            fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH", 35, 100, 0, 0))
            original = termios.tcgetattr(slave)
            environment = dict(os.environ, XDG_DATA_HOME=directory, TERM="xterm-256color")
            environment.pop("VAUCHI_TEST_SEED", None)
            child = None
            try:
                if BINARY.name == "vauchi":
                    child = subprocess.Popen(
                        ["./vauchi"], cwd=BINARY.parent, stdin=slave, stdout=slave,
                        stderr=slave, start_new_session=True, env=environment,
                    )
                    marker = b"Choose"
                else:
                    child = subprocess.Popen(
                        ["./vauchi-tui"], cwd=BINARY.parent, stdin=slave, stdout=slave,
                        stderr=slave, start_new_session=True, env=environment,
                    )
                    marker = b"Create new identity"
                exited = os.pidfd_open(child.pid)
                try:
                    transcript = bytearray()
                    deadline = time.monotonic() + 20
                    while marker not in transcript:
                        remaining = deadline - time.monotonic()
                        self.assertGreater(remaining, 0, f"startup timed out: {transcript!r}")
                        ready, _, _ = select.select([master, exited], [], [], remaining)
                        self.assertTrue(ready, "startup produced no terminal output")
                        if master in ready:
                            transcript.extend(os.read(master, 65536))
                        self.assertIsNone(child.poll(), f"exited before startup: {transcript!r}")
                    self.assertFalse(termios.tcgetattr(slave)[3] & termios.ECHO, "terminal was not in native input mode")
                    child.send_signal(shutdown)
                    deadline = time.monotonic() + 10
                    while child.poll() is None:
                        remaining = deadline - time.monotonic()
                        self.assertGreater(remaining, 0, f"signal {shutdown} did not stop the loop")
                        ready, _, _ = select.select([master, exited], [], [], remaining)
                        if master in ready:
                            transcript.extend(os.read(master, 65536))
                    self.assertEqual(child.wait(), 0, f"signal {shutdown} bypassed graceful cleanup")
                    while select.select([master], [], [], 0)[0]:
                        data = os.read(master, 65536)
                        if not data:
                            break
                        transcript.extend(data)
                    self.assertEqual(termios.tcgetattr(slave), original, f"signal {shutdown} left terminal attributes changed")
                    if BINARY.name == "vauchi-tui":
                        self.assertIn(b"\x1b[?1049h", transcript)
                        self.assertIn(b"\x1b[?1049l", transcript)
                        restored = transcript.rfind(b"\x1b[?1049l")
                        self.assertGreater(transcript.rfind(b"\x1b[?25h", 0, restored), transcript.rfind(b"\x1b[?25l", 0, restored))
                finally:
                    os.close(exited)
            finally:
                if child is not None:
                    if child.poll() is None:
                        child.kill()
                    child.wait(timeout=5)
                os.close(master)
                os.close(slave)


if __name__ == "__main__":
    unittest.main()
