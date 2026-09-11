#!/bin/bash

### This script has to be run exactly once to setup the necessary groups and priorities the app will need.
### IT HAS TO BE RUN AS ROOT.

# Ensure the script is running as root.
if [[ $EUID -gt 0 ]]; then
  echo "Must run as root"
  exit 1
fi

echo -e "\033[33mWARNING:\033[0m If they already exist (they shouldn't), this script will overwrite '\033[1m/etc/modules-load.d/uinput.conf\033[0m' and '\033[1m/etc/udev/rules.d/uinput.rules\033[0m'"

while true; do
  read -p "Continue? " -r an
  case $an in
    [Yy]* ) break;;
    [Nn]* ) echo "exiting..."; exit;;
    * ) echo "Please answer yes or no";;
  esac
done

# Define groups that will be created.
uinput_group="uinput"
app_group="daekey"

echo -e "\033[33mCreating groups... \033[0m"
# Create them if they dont exist.
getent group "$uinput_group" || groupadd "$uinput_group"
getent group "$app_group" || groupadd "$app_group"

# Create the daemon user.
getent passwd "$app_group" || useradd --system -g "$app_group" --no-create-home --shell /bin/false "$app_group"

echo -e "\033[33mCreating rules... \033[0m"
# Ensure the uinput kernel module will load on boot.
echo uinput | tee /etc/modules-load.d/uinput.conf

# Load the uinput module now, in case it's not already running.
modprobe uinput

# Change group of uinput from root to uinput with correct permissions.
echo "SUBSYSTEM==\"misc\", KERNEL==\"uinput\", GROUP=\"$uinput_group\", MODE=\"0660\"" | tee /etc/udev/rules.d/uinput.rules

# Retrigger udev to apply new rules.
udevadm trigger --subsystem-match=misc --sysname-match=uinput


