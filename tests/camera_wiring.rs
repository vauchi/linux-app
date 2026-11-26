// SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me>
// SPDX-License-Identifier: GPL-3.0-or-later

use std::sync::Arc;
use vauchi_contract::{Command, Event};
use vauchi_linux_app::{engine::CommandEngine, wiring::HardwareBridge};
use vauchi_linux_hw::{Backends, HostDevices, LinuxHardware, PipeWireCamera};

struct Engine;
impl CommandEngine for Engine {
    fn dispatch(&self, _: Event) -> Vec<Command> {
        vec![]
    }
    fn initial_commands(&self) -> Vec<Command> {
        vec![]
    }
}

// @internal — app wiring forwards resources without interpreting frames.
#[test]
fn the_app_exposes_only_its_hardware_instances_preview_directory() {
    let camera = Arc::new(PipeWireCamera::default());
    let directory = camera.preview_directory();
    let _lease = directory.publish("camera-1", 42, 1042, None).unwrap();
    let hardware = LinuxHardware::with_backends(
        Backends {
            camera: Some(camera),
            ..Backends::default()
        },
        HostDevices::default(),
    )
    .unwrap();
    let bridge = HardwareBridge::new(Engine, hardware).unwrap();
    let source = bridge
        .preview_directory()
        .unwrap()
        .resolve("camera-1")
        .unwrap()
        .unwrap();
    assert_eq!((source.node_id, source.serial), (42, 1042));
    assert!(source.remote_fd.is_none());
    assert!(
        bridge
            .preview_directory()
            .unwrap()
            .resolve("foreign")
            .unwrap()
            .is_none()
    );
    let other = HardwareBridge::new(Engine, LinuxHardware::without_devices()).unwrap();
    assert!(other.preview_directory().is_none());
}
