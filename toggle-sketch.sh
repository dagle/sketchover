#!/bin/sh

# This is a script to use the toggle pass-through
# functionallity. You should bind this to key in
# your compositor. It's possible to lock up the
# system if not using this correctly.

# Check if sketchover is running
if pgrep -x "sketchover" >/dev/null; then
	# If it's running, we do we tell it to toggles pass-through
	# when pass-through is enabled, drawing is disabled but we can
	# interact with the windows behind it
	pkill -TSTP sketchover
else
	sketchover
fi
