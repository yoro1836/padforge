#!/system/bin/sh
# Late-start entry point: boot the daemon and a supervisor so the daemon
# always lives outside the WebUI app's process tree.

MODDIR="${0%/*}"

sh "$MODDIR/keyforge.sh" start >/dev/null 2>&1
nohup sh "$MODDIR/keyforge.sh" watch >/dev/null 2>&1 &
