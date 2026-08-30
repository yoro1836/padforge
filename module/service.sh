#!/system/bin/sh
# Late-start entry point. The daemon double-forks itself into the init
# ownership, so a plain foreground call is all that is needed here.

MODDIR="${0%/*}"

sh "$MODDIR/keyforge.sh" start
