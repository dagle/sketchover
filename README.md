# Sketchover

Sketchover is a small doodle program to draw on your foreground. The main idea
is to use if for presentation, streaming or anything like that. The intention
is to be enable what a very simple paint program can do in post production but
in real time.

Sketchover uses wayland and requires a wayland compositor to be run to work. It
uses wlr-layer-shell to draw on the foreground, so your compositor needs to
support it.

## Install

Atm it's not on cargo, so you will need to install it manually

cargo install --path .

## Usage

The keybindings are hardcoded atm.

Esc => quit

c => clear

u => undo last draw

n => next color

t => next tool

+/- => change width of pen
