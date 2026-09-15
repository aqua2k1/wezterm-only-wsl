## Installing on Windows

64-bit Windows 10.0.17763 or later is required to run WezTerm; running on
earlier versions of Windows is not possible, as WezTerm requires [Pseudo
Console support that was first released in Windows
10.0.17763](https://devblogs.microsoft.com/commandline/windows-command-line-introducing-the-windows-pseudo-console-conpty/).

Download the portable ZIP; it requires no installer or administrator
privileges. The archive contains only `wezterm-gui.exe` and the native WSL/
ConPTY runtime resources.

[:fontawesome-brands-windows: Windows (portable ZIP) :material-tray-arrow-down:]({{ windows_zip_stable }}){ .md-button }
[:fontawesome-brands-windows: Nightly Windows (portable ZIP) :material-tray-arrow-down:]({{ windows_zip_nightly }}){ .md-button }

1. Download <a href="{{ windows_zip_stable }}">the release ZIP</a>
2. Extract it and run `wezterm-gui.exe`
3. Configure the WSL domain as described [here](../config/files.md)
