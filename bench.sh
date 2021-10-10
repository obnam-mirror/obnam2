#!/bin/bash
#
# Run a simple benchmark of an initial backup of a sparse file of a
# given size. This mainly measures how fast the client can split the
# live data into chunks and compute checksums for the chunks.
#
# Edit the if-statement towards the end to get a flamegraph to see
# where time is actually spent.
#
# This is very simplistic and could do with a lot of improvement. But it's a start.

set -euo pipefail

SIZE=1G

TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

chunks="$TMP/chunks"
live="$TMP/live"

mkdir "$chunks"
mkdir "$live"
truncate --size "$SIZE" "$live/data.dat"

cat <<EOF >"$TMP/server.yaml"
address: localhost:8888
chunks: $chunks
tls_key: test.key
tls_cert: test.pem
EOF

cat <<EOF >"$TMP/client.yaml"
server_url: https://localhost:8888
verify_tls_cert: false
roots:
  - $live
log: $TMP/client.log
EOF

cargo build -q --release --all-targets

OBNAM_SERVER_LOG=error cargo run -q --release --bin obnam-server -- "$TMP/server.yaml" >/dev/null &
pid="$!"

cargo run -q --release --bin obnam -- --config "$TMP/client.yaml" init --insecure-passphrase=hunter2
if true; then
	/usr/bin/time cargo run -q --release --bin obnam -- --config "$TMP/client.yaml" backup >/dev/null
else
	cargo flamegraph --bin obnam -o obnam.svg -- --config "$TMP/client.yaml" backup >/dev/null
fi

kill -9 "$pid"
