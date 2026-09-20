## Installing on Windows

64-bit Windows 10.0.17763 or later is required to run WezTerm; running on
earlier versions of Windows is not possible, as WezTerm requires [Pseudo
Console support that was first released in Windows
10.0.17763](https://devblogs.microsoft.com/commandline/windows-command-line-introducing-the-windows-pseudo-console-conpty/).

Windows is distributed as either a Setup executable or a portable ZIP.
Setup is the normal installation path; the ZIP requires no administrator
privileges. Both contain only `wezterm-gui.exe` and native WSL/ConPTY runtime
resources.

[:fontawesome-brands-windows: Windows Setup :material-tray-arrow-down:]({{ windows_exe_stable }}){ .md-button }
[:fontawesome-brands-windows: Nightly Windows Setup :material-tray-arrow-down:]({{ windows_exe_nightly }}){ .md-button }

[:fontawesome-brands-windows: Windows portable ZIP :material-tray-arrow-down:]({{ windows_zip_stable }}){ .md-button }
[:fontawesome-brands-windows: Nightly Windows portable ZIP :material-tray-arrow-down:]({{ windows_zip_nightly }}){ .md-button }

1. Download either the Setup executable or <a href="{{ windows_zip_stable }}">the release ZIP</a>
2. Run Setup, or extract the ZIP and run `wezterm-gui.exe`
3. Configure the WSL domain as described [here](../config/files.md)
