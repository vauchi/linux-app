// SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me>
// SPDX-License-Identifier: GPL-3.0-or-later

use vauchi_app::ui::AppEngine;
use vauchi_contract::{Command, Event};
use vauchi_core::{api::Vauchi, settings::NetworkCost};
use vauchi_linux_app::engine::CoreEngine;

// @internal — the shared adapter forwards Core without a GTK trait import.
#[test]
fn shared_engine_forwards_initial_commands_and_platform_observations() {
    let engine = CoreEngine::new(AppEngine::new(Vauchi::in_memory().unwrap()));
    let commands = engine.initial_commands();
    assert_eq!(commands[0].variant_name(), "ReplaceSurface");
    assert_eq!(
        commands
            .iter()
            .filter(|c| matches!(c, Command::SetContextBar { .. }))
            .count(),
        1
    );
    assert_eq!(
        engine.dispatch(Event::NetworkCostChanged {
            cost: NetworkCost::Metered
        }),
        vec![]
    );
}
