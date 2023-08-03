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

The keybindings are hardcoded atm.

Esc => quit

c => clear

u => undo last draw

n => next color

t => next tool

+/- => change width of pen

d => toggle distance
