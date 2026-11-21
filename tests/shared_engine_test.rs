// SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me>
// SPDX-License-Identifier: GPL-3.0-or-later

use vauchi_app::ui::AppEngine;
use vauchi_contract::{Command, Event};
use vauchi_core::{api::Vauchi, settings::NetworkCost};
use vauchi_linux_app::engine::CoreEngine;
use vauchi_linux_hw::{Backends, HostDevices, LinuxHardware};

// @internal — the shared adapter forwards Core without a GTK trait import.
#[test]
fn shared_engine_forwards_initial_commands_and_platform_observations() {
    let engine = CoreEngine::new(AppEngine::new(Vauchi::in_memory().unwrap()));
    let mut reference = AppEngine::new(Vauchi::in_memory().unwrap());
    assert!(
        engine.initial_commands() == reference.initial_commands().unwrap(),
        "the shared adapter must return Core's complete initial batch"
    );
    let event = Event::NetworkCostChanged {
        cost: NetworkCost::Metered,
    };
    assert!(
        engine.dispatch(event.clone()) == reference.dispatch(event).unwrap(),
        "the shared adapter must return Core's complete observation response"
    );
}

// @internal — once a hardware shell has reported, Core keeps hardware
// commands out of its answer and queues them. The adapter hands them on, or
// the wakeup Core schedules never reaches the timer (vauchi/private#1297).
#[test]
fn the_hardware_commands_core_queues_come_with_its_answer() {
    let hardware = LinuxHardware::with_backends(Backends::default(), HostDevices::default())
        .expect("hardware without devices");
    let engine = CoreEngine::new(AppEngine::new(Vauchi::in_memory().unwrap()));
    engine.dispatch(hardware.report());

    let answer = engine.dispatch(Event::WakeupFired);

    let wakeups = answer
        .iter()
        .filter(|command| matches!(command, Command::ScheduleWakeup { .. }))
        .count();
    assert_eq!(wakeups, 1);
}
