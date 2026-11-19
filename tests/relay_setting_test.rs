// SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me>
// SPDX-License-Identifier: GPL-3.0-or-later
//! A relay is three things: its address, the trust anchor its OHTTP keys
//! chain to, and the OHTTP relay in front of it. Core accepts no key from a
//! relay without an anchor unless it is the production one, so an address
//! alone never synced. The three come from one place, never mixed.
use vauchi_core::api::VauchiConfig;
use vauchi_linux_app::relay_setting::{RelaySetting, RelaySources, resolve};

const ANCHOR_HEX: &str = "5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a";
const OTHER_ANCHOR_HEX: &str = "1111111111111111111111111111111111111111111111111111111111111111";

fn some(value: &str) -> Option<String> {
    Some(value.to_owned())
}

fn setting_file() -> Option<String> {
    Some(format!(
        r#"{{"url": "wss://relay.home.example", "anchor": "{ANCHOR_HEX}", "ohttp_relay": "https://ohttp.home.example"}}"#
    ))
}

// @internal
#[test]
fn the_setting_file_gives_address_anchor_and_ohttp_relay() {
    let setting = resolve(RelaySources {
        setting_file: setting_file(),
        ..RelaySources::default()
    })
    .expect("a complete setting file resolves");

    assert_eq!(
        setting,
        RelaySetting {
            url: "wss://relay.home.example".to_owned(),
            anchor: Some([0x5a; 32]),
            ohttp_relay: some("https://ohttp.home.example"),
        }
    );
}

// @internal
#[test]
fn the_setting_file_is_taken_whole_and_the_environment_left_out() {
    let setting = resolve(RelaySources {
        setting_file: some(r#"{"url": "wss://relay.home.example"}"#),
        env_url: some("ws://127.0.0.1:9000"),
        env_anchor: some(OTHER_ANCHOR_HEX),
        env_ohttp_relay: some("http://127.0.0.1:9001"),
        ..RelaySources::default()
    })
    .expect("a setting file with an address resolves");

    assert_eq!(
        setting,
        RelaySetting {
            url: "wss://relay.home.example".to_owned(),
            anchor: None,
            ohttp_relay: None,
        }
    );
}

// @internal
#[test]
fn the_old_address_file_gives_an_address_and_no_other_relay_s_anchor() {
    let setting = resolve(RelaySources {
        legacy_url_file: some("wss://relay.old.example\n"),
        env_url: some("ws://127.0.0.1:9000"),
        env_anchor: some(OTHER_ANCHOR_HEX),
        ..RelaySources::default()
    })
    .expect("the old address file resolves");

    assert_eq!(
        setting,
        RelaySetting {
            url: "wss://relay.old.example".to_owned(),
            anchor: None,
            ohttp_relay: None,
        }
    );
}

// @internal
#[test]
fn the_environment_gives_all_three_when_no_file_does() {
    let setting = resolve(RelaySources {
        env_url: some("ws://127.0.0.1:9000"),
        env_anchor: some(ANCHOR_HEX),
        env_ohttp_relay: some("http://127.0.0.1:9001"),
        ..RelaySources::default()
    })
    .expect("the environment resolves");

    assert_eq!(
        setting,
        RelaySetting {
            url: "ws://127.0.0.1:9000".to_owned(),
            anchor: Some([0x5a; 32]),
            ohttp_relay: some("http://127.0.0.1:9001"),
        }
    );
}

// @internal
#[test]
fn an_anchor_in_the_environment_without_an_address_belongs_to_no_relay() {
    let error = resolve(RelaySources {
        env_anchor: some(ANCHOR_HEX),
        ..RelaySources::default()
    })
    .expect_err("an anchor without its relay must be refused");

    assert_eq!(
        error,
        "VAUCHI_RELAY_ANCHOR and VAUCHI_OHTTP_RELAY_URL describe the relay in VAUCHI_RELAY_URL, which is not set"
    );
}

// @internal
#[test]
fn with_nothing_set_the_relay_is_the_production_one() {
    let setting = resolve(RelaySources::default()).expect("no setting resolves");

    assert_eq!(
        setting,
        RelaySetting {
            url: "wss://relay.vauchi.app".to_owned(),
            anchor: None,
            ohttp_relay: None,
        }
    );
    assert!(!setting.lacks_its_anchor());
}

// @internal
#[test]
fn blank_values_count_as_not_set() {
    let setting = resolve(RelaySources {
        legacy_url_file: some("  \n"),
        env_url: some(""),
        env_anchor: some(" "),
        env_ohttp_relay: some(""),
        ..RelaySources::default()
    })
    .expect("blank values resolve");

    assert_eq!(setting.url, "wss://relay.vauchi.app");
}

// @internal
#[test]
fn an_anchor_that_is_not_64_hex_characters_is_refused() {
    for anchor in [
        &ANCHOR_HEX[1..],
        "not-hex",
        &format!("{ANCHOR_HEX}00"),
        &format!("{}zz", &ANCHOR_HEX[2..]),
        // A sign is accepted by integer parsing and is no hex digit.
        &format!("+{}", &ANCHOR_HEX[1..]),
    ] {
        let error = resolve(RelaySources {
            env_url: some("ws://127.0.0.1:9000"),
            env_anchor: some(anchor),
            ..RelaySources::default()
        })
        .expect_err("a malformed anchor must be refused, not dropped");

        assert_eq!(error, "a relay anchor is 64 hex characters", "{anchor}");
    }
}

// @internal
#[test]
fn a_setting_file_that_is_not_the_expected_object_is_refused() {
    for (file, reason) in [
        (
            "wss://relay.home.example",
            "relay.json is not a JSON object",
        ),
        ("[]", "relay.json is not a JSON object"),
        (r#"{"anchor": "00"}"#, "relay.json names no \"url\""),
        (r#"{"url": 7}"#, "relay.json: \"url\" is not text"),
        (r#"{"url": "  "}"#, "relay.json names no \"url\""),
        (
            r#"{"url": "wss://relay.home.example", "ohttp-relay": "https://o.example"}"#,
            "relay.json: unknown key \"ohttp-relay\" (known: url, anchor, ohttp_relay)",
        ),
        (
            r#"{"url": "wss://relay.home.example", "anchor": "abc"}"#,
            "a relay anchor is 64 hex characters",
        ),
    ] {
        let error = resolve(RelaySources {
            setting_file: some(file),
            ..RelaySources::default()
        })
        .expect_err("a malformed setting file must be refused");

        assert_eq!(error, reason, "{file}");
    }
}

// @internal
#[test]
fn a_relay_of_one_s_own_without_an_anchor_is_reported() {
    let setting = resolve(RelaySources {
        legacy_url_file: some("wss://relay.old.example"),
        ..RelaySources::default()
    })
    .expect("the old address file resolves");

    assert!(setting.lacks_its_anchor());
}

// @internal
#[test]
fn the_setting_reaches_core_s_configuration_as_one_relay() {
    let setting = resolve(RelaySources {
        setting_file: setting_file(),
        ..RelaySources::default()
    })
    .expect("a complete setting file resolves");

    let config = setting.apply(VauchiConfig::default());

    assert_eq!(config.relay.server_url, "wss://relay.home.example");
    assert_eq!(config.relay.ohttp_trust_anchor(), Some([0x5a; 32]));
    assert_eq!(
        config.relay.ohttp_relay_url.as_deref(),
        Some("https://ohttp.home.example")
    );
    assert!(!setting.lacks_its_anchor());
}

// @internal
#[test]
fn a_setting_without_an_anchor_leaves_core_none_for_that_relay() {
    let setting = resolve(RelaySources {
        legacy_url_file: some("wss://relay.old.example"),
        ..RelaySources::default()
    })
    .expect("the old address file resolves");

    let config = setting.apply(VauchiConfig::default());

    assert_eq!(config.relay.server_url, "wss://relay.old.example");
    assert_eq!(config.relay.ohttp_trust_anchor(), None);
}
