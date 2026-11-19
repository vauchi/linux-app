// SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me>
// SPDX-License-Identifier: GPL-3.0-or-later
//! `--reset-for-testing` seeds a test identity in every display shell, and
//! the toolkit never sees the flag (private#1029: qvauchi ignored it). A
//! launch offers its `vauchi://` links to a running app before it starts.
use vauchi_linux_app::wiring::{launch_unless_handed_off, take_reset_for_testing};

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

// @internal — a second qvauchi must not open the same data twice
// (private#650, #1029): the running app takes the link, this launch ends.
#[test]
fn a_launch_whose_link_a_running_app_took_starts_nothing() {
    let launch_args = args(&["qvauchi", "vauchi://exchange/opaque"]);
    let mut offered = Vec::new();
    let mut started = false;

    let exit: Result<i32, String> = launch_unless_handed_off(
        &launch_args,
        |seen| {
            offered = seen.to_vec();
            true
        },
        |_| {
            started = true;
            Ok(7)
        },
    );

    assert_eq!(exit, Ok(0));
    assert_eq!(offered, launch_args);
    assert!(!started);
}

// @internal — with no running app to take the link, this launch is the app.
#[test]
fn a_launch_nobody_took_starts_the_app_with_its_arguments() {
    let launch_args = args(&["qvauchi", "vauchi://exchange/opaque"]);
    let mut started_with = Vec::new();

    let exit: Result<i32, String> = launch_unless_handed_off(
        &launch_args,
        |_| false,
        |seen| {
            started_with = seen.to_vec();
            Ok(7)
        },
    );

    assert_eq!(exit, Ok(7));
    assert_eq!(started_with, launch_args);
}

// @internal — a failed start is reported, not turned into a clean exit.
#[test]
fn a_failed_start_is_returned_to_the_caller() {
    let exit: Result<i32, String> = launch_unless_handed_off(
        &args(&["qvauchi"]),
        |_| false,
        |_| Err("engine did not start".to_owned()),
    );

    assert_eq!(exit, Err("engine did not start".to_owned()));
}
