#!/usr/bin/env bash
# Works out the version of this build and prints it as key=value lines for
# $GITHUB_OUTPUT (version, tag, title, channel, prerelease, latest).
#
#   vX.Y    Official release. Made by running the "build" workflow by hand on
#           main with "official" ticked; the next one after vX.Y is vX.(Y+1)
#           unless a version is typed in.
#   vX.Y.Z  Beta. Every other change on main: Z counts the changes on main
#           since the official release vX.Y, so v0.2 is followed by v0.2.1,
#           v0.2.2, ... Before the first official release, betas keep the
#           old numbering, v0.1.<run number>.
#
# Order: 0.2 < 0.2.1 < 0.2.2 < 0.3. Needs the full history and tags.
#
# Usage: version.sh beta|official [X.Y]
set -euo pipefail

kind="${1:-beta}"
wanted="${2:-}"
wanted="${wanted#v}"

# Every release tag without its "v", oldest version first.
tags=$(git tag --list 'v*' | sed 's/^v//' | { grep -E '^[0-9]+\.[0-9]+(\.[0-9]+)?$' || true; } | sort -V)
last=$(printf '%s\n' "$tags" | { grep -E '^[0-9]+\.[0-9]+$' || true; } | tail -n1)

if [ "$kind" = official ]; then
  if [ -n "$wanted" ]; then
    if ! [[ "$wanted" =~ ^[0-9]+\.[0-9]+$ ]]; then
      echo "::error::Official versions look like 1.0 or 0.3, not '$wanted'" >&2
      exit 1
    fi
    version="$wanted"
  elif [ -n "$last" ]; then
    IFS=. read -r x y <<<"$last"
    version="$x.$((y + 1))"
  else
    version="0.2"
  fi
  # It must come after every earlier release, betas included (0.1.17 < 0.2).
  newest=$(printf '%s\n%s\n' "$tags" "$version" | sed '/^$/d' | sort -V | tail -n1)
  if printf '%s\n' "$tags" | grep -qxF "$version" || [ "$newest" != "$version" ]; then
    echo "::error::v$version is not newer than every earlier release (newest is v$newest)" >&2
    exit 1
  fi
  title="Stream Sound v$version"
  channel=official
  prerelease=false
  latest=true
elif [ -n "$last" ]; then
  n=$(git rev-list --first-parent --count "v$last"..HEAD)
  if [ "$n" -eq 0 ]; then
    echo "::error::Nothing has changed since v$last" >&2
    exit 1
  fi
  version="$last.$n"
  title="Stream Sound v$version (Beta)"
  channel=beta
  prerelease=true
  latest=false
else
  # Installs from before version numbers compare only the last number of the
  # tag, so until the first official release betas stay above build 14 and
  # are marked latest, which is the only release those installs look at.
  version="0.1.${GITHUB_RUN_NUMBER:?}"
  title="Stream Sound v$version (Beta)"
  channel=beta
  prerelease=false
  latest=true
fi

echo "version=$version"
echo "tag=v$version"
echo "title=$title"
echo "channel=$channel"
echo "prerelease=$prerelease"
echo "latest=$latest"
