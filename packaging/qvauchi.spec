# SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me>
# SPDX-License-Identifier: GPL-3.0-or-later

# Its own spec, not a gvauchi subpackage: both install
# applications/app.vauchi.Vauchi.desktop with a different Exec, and one
# build root cannot hold two files at one path. Install gvauchi or
# qvauchi, not both: they share one data directory (private#1029).

Name:           qvauchi
Version:        0.11.0
Release:        1%{?dist}
Summary:        Privacy-focused updatable contact cards (Qt desktop)

License:        GPL-3.0-or-later AND AGPL-3.0-or-later AND OFL-1.1
URL:            https://vauchi.app
Source0:        %{name}-%{version}.tar.gz

BuildRequires:  cargo
BuildRequires:  rust
BuildRequires:  qt6-qtbase-devel
BuildRequires:  cmake
BuildRequires:  gcc-c++
BuildRequires:  alsa-lib-devel
BuildRequires:  pkgconfig(libpipewire-0.3) >= 0.3.77
BuildRequires:  dbus-devel
BuildRequires:  clang-devel
BuildRequires:  fontconfig-devel
BuildRequires:  qrencode-devel
BuildRequires:  pkgconfig

Requires:       qt6-qtbase
Requires:       pipewire >= 0.3.77
Recommends:     wireplumber
Requires:       qrencode-libs
Conflicts:      gvauchi
Obsoletes:      vauchi-qt < 0.6.0
Provides:       vauchi-qt = %{version}-%{release}

%description
Vauchi lets you exchange updatable contact cards in person.
End-to-end encrypted and decentralized. This package provides
qvauchi, the native Linux desktop client built with Qt 6. It replaces
the vauchi-qt package. Install gvauchi or qvauchi, not both: they
share one data directory.

%prep
%autosetup

%build
cargo build --release --no-default-features --features qt --bin qvauchi

%install
install -Dm 755 target/release/qvauchi \
    %{buildroot}%{_bindir}/qvauchi
install -d %{buildroot}%{_datadir}/applications
sed -e 's/^Exec=gvauchi /Exec=qvauchi /' -e '/^DBusActivatable=/d' \
    -e 's/^Categories=Network;ContactManagement;GTK;$/Categories=Network;ContactManagement;Qt;/' \
    data/app.vauchi.Vauchi.desktop \
    > %{buildroot}%{_datadir}/applications/app.vauchi.Vauchi.desktop
install -Dm 644 data/app.vauchi.Vauchi.metainfo.xml \
    %{buildroot}%{_datadir}/metainfo/app.vauchi.Vauchi.metainfo.xml
install -Dm 644 data/app.vauchi.Vauchi.svg \
    %{buildroot}%{_datadir}/icons/hicolor/scalable/apps/app.vauchi.Vauchi.svg
install -d %{buildroot}%{_datadir}/fonts/vauchi
install -m 644 data/fonts/*.ttf %{buildroot}%{_datadir}/fonts/vauchi/

%files
%license LICENSES/GPL-3.0-or-later.txt
%{_bindir}/qvauchi
%{_datadir}/applications/app.vauchi.Vauchi.desktop
%{_datadir}/metainfo/app.vauchi.Vauchi.metainfo.xml
%{_datadir}/icons/hicolor/scalable/apps/app.vauchi.Vauchi.svg
%{_datadir}/fonts/vauchi/

%changelog
* Thu Nov 26 2026 Mattia Egloff <mattia.egloff@pm.me> - 0.11.0-1
- Add PipeWire camera capture and live QR preview with portal remote handoff

* Wed Nov 11 2026 Mattia Egloff <mattia.egloff@pm.me> - 0.10.1-1
- First qvauchi package, built from linux-app: it replaces vauchi-qt
  and conflicts with gvauchi (private#1029)
