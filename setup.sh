#!/bin/bash

### This script has to be run exactly once to setup the necessary groups and priorities the app will need.
### IT HAS TO BE RUN AS ROOT.

# Ensure the script is runnign as root.
if [[ $EUID -gt 0 ]]; then
  echo "Must run as root"
  exit 1
fi

# Define groups that will be created.
uinput_group="uinput"
app_group="daekey"

# Create them if they dont exist.
getent group "$uinput_group" || groupadd "$uinput_group"
getent group "$app_group" || groupadd "$app_group"

# TODO: Change group of uinput from root to uinput
