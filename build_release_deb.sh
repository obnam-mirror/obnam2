#!/bin/bash
#
# Build an Obnam release as a Debian .deb package. Usage:
#
#    ./build_release_deb.sh /workspace

set -euo pipefail

cleanup()
{
    rm -rf "$tmpdir"
}

if [ "$#" != 2 ]
then
    echo "ERROR: Usage: $0 TARGET-DIR GIT-TAG" 1>&2
    exit 1
fi

target="$(cd "$1"; pwd)"
tag="$2"

# Create a temporary directory and arrange for it to be deleted at the
# end.
tmpdir="$(mktemp -d)"
echo "$tmpdir"
trap cleanup EXIT

# Export the tag to a temporary source tarball.
git archive "$tag" | xz > "$tmpdir/src.tar.xz"

# Unpack the temporary source tarball to a new source tree.
mkdir "$tmpdir/src"
tar -C "$tmpdir/src" -xf "$tmpdir/src.tar.xz"

# Switch to the new source tree.
cd "$tmpdir/src"

# Get name and version of source package.
name="$(dpkg-parsechangelog -SSource)"
version="$(dpkg-parsechangelog -SVersion)"

# Get upstream version: everything before the last dash.
uv="$(echo "$version" | sed 's/-[^-]*$//')"
orig="${name}_${uv}.orig.tar.xz"

# Rename the source tarball to what dpkg-buildpackage wants.
mv ../src.tar.xz "../$orig"

# Build the package.
dpkg-buildpackage -us -uc

# Copy the results to the desired location.
cp ../*_* "$target"
