#!/bin/sh
#
# Build docs.

set -eu

sp-docgen obnam.md -o obnam.html
sp-docgen obnam.md -o obnam.pdf
