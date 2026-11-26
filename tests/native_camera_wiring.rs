// SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me>
// SPDX-License-Identifier: GPL-3.0-or-later

#![cfg(feature = "camera-modules")]
use std::{fs::File, os::unix::fs::PermissionsExt, sync::Arc};
use vauchi_contract::{Command, Event};
use vauchi_linux_app::{engine::CommandEngine, wiring::HardwareBridge};
use vauchi_linux_hw::{
    Backends, HostDevices, LinuxHardware,
    camera_modules::{BackendChoice, CameraModules},
    frame_buffer::{FrameReader, FrameWriter},
    frame_directory::FrameResource,
};

struct Engine;
impl CommandEngine for Engine {
    fn dispatch(&self, _: Event) -> Vec<Command> {
        vec![]
    }
    fn initial_commands(&self) -> Vec<Command> {
        vec![]
    }
}

// @internal — verify the published hardware/display resource contract across SDKs.
#[cfg(feature = "gtk")]
#[test]
fn hardware_native_frames_are_consumed_by_the_installed_gtk_sdk() {
    use vauchi_linux_hw::camera_frame::{CameraFrame, PixelFormat};
    let writer = FrameWriter::initialize(tempfile::tempfile().unwrap()).unwrap();
    let frame = CameraFrame::from_raw(PixelFormat::Rgba, 2, 1, 8, 0, &[1, 2, 3, 255, 4, 5, 6, 255])
        .unwrap();
    let mut display =
        gtk_display::core_ui::camera_preview::NativePreviewReader::new(writer.reader_fd().unwrap())
            .unwrap();
    assert!(writer.write(&frame).unwrap());
    let painted = display.poll().unwrap().unwrap();
    assert_eq!(
        (painted.width(), painted.height()),
        (frame.width(), frame.height())
    );
    assert_eq!(painted.rgba(), frame.rgba());
    writer.close().unwrap();
    assert_eq!(display.poll().unwrap(), None);
    assert!(display.closed());
}

// @internal — hand off only this hardware instance's prepared native resource.
#[test]
fn native_resource_wiring_preserves_identity_and_transfers_a_read_only_descriptor() {
    let folder = tempfile::tempdir().unwrap();
    let provider = folder.path().join("vauchi-camera-v4l2");
    std::fs::write(&provider, "#!/bin/sh\nexit 0\n").unwrap();
    std::fs::set_permissions(provider, std::fs::Permissions::from_mode(0o755)).unwrap();
    let module = CameraModules::discover_in(folder.path(), BackendChoice::Direct, false).unwrap();
    let directory = module.directory();
    let writer = Arc::new(FrameWriter::initialize(tempfile::tempfile().unwrap()).unwrap());
    let lease = directory.publish("scan", writer).unwrap();
    let hardware = LinuxHardware::with_backends(
        Backends {
            camera_module: Some(module),
            ..Backends::default()
        },
        HostDevices::default(),
    )
    .unwrap();
    let bridge = HardwareBridge::new(Engine, hardware).unwrap();
    let prepared = bridge.frame_directory().unwrap();
    assert!(matches!(
        prepared.poll("foreign", 0).unwrap(),
        FrameResource::Absent
    ));
    let FrameResource::New { fd, generation } = prepared.poll("scan", 0).unwrap() else {
        panic!("resource not wired");
    };
    use std::os::unix::fs::FileExt;
    let file = File::from(fd);
    assert!(file.write_at(&[0], 0).is_err());
    let mut reader = FrameReader::new(file).unwrap();
    assert_eq!(reader.poll().unwrap(), None);
    assert!(matches!(
        prepared.poll("scan", generation).unwrap(),
        FrameResource::Unchanged
    ));
    drop(lease);
    reader.poll().unwrap();
    assert!(reader.closed());
    let empty = HardwareBridge::new(Engine, LinuxHardware::without_devices()).unwrap();
    assert!(empty.frame_directory().is_none());
}
