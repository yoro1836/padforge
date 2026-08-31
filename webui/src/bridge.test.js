import assert from 'node:assert/strict'
import { spawnSync } from 'node:child_process'
import { chmodSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import test from 'node:test'

import {
  buildScriptCommand,
  escapeAxManagerCommand,
  execRoot,
  hasCommandBridge,
  shellQuote,
} from './bridge.js'

test('shellQuote preserves apostrophes', () => {
  assert.equal(shellQuote("it's ready"), "'it'\\''s ready'")
})

test('AxManager command escaping survives its nested sh wrapper', () => {
  const argument = 'runtime "$PATH" `tick` \\ slash it\'s'
  const innerCommand = `printf '%s\\n' ${shellQuote(argument)}`
  const outerCommand = `sh -c "${escapeAxManagerCommand(innerCommand)}"`
  const result = spawnSync('sh', ['-c', outerCommand], { encoding: 'utf8' })

  assert.equal(result.status, 0, result.stderr)
  assert.equal(result.stdout, `${argument}\n`)
})

test('buildScriptCommand runs runtime as an argument through AxManager', () => {
  const directory = mkdtempSync(join(tmpdir(), 'keyforge-bridge-'))
  const script = join(directory, 'keyforge.sh')
  writeFileSync(script, '#!/bin/sh\nprintf "argument=<%s>\\n" "$1"\n')
  chmodSync(script, 0o755)

  try {
    const axCommand = buildScriptCommand(['runtime'], true).replace(
      /\/data\/user_de\/0\/(?:com\.android\.shell|android)\/axeron\/plugins\/keyforge\/keyforge\.sh/g,
      script,
    )
    const outerCommand = `sh -c "${axCommand}"`
    const result = spawnSync('sh', ['-c', outerCommand], { encoding: 'utf8' })

    assert.equal(result.status, 0, result.stderr)
    assert.equal(result.stdout, 'argument=<runtime>\n')
  } finally {
    rmSync(directory, { recursive: true, force: true })
  }
})

test('native AxManager bridge works without the KernelSU compatibility bridge', async () => {
  const previousAxeron = globalThis.Axeron
  const previousKsu = globalThis.ksu
  const previousKernelSu = globalThis.kernelsu
  delete globalThis.ksu
  delete globalThis.kernelsu
  globalThis.Axeron = {
    exec: (command, options) => {
      assert.equal(command, 'status')
      assert.equal(options, '{}')
      return JSON.stringify({ errno: 0, stdout: 'running pid=42\n', stderr: '' })
    },
  }

  try {
    assert.equal(hasCommandBridge(), true)
    assert.equal(await execRoot('status'), 'running pid=42\n')
  } finally {
    if (previousAxeron === undefined) delete globalThis.Axeron
    else globalThis.Axeron = previousAxeron
    if (previousKsu === undefined) delete globalThis.ksu
    else globalThis.ksu = previousKsu
    if (previousKernelSu === undefined) delete globalThis.kernelsu
    else globalThis.kernelsu = previousKernelSu
  }
})

test('buildScriptCommand uses manager-specific module locations', () => {
  const axCommand = buildScriptCommand(['runtime'], true)
  assert.match(axCommand, /com\.android\.shell\/axeron\/plugins\/keyforge/)
  assert.match(axCommand, /user_de\/0\/android\/axeron\/plugins\/keyforge/)
  assert.doesNotMatch(axCommand, /\$_kf/)
  assert.equal((axCommand.match(/'runtime'/g) || []).length, 2)

  const rootCommand = buildScriptCommand(['runtime'], false)
  assert.match(rootCommand, /\/data\/adb\/modules\/keyforge\/keyforge\.sh 'runtime'/)
  assert.doesNotMatch(rootCommand, /axeron\/plugins/)
})
