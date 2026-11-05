// SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me>
// SPDX-License-Identifier: GPL-3.0-or-later

use vauchi_app::ui::AppEngine;
use vauchi_contract::Event;
use vauchi_core::{api::Vauchi, settings::NetworkCost};
use vauchi_linux_app::engine::CoreEngine;

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
