Name:           velox
Version:        0.2.4
Release:        1%{?dist}
Summary:        Fast, modern, lightweight terminal emulator for Linux

License:        MIT
URL:            https://github.com/lnoxsian/Velox
Source0:        https://github.com/lnoxsian/Velox/archive/refs/tags/v%{version}.tar.gz

BuildRequires:  cargo >= 1.80
BuildRequires:  rust >= 1.80
BuildRequires:  python3

Requires:       glibc
Requires:       libgcc
Recommends:     fontconfig
Recommends:     hicolor-icon-theme

%description
Velox is a high-performance native Linux terminal emulator written in
pure Rust featuring a custom multi-threaded software renderer, sub-millisecond
input latency, truecolor, full Unicode/glyph fallback, and zero C library
runtime dependencies.

%prep
%autosetup -n Velox-%{version}

%build
cargo build --profile optimized-release --locked

%install
rm -rf %{buildroot}
install -D -p -m 0755 target/optimized-release/velox %{buildroot}%{_bindir}/velox
install -D -p -m 0644 assets/io.github.lnoxsian.Velox.desktop %{buildroot}%{_datadir}/applications/io.github.lnoxsian.Velox.desktop

for size in 16x16 32x32 48x48 64x64 128x128 256x256 512x512 1024x1024; do
    if [ -f "assets/generated_icons/icon_${size}.png" ]; then
        install -D -p -m 0644 "assets/generated_icons/icon_${size}.png" \
            "%{buildroot}%{_datadir}/icons/hicolor/${size}/apps/io.github.lnoxsian.Velox.png"
    fi
done

if [ -f "assets/icons/velox_terminal_icon_final.svg" ]; then
    install -D -p -m 0644 assets/icons/velox_terminal_icon_final.svg \
        %{buildroot}%{_datadir}/icons/hicolor/scalable/apps/io.github.lnoxsian.Velox.svg
fi

install -D -p -m 0644 README.md %{buildroot}%{_docdir}/%{name}/README.md
install -D -p -m 0644 LICENSE %{buildroot}%{_licensesdir}/%{name}/LICENSE

%check
cargo test --release --locked

%post
/bin/touch --no-create %{_datadir}/icons/hicolor &>/dev/null || :
/usr/bin/update-desktop-database &>/dev/null || :

%postun
if [ $1 -eq 0 ] ; then
    /bin/touch --no-create %{_datadir}/icons/hicolor &>/dev/null
    /usr/bin/gtk-update-icon-cache %{_datadir}/icons/hicolor &>/dev/null || :
    /usr/bin/update-desktop-database &>/dev/null || :
fi

%posttrans
/usr/bin/gtk-update-icon-cache %{_datadir}/icons/hicolor &>/dev/null || :

%files
%license LICENSE
%doc README.md
%{_bindir}/velox
%{_datadir}/applications/io.github.lnoxsian.Velox.desktop
%{_datadir}/icons/hicolor/*/apps/io.github.lnoxsian.Velox.*

%changelog
* Mon Oct 14 2026 lnoxsian <https://github.com/lnoxsian/Velox> - 0.2.4-1
- Initial release and packaging for Velox v0.2.4
