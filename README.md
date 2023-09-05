# Sketchover

Sketchover is a small doodle program to draw directly on your screens
foreground. The main idea is to use it for presentation, streaming or anything
like that.

Sketchover uses wayland and requires a wayland compositor to be run to work. It
uses wlr-layer-shell to draw on the foreground, so your compositor needs to
support it.

## Install

Atm it's not a registered crate, so you will need to install it manually

cargo install --path .

## Usage

Esc => quit is hardcoded.

For keybindings and other option look in the config file

sketchover uses XDG_CONFIG_HOME for it's config files. Normally this is:

$HOME/.config/sketchover/default-config.toml

if the file doesn't exists, run sketchover and it will populate it with default values.
