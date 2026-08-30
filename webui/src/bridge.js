import { exec as kernelSuExec, toast as kernelSuToast } from 'kernelsu'

const MODULE_SCRIPT = `if [ -f /data/adb/modules/keyforge/keyforge.sh ]; then _kf=/data/adb/modules/keyforge/keyforge.sh; elif [ -f /data/user_de/0/com.android.shell/axeron/plugins/keyforge/keyforge.sh ]; then _kf=/data/user_de/0/com.android.shell/axeron/plugins/keyforge/keyforge.sh; else echo 'KeyForge module script not found' >&2; exit 127; fi; sh "$_kf"`

function normalizeResult(result) {
  if (typeof result === 'string') {
    return { errno: 0, stdout: result, stderr: '' }
  }
  if (result && typeof result === 'object') {
    return {
      errno: Number(result.errno ?? result.code ?? 0),
      stdout: String(result.stdout ?? result.out ?? ''),
      stderr: String(result.stderr ?? result.error ?? ''),
    }
  }
  return { errno: 0, stdout: result == null ? '' : String(result), stderr: '' }
}

export function shellQuote(value) {
  return `'${String(value).replace(/'/g, `'"'"'`)}'`
}

export function hasRootBridge() {
  return Boolean(
    (globalThis.kernelsu && typeof globalThis.kernelsu.exec === 'function') ||
      (globalThis.ksu && typeof globalThis.ksu.exec === 'function'),
  )
}

export async function execRoot(command) {
  let raw
  if (globalThis.kernelsu && typeof globalThis.kernelsu.exec === 'function') {
    raw = await globalThis.kernelsu.exec(command)
  } else if (globalThis.ksu && typeof globalThis.ksu.exec === 'function') {
    raw = await kernelSuExec(command)
  } else {
    throw new Error('Root WebUI bridge unavailable. Open this page from AX Manager or KernelSU.')
  }

  const result = normalizeResult(raw)
  if (result.errno !== 0) {
    throw new Error(result.stderr.trim() || `Command failed with code ${result.errno}`)
  }
  return result.stdout
}

export function runScript(...args) {
  const suffix = args.length ? ` ${args.map(shellQuote).join(' ')}` : ''
  return execRoot(`${MODULE_SCRIPT}${suffix}`)
}

export function nativeToast(message) {
  try {
    if (globalThis.ksu && typeof globalThis.ksu.toast === 'function') {
      kernelSuToast(message)
    } else if (globalThis.kernelsu && typeof globalThis.kernelsu.toast === 'function') {
      globalThis.kernelsu.toast(message)
    }
  } catch {
    // The in-page snackbar remains the source of truth.
  }
}
