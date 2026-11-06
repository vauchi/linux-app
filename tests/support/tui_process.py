# SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me>
# SPDX-License-Identifier: GPL-3.0-or-later

import fcntl
import os
import pathlib
import pty
import select
import struct
import subprocess
import sys
import tempfile
import termios
import time
import unittest


BINARY = pathlib.Path(sys.argv.pop()).resolve(strict=True)


class TerminalProcessTest(unittest.TestCase):
    # @internal — native acceptance checks must remain active under python -O.
    def test_startup_quit_and_terminal_restoration(self):
        self.assertEqual(BINARY.name, "vauchi-tui", "only the Cargo-built vauchi-tui binary may be executed")
        with tempfile.TemporaryDirectory() as directory:
            master, slave = pty.openpty()
            original = termios.tcgetattr(slave)
            fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH", 35, 100, 0, 0))
            environment = dict(os.environ, XDG_DATA_HOME=directory, TERM="xterm-256color")
            environment.pop("VAUCHI_TEST_SEED", None)
            child = None
            try:
                child = subprocess.Popen(
                    ["./vauchi-tui"], cwd=BINARY.parent, stdin=slave, stdout=slave, stderr=slave,
                    start_new_session=True, env=environment,
                )
                exited = os.pidfd_open(child.pid)
                try:
                    self.exercise_terminal(child, master, slave, exited, original)
                finally:
                    os.close(exited)
            finally:
                if child is not None:
                    if child.poll() is None:
                        child.kill()
                    child.wait(timeout=5)
                os.close(master)
                os.close(slave)

    def exercise_terminal(self, child, master, slave, exited, original):
        transcript = bytearray()
        deadline = time.monotonic() + 20
        while b"Create new identity" not in transcript:
            remaining = deadline - time.monotonic()
            self.assertGreater(remaining, 0, f"startup timed out: {transcript!r}")
            ready, _, _ = select.select([master, exited], [], [], remaining)
            self.assertTrue(ready, f"startup did not render: {transcript!r}")
            if master in ready:
                transcript.extend(os.read(master, 65536))
            self.assertIsNone(child.poll(), f"exited before startup: {transcript!r}")
        os.write(master, b"\x03")
        deadline = time.monotonic() + 10
        while child.poll() is None:
            remaining = deadline - time.monotonic()
            self.assertGreater(remaining, 0, "Ctrl-C did not terminate the native loop")
            ready, _, _ = select.select([master, exited], [], [], remaining)
            if master in ready:
                transcript.extend(os.read(master, 65536))
        self.assertEqual(child.wait(), 0, f"exit failed: {transcript!r}")
        while select.select([master], [], [], 0)[0]:
            data = os.read(master, 65536)
            if not data:
                break
            transcript.extend(data)
        self.assertEqual(termios.tcgetattr(slave), original, "terminal attributes were not restored")
        self.assertIn(b"\x1b[?1049h", transcript, "alternate screen was not entered")
        self.assertIn(b"\x1b[?1049l", transcript, "alternate screen was not restored")
        restored_screen = transcript.rfind(b"\x1b[?1049l")
        hidden_cursor = transcript.rfind(b"\x1b[?25l", 0, restored_screen)
        shown_cursor = transcript.rfind(b"\x1b[?25h", 0, restored_screen)
        self.assertGreater(shown_cursor, hidden_cursor, "cursor was not restored before leaving the alternate screen")
        print("startup rendered; quit clean; terminal restored")


if __name__ == "__main__":
    unittest.main()
