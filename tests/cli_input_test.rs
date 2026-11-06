// SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me>
// SPDX-License-Identifier: GPL-3.0-or-later

#![cfg(feature = "cli")]

use std::{io::Write, os::unix::net::UnixStream, time::Duration};
use vauchi_linux_app::terminal_cli::{FdInput, InputChunk, LineInput};

// @internal — idle input must return without waiting for a newline or launching a reader thread.
#[test]
fn native_descriptor_distinguishes_idle_partial_data_and_eof() {
    let (reader, mut writer) = UnixStream::pair().unwrap();
    let mut input = FdInput::new(reader).unwrap();
    assert_eq!(input.read(Duration::ZERO).unwrap(), InputChunk::Idle);
    writer.write_all(b"partial").unwrap();
    assert_eq!(
        input.read(Duration::ZERO).unwrap(),
        InputChunk::Data(b"partial".to_vec())
    );
    drop(writer);
    assert_eq!(input.read(Duration::ZERO).unwrap(), InputChunk::End);
    input.restore().unwrap();
    assert_eq!(input.read(Duration::ZERO).unwrap(), InputChunk::End);
}
