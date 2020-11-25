#!/bin/bash

set -euo pipefail

chunkdir="$1"
bin="$2"

cleanup()
{
    echo "emptying $chunkdir" 1>&2
    find "$chunkdir" -mindepth 1 -delete
}

cleanup

echo "running benchmarks for various sizes"
for n in 1 10 100 1000 10000 100000 1000000
do
    echo "size $n" 1>&2
    for prog in benchmark-null benchmark-index benchmark-store benchmark-indexedstore
    do
	/usr/bin/time --format "$prog $n %e" "$bin/$prog" "$chunkdir" "$n" 2>&1
	cleanup
    done
done | awk '{ printf "%-30s %10s %10s\n", $1, $2, $3 }'
