#!/usr/bin/env sh
# System libraries needed to build Glowfly on Debian/Ubuntu: windowing and input (eframe), the
# file dialog (rfd, GTK 3) and the desktop keyring (Secret Service over D-Bus).
set -e
sudo apt-get update
sudo apt-get install -y --no-install-recommends \
  pkg-config libgtk-3-dev libxkbcommon-dev libxcb-render0-dev libxcb-shape0-dev \
  libxcb-xfixes0-dev libwayland-dev libdbus-1-dev
