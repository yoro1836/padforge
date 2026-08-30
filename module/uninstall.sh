#!/system/bin/sh
MODDIR="${0%/*}"

if [ -f "$MODDIR/keyforge.sh" ]; then
    sh "$MODDIR/keyforge.sh" stop >/dev/null 2>&1
fi
