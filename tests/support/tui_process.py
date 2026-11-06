# SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me>
# SPDX-License-Identifier: GPL-3.0-or-later

import fcntl
import os
import pathlib
import pty
import select
import signal
import struct
import sys
import tempfile
import termios
import time


with tempfile.TemporaryDirectory() as directory:
    master, slave = pty.openpty()
    original = termios.tcgetattr(slave)
    fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH", 35, 100, 0, 0))
    pid = os.fork()
    if pid == 0:
        os.close(master)
        os.setsid()
        fcntl.ioctl(slave, termios.TIOCSCTTY, 0)
        for fd in (0, 1, 2):
            os.dup2(slave, fd)
        if slave > 2:
            os.close(slave)
        environment = dict(os.environ, XDG_DATA_HOME=directory, TERM="xterm-256color")
        environment.pop("VAUCHI_TEST_SEED", None)
        os.execve(str(pathlib.Path(sys.argv[1]).resolve()), [sys.argv[1]], environment)

    transcript = bytearray()
    exited = False
    try:
        deadline = time.monotonic() + 20
        while b"Create new identity" not in transcript:
            remaining = deadline - time.monotonic()
            assert remaining > 0, f"startup timed out: {transcript!r}"
            ready, _, _ = select.select([master], [], [], remaining)
            assert ready, f"startup did not render: {transcript!r}"
            transcript.extend(os.read(master, 65536))
            child, status = os.waitpid(pid, os.WNOHANG)
            if child:
                exited = True
            assert child == 0, f"exited before startup: {status}: {transcript!r}"
        os.write(master, b"\x03")
        deadline = time.monotonic() + 10
        while True:
            child, status = os.waitpid(pid, os.WNOHANG)
            if child == pid:
                exited = True
                assert os.waitstatus_to_exitcode(status) == 0, f"exit failed: {status}: {transcript!r}"
                break
            remaining = deadline - time.monotonic()
            assert remaining > 0, "Ctrl-C did not terminate the native loop"
            ready, _, _ = select.select([master], [], [], min(remaining, 0.1))
            if ready:
                transcript.extend(os.read(master, 65536))
        while select.select([master], [], [], 0)[0]:
            data = os.read(master, 65536)
            if not data:
                break
            transcript.extend(data)
        assert termios.tcgetattr(slave) == original, "terminal attributes were not restored"
        assert b"\x1b[?1049h" in transcript, "alternate screen was not entered"
        assert b"\x1b[?1049l" in transcript, "alternate screen was not restored"
        restored_screen = transcript.rfind(b"\x1b[?1049l")
        hidden_cursor = transcript.rfind(b"\x1b[?25l", 0, restored_screen)
        shown_cursor = transcript.rfind(b"\x1b[?25h", 0, restored_screen)
        assert shown_cursor > hidden_cursor, "cursor was not restored before leaving the alternate screen"
        print("startup rendered; quit clean; terminal restored")
    finally:
        if not exited:
            os.kill(pid, signal.SIGKILL)
            os.waitpid(pid, 0)
        os.close(master)
        os.close(slave)
