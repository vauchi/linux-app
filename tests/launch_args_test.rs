// SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me>
// SPDX-License-Identifier: GPL-3.0-or-later
//! `--reset-for-testing` seeds a test identity in every display shell, and
//! the toolkit never sees the flag (private#1029: qvauchi ignored it).
use vauchi_linux_app::wiring::take_reset_for_testing;

fn args(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| (*value).to_owned()).collect()
}

// @internal — the AT-SPI suites start the app with this flag.
#[test]
fn the_flag_is_taken_out_and_reported() {
    assert_eq!(
        take_reset_for_testing(args(&[
            "qvauchi",
            "--reset-for-testing",
            "-platform",
            "xcb"
        ])),
        (args(&["qvauchi", "-platform", "xcb"]), true)
    );
}

// @internal — the AT-SPI suites start the app with this flag.
#[test]
fn without_the_flag_the_arguments_pass_unchanged() {
    for values in [
        &["qvauchi"][..],
        &["qvauchi", "vauchi://exchange/opaque"][..],
        &["qvauchi", "--reset-for-testing=1", "reset-for-testing"][..],
    ] {
        assert_eq!(take_reset_for_testing(args(values)), (args(values), false));
    }
}

// @internal — the AT-SPI suites start the app with this flag.
#[test]
fn a_repeated_flag_is_taken_out_entirely() {
    assert_eq!(
        take_reset_for_testing(args(&[
            "qvauchi",
            "--reset-for-testing",
            "--reset-for-testing"
        ])),
        (args(&["qvauchi"]), true)
    );
}
