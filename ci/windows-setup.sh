#!/usr/bin/env bash
# Build the Windows Setup executable from the already-built native GUI.
set -euo pipefail

if ! command -v iscc.exe >/dev/null 2>&1; then
  echo "windows-setup.sh requires Inno Setup's iscc.exe" >&2
  exit 1
fi

TAG_NAME=${TAG_NAME:-$(git -c core.abbrev=8 show -s --format=%cd --date=format:%Y%m%d-%H%M%S)}
if [[ "${BUILD_REASON:-}" == "Schedule" ]]; then
  version=${TAG_NAME#nightly-}
  output="WezTerm-nightly-setup"
else
  version=${TAG_NAME#nightly-}
  output="WezTerm-${TAG_NAME}-setup"
fi

iscc.exe "-DMyAppVersion=${version}" "-F${output}" ci/windows-installer.iss
