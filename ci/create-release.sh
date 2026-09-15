#!/usr/bin/env bash
set -x
name="$1"

notes=$(cat <<EOT
See https://wezterm.org/changelog.html#$name for the changelog

For the supported portable Windows build and installation notes, see:

[Windows](https://wezterm.org/install/windows.html)
EOT
)

gh release view "$name" || gh release create --prerelease --notes "$notes" --title "$name" "$name"
