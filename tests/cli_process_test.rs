// SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me>
// SPDX-License-Identifier: GPL-3.0-or-later

#![cfg(feature = "cli")]

// @internal — actual native input must exit on Ctrl-C/EOF and restore its terminal.
#[test]
fn native_cli_quits_on_ctrl_c_and_pipe_eof_and_restores_its_terminal() {
    let output = std::process::Command::new("python3")
        .arg(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/support/cli_process.py"
        ))
        .arg(env!("CARGO_BIN_EXE_vauchi"))
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
        "CLI startup; quit and EOF clean; terminal restored"
    );
}
