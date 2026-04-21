#!/bin/bash
# Extract release notes for a given version from CHANGELOG.md.
#
# Usage: extract-release-notes.sh <version>
#
# Inputs:
#   <version>  Semver string (e.g. "0.2.0"), without the leading "v".
#
# Outputs (written to the current directory):
#   release-title.txt  — One-line title: "v0.2.0 | <subtitle>".
#                        Subtitle is the first "> ..." line below the version
#                        header, if present; otherwise the bare tag.
#   release-notes.md   — Full release body: the CHANGELOG section for this
#                        version, followed by a common installation footer
#                        covering crates.io, PyPI, and the pre-built binaries.
#
# We keep the extraction pure-awk/sed (no Python or node runtime) so this can
# execute on any runner image before Rust / Python is installed.
#
# The CHANGELOG format we expect matches Keep a Changelog:
#
#   ## [0.2.0] - 2026-04-20
#   > Short one-line subtitle.
#
#   ### Added
#   - ...
#
#   ## [0.1.0] - 2026-04-01
#   ...

set -euo pipefail

VERSION="$1"
CHANGELOG="CHANGELOG.md"

if [ ! -f "$CHANGELOG" ]; then
  echo "Error: $CHANGELOG not found" >&2
  exit 1
fi

# Subtitle = first "> ..." line after the matching version header.
SUBTITLE=$(awk "/^## \[${VERSION}\]/{found=1; next} found && /^>/{gsub(/^> */, \"\"); print; exit}" "$CHANGELOG")

if [ -n "$SUBTITLE" ]; then
  echo "v${VERSION} | ${SUBTITLE}" > release-title.txt
else
  echo "v${VERSION}" > release-title.txt
fi

# Body = everything between this version's ## header and the next ## header.
# For the last (oldest) section, the awk scan runs to EOF, which includes
# Keep-a-Changelog's link-reference footer (``[0.1.0]: https://…``). Strip:
#   * the "> subtitle" lines (already captured by the title)
#   * the trailing link-reference lines ([tag]: url)
#   * the leading blank line awk leaves behind
awk "/^## \[${VERSION}\]/{flag=1; next} /^## \[/{flag=0} flag" "$CHANGELOG" \
  | sed '/^> /d' \
  | sed -E '/^\[[^]]+\]: /d' \
  | sed '1{/^$/d}' > changelog-section.md

# Trim trailing blank lines left after stripping the link references.
# ``sed -i '' ...`` isn't portable across GNU/BSD, so do it with a
# temp-file swap instead.
awk 'NF {p=1} p' changelog-section.md \
  | awk 'BEGIN{n=0} /^$/{buf[n++]=$0; next} {for(i=0;i<n;i++)print buf[i]; n=0; print}' \
  > changelog-section.md.trimmed
mv changelog-section.md.trimmed changelog-section.md

if [ ! -s changelog-section.md ]; then
  echo "Warning: No changelog content found for version ${VERSION}" >&2
fi

cat changelog-section.md > release-notes.md
cat >> release-notes.md << 'FOOTER'

---

### Installation

**Rust (crates.io)**
```bash
cargo add olga
```

**Python (PyPI)**
```bash
pip install olgadoc
```

**CLI — pre-built binaries**
Download the archive for your platform from the assets below, then move
`olga` somewhere on your `PATH`:

```bash
# Linux / macOS
tar -xzf olga-<platform>-<version>.tar.gz
sudo mv olga /usr/local/bin/

# Windows (PowerShell)
Expand-Archive olga-windows-x86_64-<version>.zip .
Move-Item olga.exe "$env:USERPROFILE\bin\"
```

**CLI — from crates.io**
```bash
cargo install olga --locked
```

### Platform support

| Platform | Architecture     | Archive                                    |
| -------- | ---------------- | ------------------------------------------ |
| Linux    | x86_64 (glibc)   | `olga-linux-x86_64-<version>.tar.gz`       |
| Linux    | x86_64 (musl)    | `olga-linux-x86_64-musl-<version>.tar.gz`  |
| Linux    | aarch64 (glibc)  | `olga-linux-aarch64-<version>.tar.gz`      |
| macOS    | x86_64 (Intel)   | `olga-macos-x86_64-<version>.tar.gz`       |
| macOS    | aarch64 (Apple)  | `olga-macos-aarch64-<version>.tar.gz`      |
| Windows  | x86_64           | `olga-windows-x86_64-<version>.zip`        |

Python wheels are published to PyPI for CPython 3.8+ on the same platforms
via [abi3](https://docs.python.org/3/c-api/stable.html); install via
`pip install olgadoc`.

### Changelog

See [CHANGELOG.md](https://github.com/Hugues-DTANKOUO/olga/blob/main/CHANGELOG.md) for full details.
FOOTER

rm -f changelog-section.md

echo "Generated release-title.txt and release-notes.md for v${VERSION}"
