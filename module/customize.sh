#!/system/bin/sh

if [ "${ARCH:-}" != "arm64" ]; then
    abort "KeyForge currently supports arm64 devices only"
fi

MANAGER="Magisk"
[ "${KSU:-}" = "true" ] && MANAGER="KernelSU"
ui_print "- Installing KeyForge for $MANAGER"
set_perm "$MODPATH/keyforge" 0 0 0755
set_perm "$MODPATH/keyforge.sh" 0 0 0755
set_perm "$MODPATH/service.sh" 0 0 0755
set_perm "$MODPATH/uninstall.sh" 0 0 0755
