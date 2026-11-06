// SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me>
// SPDX-License-Identifier: GPL-3.0-or-later

#![cfg(feature = "cli")]

use std::os::unix::ffi::OsStringExt;

// @internal — an interactive host must reject command options rather than silently ignore them.
#[test]
fn native_cli_rejects_unsupported_arguments() {
    for argument in [
        std::ffi::OsString::from("--unsupported"),
        std::ffi::OsString::from_vec(vec![255]),
    ] {
        let directory = tempfile::tempdir().unwrap();
        let output = std::process::Command::new(env!("CARGO_BIN_EXE_vauchi"))
            .arg(argument)
            .env("XDG_DATA_HOME", directory.path())
            .stdin(std::process::Stdio::null())
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(2));
        assert!(String::from_utf8_lossy(&output.stderr).contains("Usage: vauchi"));
        assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 0);
    }
}

// @internal — help/version must not open storage or enter raw mode.
#[test]
fn native_cli_help_and_version_exit_without_starting_the_host() {
    for (argument, expected) in [
        ("--help", "Usage: vauchi"),
        ("--version", concat!("vauchi ", env!("CARGO_PKG_VERSION"))),
    ] {
        let directory = tempfile::tempdir().unwrap();
        let output = std::process::Command::new(env!("CARGO_BIN_EXE_vauchi"))
            .arg(argument)
            .env("XDG_DATA_HOME", directory.path())
            .stdin(std::process::Stdio::null())
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(0));
        assert!(String::from_utf8_lossy(&output.stdout).contains(expected));
        assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 0);
    }
}

// @internal — actual native input must exit on Ctrl-C/EOF and restore its terminal.
#[test]
fn native_cli_quits_on_ctrl_c_and_pipe_eof_and_restores_its_terminal() {
    for optimized in [false, true] {
        let mut command = std::process::Command::new("python3");
        if optimized {
            command.arg("-O");
        }
        let output = command
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
}

// @internal — the helper must reject arbitrary executables even under python -O.
#[test]
fn native_cli_helper_rejects_other_executables() {
    for optimized in [false, true] {
        let mut command = std::process::Command::new("python3");
        if optimized {
            command.arg("-O");
        }
        let output = command
            .arg(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/tests/support/cli_process.py"
            ))
            .arg("/usr/bin/true")
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(1));
        assert!(
            String::from_utf8_lossy(&output.stderr)
                .contains("only the Cargo-built vauchi binary may be executed")
        );
    }
}
