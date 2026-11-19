// SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The relay this install talks to: its address, the trust anchor its OHTTP
//! keys must chain to, and the OHTTP relay in front of it.
//!
//! Core accepts no OHTTP key from a relay without an anchor unless it is
//! the production relay, so an address alone never synced with a relay of
//! one's own. The three are taken from one source, never mixed: an anchor
//! names one relay, and beside another relay's address it would let that
//! relay's keys fail for a reason nobody can see (ADR-082 decision 10).
//!
//! Sources, first match wins:
//! 1. `<data dir>/relay.json`: `{"url": …, "anchor": …, "ohttp_relay": …}`
//! 2. `<data dir>/relay_url.txt`: an address alone (the earlier setting)
//! 3. `VAUCHI_RELAY_URL`, with `VAUCHI_RELAY_ANCHOR` and
//!    `VAUCHI_OHTTP_RELAY_URL`
//! 4. the production relay

use std::path::Path;

use vauchi_core::api::{RelayConfig, VauchiConfig};

const PRODUCTION_RELAY_URL: &str = "wss://relay.vauchi.app";
const SETTING_FILE: &str = "relay.json";
const LEGACY_URL_FILE: &str = "relay_url.txt";
const KNOWN_KEYS: [&str; 3] = ["url", "anchor", "ohttp_relay"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelaySetting {
    pub url: String,
    pub anchor: Option<[u8; 32]>,
    pub ohttp_relay: Option<String>,
}

/// What each source holds, read but not yet interpreted.
#[derive(Debug, Default)]
pub struct RelaySources {
    pub setting_file: Option<String>,
    pub legacy_url_file: Option<String>,
    pub env_url: Option<String>,
    pub env_anchor: Option<String>,
    pub env_ohttp_relay: Option<String>,
}

impl RelaySources {
    pub fn read(data_dir: &Path) -> Self {
        Self {
            setting_file: std::fs::read_to_string(data_dir.join(SETTING_FILE)).ok(),
            legacy_url_file: std::fs::read_to_string(data_dir.join(LEGACY_URL_FILE)).ok(),
            env_url: std::env::var("VAUCHI_RELAY_URL").ok(),
            env_anchor: std::env::var("VAUCHI_RELAY_ANCHOR").ok(),
            env_ohttp_relay: std::env::var("VAUCHI_OHTTP_RELAY_URL").ok(),
        }
    }
}

fn present(value: Option<String>) -> Option<String> {
    value
        .map(|text| text.trim().to_owned())
        .filter(|text| !text.is_empty())
}

fn anchor_from_hex(hex: &str) -> Result<[u8; 32], String> {
    let malformed = || "a relay anchor is 64 hex characters".to_owned();
    // from_str_radix takes a leading sign, so the digits are checked first.
    if hex.len() != 64 || !hex.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(malformed());
    }
    let mut anchor = [0u8; 32];
    for (byte, pair) in anchor.iter_mut().zip(hex.as_bytes().chunks_exact(2)) {
        let pair = std::str::from_utf8(pair).map_err(|_| malformed())?;
        *byte = u8::from_str_radix(pair, 16).map_err(|_| malformed())?;
    }
    Ok(anchor)
}

fn text_of(
    object: &serde_json::Map<String, serde_json::Value>,
    key: &str,
) -> Result<Option<String>, String> {
    match object.get(key) {
        None | Some(serde_json::Value::Null) => Ok(None),
        Some(serde_json::Value::String(text)) => Ok(present(Some(text.clone()))),
        Some(_) => Err(format!("{SETTING_FILE}: \"{key}\" is not text")),
    }
}

fn from_setting_file(file: &str) -> Result<RelaySetting, String> {
    let not_an_object = || format!("{SETTING_FILE} is not a JSON object");
    let value: serde_json::Value = serde_json::from_str(file).map_err(|_| not_an_object())?;
    let object = value.as_object().ok_or_else(not_an_object)?;
    if let Some(unknown) = object
        .keys()
        .find(|key| !KNOWN_KEYS.contains(&key.as_str()))
    {
        return Err(format!(
            "{SETTING_FILE}: unknown key \"{unknown}\" (known: {})",
            KNOWN_KEYS.join(", ")
        ));
    }
    let url = text_of(object, "url")?.ok_or_else(|| format!("{SETTING_FILE} names no \"url\""))?;
    Ok(RelaySetting {
        url,
        anchor: text_of(object, "anchor")?
            .as_deref()
            .map(anchor_from_hex)
            .transpose()?,
        ohttp_relay: text_of(object, "ohttp_relay")?,
    })
}

/// A malformed source is refused, not skipped: skipping it would quietly
/// leave the relay without its anchor, or pick another relay (DC-01).
pub fn resolve(sources: RelaySources) -> Result<RelaySetting, String> {
    if let Some(file) = sources.setting_file {
        return from_setting_file(&file);
    }
    if let Some(url) = present(sources.legacy_url_file) {
        return Ok(RelaySetting {
            url,
            anchor: None,
            ohttp_relay: None,
        });
    }
    let anchor = present(sources.env_anchor);
    let ohttp_relay = present(sources.env_ohttp_relay);
    match present(sources.env_url) {
        Some(url) => Ok(RelaySetting {
            url,
            anchor: anchor.as_deref().map(anchor_from_hex).transpose()?,
            ohttp_relay,
        }),
        None if anchor.is_some() || ohttp_relay.is_some() => Err(
            "VAUCHI_RELAY_ANCHOR and VAUCHI_OHTTP_RELAY_URL describe the relay in VAUCHI_RELAY_URL, which is not set"
                .to_owned(),
        ),
        None => Ok(RelaySetting {
            url: PRODUCTION_RELAY_URL.to_owned(),
            anchor: None,
            ohttp_relay: None,
        }),
    }
}

impl RelaySetting {
    pub fn apply(&self, config: VauchiConfig) -> VauchiConfig {
        let config = match self.anchor {
            Some(anchor) => config.with_relay(self.url.clone(), anchor),
            None => config.with_relay_url(self.url.clone()),
        };
        match &self.ohttp_relay {
            Some(ohttp_relay) => config.with_ohttp_relay_url(ohttp_relay.clone()),
            None => config,
        }
    }

    /// Whether nothing will sync: Core takes no OHTTP key from this relay
    /// until the setting names its anchor.
    pub fn lacks_its_anchor(&self) -> bool {
        self.anchor.is_none() && RelayConfig::requires_ohttp_anchor(&self.url)
    }
}
