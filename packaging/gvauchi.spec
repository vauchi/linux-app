# SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me>
# SPDX-License-Identifier: GPL-3.0-or-later

Name:           gvauchi
Version:        0.8.0
Release:        1%{?dist}
Summary:        Privacy-focused updatable contact cards (GTK desktop)

License:        GPL-3.0-or-later AND AGPL-3.0-or-later AND OFL-1.1
URL:            https://vauchi.app
Source0:        %{name}-%{version}.tar.gz

BuildRequires:  cargo
BuildRequires:  rust
BuildRequires:  gtk4-devel
BuildRequires:  alsa-lib-devel
BuildRequires:  dbus-devel
BuildRequires:  clang-devel
BuildRequires:  fontconfig-devel
BuildRequires:  libadwaita-devel
BuildRequires:  qrencode-devel
BuildRequires:  pkgconfig

Requires:       gtk4
Obsoletes:      vauchi-gtk < 0.8.0
Provides:       vauchi-gtk = %{version}-%{release}
Requires:       libadwaita
Requires:       qrencode-libs

%description
Vauchi lets you exchange updatable contact cards in person.
End-to-end encrypted and decentralized. This package provides
gvauchi, the native Linux desktop client built with GTK4 and
libadwaita. It replaces the vauchi-gtk package.

%prep
%autosetup

%build
cargo build --release --bin gvauchi

%install
install -Dm 755 target/release/gvauchi \
    %{buildroot}%{_bindir}/gvauchi
install -Dm 644 data/app.vauchi.Vauchi.desktop \
    %{buildroot}%{_datadir}/applications/app.vauchi.Vauchi.desktop
install -Dm 644 data/app.vauchi.Vauchi.service \
    %{buildroot}%{_datadir}/dbus-1/services/app.vauchi.Vauchi.service
install -Dm 644 data/app.vauchi.Vauchi.metainfo.xml \
    %{buildroot}%{_datadir}/metainfo/app.vauchi.Vauchi.metainfo.xml
install -Dm 644 data/app.vauchi.Vauchi.svg \
    %{buildroot}%{_datadir}/icons/hicolor/scalable/apps/app.vauchi.Vauchi.svg
install -d %{buildroot}%{_datadir}/fonts/vauchi
install -m 644 data/fonts/*.ttf %{buildroot}%{_datadir}/fonts/vauchi/

%files
%license LICENSES/GPL-3.0-or-later.txt
%{_bindir}/gvauchi
%{_datadir}/applications/app.vauchi.Vauchi.desktop
%{_datadir}/dbus-1/services/app.vauchi.Vauchi.service
%{_datadir}/metainfo/app.vauchi.Vauchi.metainfo.xml
%{_datadir}/icons/hicolor/scalable/apps/app.vauchi.Vauchi.svg
%{_datadir}/fonts/vauchi/

%changelog
* Tue Nov 03 2026 Mattia Egloff <mattia.egloff@pm.me> - 0.8.0-1
- First gvauchi package, built from linux-app: it replaces vauchi-gtk
  and installs the app icon and the brand fonts (private#1029)
