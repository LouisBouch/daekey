#!/bin/bash

# Run script from location of this script.
cd "$(dirname -- "${BASH_SOURCE[0]}")" || exit 1

cargo build --release
# TODO: cp the built binary to /usr/local/sbin/daekey
