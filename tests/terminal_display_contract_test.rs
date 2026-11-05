// SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me>
// SPDX-License-Identifier: GPL-3.0-or-later

use vauchi_app::ui::AppEngine;
use vauchi_contract::{Command, SurfaceSpec};
use vauchi_core::api::Vauchi;
use vauchi_linux_app::{engine::CoreEngine, wiring::HardwareBridge};
use vauchi_linux_hw::{Backends, HostDevices, LinuxHardware};

fn startup() -> (Vec<Command>, SurfaceSpec) {
    let mut engine = AppEngine::new(Vauchi::in_memory().unwrap());
    engine.bootstrap();
    let hardware = LinuxHardware::with_backends(
        Backends::default(),
        HostDevices {
            bluetooth_adapter: false,
            sound_card: false,
            video_device: false,
            nfc_reader: false,
        },
    )
    .unwrap();
    let mut bridge = HardwareBridge::new(CoreEngine::new(engine), hardware).unwrap();
    let commands = bridge.take_commands();
    let surface = commands
        .iter()
        .rev()
        .find_map(|command| match command {
            Command::ReplaceSurface { surface } => Some(surface.clone()),
            _ => None,
        })
        .expect("Core supplied a startup surface");
    (commands, surface)
}

// @internal — the released CLI display must consume the app's exact contract type.
#[test]
fn released_cli_accepts_the_apps_contract_commands() {
    let (commands, expected) = startup();
    let mut display = vauchi_linux_app::cli_display::presentation::PresentationState::default();
    let _effects = display.apply(&commands);
    assert_eq!(display.surface(), Some(&expected));
}

// @internal — the released TUI display must consume the app's exact contract type.
#[test]
fn released_tui_accepts_the_apps_contract_commands() {
    let (commands, expected) = startup();
    let mut display = vauchi_linux_app::tui_display::TerminalSession::default();
    let _effects = display.apply_commands(&commands);
    assert_eq!(display.presentation().surface(), Some(&expected));
}
