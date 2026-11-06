// SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me>
// SPDX-License-Identifier: GPL-3.0-or-later

#![cfg(feature = "tui")]

// @internal — the actual binary must paint its startup surface, exit on Ctrl-C, and restore the terminal.
#[test]
fn native_tui_quits_and_restores_its_terminal() {
    for optimized in [false, true] {
        let mut command = std::process::Command::new("python3");
        if optimized {
            command.arg("-O");
        }
        let output = command
            .arg(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/tests/support/tui_process.py"
            ))
            .arg(env!("CARGO_BIN_EXE_vauchi-tui"))
            .output()
            .unwrap();
        assert_eq!(
            output.status.code(),
            Some(0),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            String::from_utf8_lossy(&output.stdout).trim(),
            "startup rendered; quit clean; terminal restored"
        );
    }
}
