#!/bin/sh
set -eu

project_dir=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
build_dir=$(mktemp -d)
trap 'rm -rf "$build_dir"' EXIT HUP INT TERM

tar -C "$project_dir" \
    --exclude=target \
    --exclude=.git \
    --exclude='*.deb' \
    --exclude='*.buildinfo' \
    --exclude='*.changes' \
    -cf - . | tar -C "$build_dir" -xf -
cp -a "$build_dir/packaging/debian" "$build_dir/debian"

cd "$build_dir"
dpkg-buildpackage --build=binary --no-sign
cp ../appliance-display_*.deb "$project_dir/"
