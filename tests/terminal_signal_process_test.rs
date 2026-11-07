// SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me>
// SPDX-License-Identifier: GPL-3.0-or-later

#![cfg(any(feature = "cli", feature = "tui"))]

fn run(binary: &str, optimized: bool) -> std::process::Output {
    let mut command = std::process::Command::new("python3");
    if optimized {
        command.arg("-O");
    }
    command
        .arg(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/support/terminal_signals.py"
        ))
        .arg(binary)
        .output()
        .unwrap()
}

// @internal — external shutdown must restore the actual CLI terminal before process exit.
#[cfg(feature = "cli")]
#[test]
fn native_cli_restores_terminal_after_external_shutdown_signals() {
    for optimized in [false, true] {
        let output = run(env!("CARGO_BIN_EXE_vauchi"), optimized);
        assert_eq!(
            output.status.code(),
            Some(0),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            String::from_utf8_lossy(&output.stdout).trim(),
            "external shutdown signals restore terminal"
        );
    }
}

// @internal — TUI restoration includes raw mode, the alternate screen and the cursor.
#[cfg(feature = "tui")]
#[test]
fn native_tui_restores_terminal_after_external_shutdown_signals() {
    for optimized in [false, true] {
        let output = run(env!("CARGO_BIN_EXE_vauchi-tui"), optimized);
        assert_eq!(
            output.status.code(),
            Some(0),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            String::from_utf8_lossy(&output.stdout).trim(),
            "external shutdown signals restore terminal"
        );
    }
}

// @internal — the helper's executable guard must remain active under python -O.
#[test]
fn native_signal_helper_rejects_other_executables() {
    for optimized in [false, true] {
        let output = run("/usr/bin/true", optimized);
        assert_eq!(output.status.code(), Some(1));
        assert!(
            String::from_utf8_lossy(&output.stderr)
                .contains("only Cargo-built terminal binaries may be executed")
        );
    }
}
