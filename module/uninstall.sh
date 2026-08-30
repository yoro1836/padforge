#!/system/bin/sh
MODDIR="${0%/*}"

if [ -f "$MODDIR/keyforge-watch.pid" ]; then
    _wpid=""
    read -r _wpid < "$MODDIR/keyforge-watch.pid" 2>/dev/null || :
    [ -n "$_wpid" ] && kill "$_wpid" 2>/dev/null || :
    rm -f "$MODDIR/keyforge-watch.pid"
fi

if [ -f "$MODDIR/keyforge.sh" ]; then
    sh "$MODDIR/keyforge.sh" stop >/dev/null 2>&1
fi
rm -f "$MODDIR/.kf-request"
