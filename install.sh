#!/bin/bash

# Run script from location of this script.
cd "$(dirname -- "${BASH_SOURCE[0]}")" || exit 1

if [[ "$1" == "--debug" ]]; then
  target="debug"
  cargo build -p bindae || exit 1
else
  target="release"
  cargo build --release -p bindae || exit 1
fi


sudo cp target/"$target"/bindae /usr/local/sbin/
# User must restart the service to use the updated binary.
