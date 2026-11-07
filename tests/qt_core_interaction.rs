// SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me>
// SPDX-License-Identifier: GPL-3.0-or-later

use std::{cell::RefCell, rc::Rc};
use vauchi_app::ui::AppEngine;
use vauchi_contract::{Command, Event};
use vauchi_core::api::Vauchi;
use vauchi_linux_app::{engine::CommandEngine, qt::QtHost, wiring::HardwareBridge};
use vauchi_linux_hw::{Backends, HostDevices, LinuxHardware};

struct CheckedCore {
    engine: RefCell<AppEngine>,
    errors: Rc<RefCell<Vec<String>>>,
}

impl CommandEngine for CheckedCore {
    fn dispatch(&self, event: Event) -> Vec<Command> {
        match self.engine.borrow_mut().dispatch(event) {
            Ok(commands) => commands,
            Err(error) => {
                self.errors.borrow_mut().push(error.kind().to_string());
                vec![]
            }
        }
    }
    fn initial_commands(&self) -> Vec<Command> {
        self.engine.borrow_mut().initial_commands().unwrap()
    }
}

// @scenario: generic_presentation_protocol.feature :: User interaction returns as an opaque event
fn main() {
    let errors = Rc::new(RefCell::new(vec![]));
    let core = CheckedCore {
        engine: RefCell::new(AppEngine::new(Vauchi::in_memory().unwrap())),
        errors: errors.clone(),
    };
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
    let host = Rc::new(RefCell::new(QtHost::new(
        HardwareBridge::new(core, hardware).unwrap(),
    )));
    let events = host.clone();
    let progressed = Rc::new(RefCell::new(false));
    let observed = progressed.clone();
    let args = vec![
        "qt-core-interaction".into(),
        "-platform".into(),
        "offscreen".into(),
    ];
    let exit = qt_display::run_with_poll(
        &args,
        Box::new(move |json| events.borrow_mut().on_event(json).unwrap()),
        Box::new(move || {
            let mut host = host.borrow_mut();
            let initial: Vec<Command> =
                serde_json::from_str(&host.poll().unwrap().unwrap()).unwrap();
            let surface = initial
                .iter()
                .find_map(|command| match command {
                    Command::ReplaceSurface { surface } => Some(surface.clone()),
                    _ => None,
                })
                .unwrap();
            let primary = initial
                .iter()
                .find_map(|command| match command {
                    Command::SetContextBar {
                        bar, surface_id, ..
                    } if *surface_id == surface.surface_id => bar.primary.clone(),
                    _ => None,
                })
                .unwrap();
            for event in [
                Event::SurfaceActivated {
                    surface_id: surface.surface_id.clone(),
                },
                Event::ActionActivated {
                    surface_id: surface.surface_id,
                    interaction_id: primary.interaction_id,
                },
            ] {
                host.on_event(&serde_json::to_string(&event).unwrap())
                    .unwrap();
            }
            let next: Vec<Command> = serde_json::from_str(&host.poll().unwrap().unwrap()).unwrap();
            *observed.borrow_mut() = next.iter().any(|command| matches!(
                command, Command::ReplaceSurface { surface: next } if next.title != surface.title
            ));
            qt_display::stop();
        }),
    );
    assert_eq!(exit, 0);
    assert!(
        errors.borrow().is_empty(),
        "Core rejected native interaction: {:?}",
        errors.borrow()
    );
    assert!(
        *progressed.borrow(),
        "welcome action must advance onboarding"
    );
}
