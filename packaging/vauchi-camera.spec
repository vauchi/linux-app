# SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me>
# SPDX-License-Identifier: GPL-3.0-or-later

Name:           vauchi-camera
Version:        0.11.0
Release:        1%{?dist}
Summary:        Optional Vauchi camera providers
License:        GPL-3.0-or-later AND AGPL-3.0-or-later
URL:            https://vauchi.app
Source0:        %{name}-%{version}.tar.gz
BuildRequires:  cargo
BuildRequires:  rust
BuildRequires:  gcc-c++
BuildRequires:  clang-devel
BuildRequires:  nasm
BuildRequires:  alsa-lib-devel
BuildRequires:  dbus-devel
BuildRequires:  pkgconfig(libpipewire-0.3) >= 0.3.77
BuildRequires:  pkgconfig

%description
Optional native camera capture, QR decoding and live preview for Vauchi.

%package -n vauchi-camera-pipewire
Summary:        Optional PipeWire camera provider for Vauchi
Recommends:     pipewire >= 0.3.77
Recommends:     wireplumber

%description -n vauchi-camera-pipewire
Capture and decode QR codes through PipeWire, with live preview in GTK or Qt.

%package -n vauchi-camera-v4l2
Summary:        Optional direct V4L2 camera provider for Vauchi

%description -n vauchi-camera-v4l2
Capture and decode QR codes directly through V4L2 without PipeWire, with live
preview in GTK or Qt. Set VAUCHI_CAMERA_BACKEND=direct to prefer this provider.

%prep
%autosetup

%build
cargo build --release --no-default-features --features capture-pipewire --bin vauchi-camera-pipewire
cargo build --release --no-default-features --features capture-v4l2 --bin vauchi-camera-v4l2

%install
install -Dm 755 target/release/vauchi-camera-pipewire \
    %{buildroot}%{_libexecdir}/vauchi/vauchi-camera-pipewire
install -Dm 755 target/release/vauchi-camera-v4l2 \
    %{buildroot}%{_libexecdir}/vauchi/vauchi-camera-v4l2

%files -n vauchi-camera-pipewire
%license LICENSES/GPL-3.0-or-later.txt
%{_libexecdir}/vauchi/vauchi-camera-pipewire

%files -n vauchi-camera-v4l2
%license LICENSES/GPL-3.0-or-later.txt
%{_libexecdir}/vauchi/vauchi-camera-v4l2
