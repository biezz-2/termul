/**
 * #1391 regression hunt: boot nightly termul-server, spawn a real ACP agent
 * (opencode) over /ws, let it idle, and sample CPU of the server process.
 * Pre-fix symptom (issue #1391): idle agent busy-loop pinned one full core
 * of termul-server on Linux. PR #1398 (in dev, NOT in pinned nightly
 * a130218 per our compare) moved stdio reads to tokio so idle agents park.
 * The pinned nightly a130218 predates PR #1398, so an idle agent pinning
 * CPU is EXPECTED on this nightly; the fix ships in the next one.
 */

import { randomBytes } from 'node:crypto'
import { createRequire } from 'node:module'
import net from 'node:net'

const PORT = 19617
const TOKEN = 'nightly-audit'
const PROTO = 7
const REQ_TIMEOUT = 60_000

class WsClient {
  constructor() {
    this.buffer = Buffer.alloc(0)
    this.frames = []
    this.waiters = []
    this.closed = false
  }
  connect(url) {
    return new Promise((resolve, reject) => {
      const target = new URL(url)
      const key = randomBytes(16).toString('base64')
      const socket = net.connect({ host: target.hostname, port: Number(target.port) })
      this.socket = socket
      let handshake = Buffer.alloc(0)
      const onHandshake = (chunk) => {
        handshake = Buffer.concat([handshake, chunk])
        const idx = handshake.indexOf('\r\n\r\n')
        if (idx === -1) return
        const head = handshake.subarray(0, idx).toString()
        if (!/^HTTP\/1\.1 101/.test(head)) {
          socket.destroy()
          reject(new Error(`WS handshake rejected: ${head.split('\r\n')[0]}`))
          return
        }
        socket.off('data', onHandshake)
        socket.on('data', (c) => this.onData(c))
        const rest = handshake.subarray(idx + 4)
        if (rest.length > 0) this.onData(rest)
        resolve()
      }
      socket.on('data', onHandshake)
      socket.once('error', (e) => reject(e))
      socket.write(
        `GET ${target.pathname} HTTP/1.1\r\nHost: ${target.host}\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Key: ${key}\r\nSec-WebSocket-Version: 13\r\n\r\n`
      )
    })
  }
  onData(chunk) {
    this.buffer = Buffer.concat([this.buffer, chunk])
    for (;;) {
      const frame = this.readFrame()
      if (!frame) return
      if (frame.opcode === 0x1) {
        this.frames.push(frame.payload.toString('utf8'))
        this.notify()
      } else if (frame.opcode === 0x8) {
        this.closed = true
        this.socket.destroy()
        this.notify()
        return
      } else if (frame.opcode === 0x9) this.writeFrame(0xa, frame.payload)
    }
  }
  readFrame() {
    const b = this.buffer
    if (b.length < 2) return null
    const len = b[1] & 0x7f
    let offset = 2
    let payloadLen = len
    if (len === 126) {
      if (b.length < 4) return null
      payloadLen = b.readUInt16BE(2)
      offset = 4
    } else if (len === 127) {
      if (b.length < 10) return null
      payloadLen = Number(b.readBigUInt64BE(2))
      offset = 10
    }
    const masked = (b[1] & 0x80) !== 0
    const maskLen = masked ? 4 : 0
    if (b.length < offset + maskLen + payloadLen) return null
    let payload = b.subarray(offset + maskLen, offset + maskLen + payloadLen)
    if (masked) {
      const mask = b.subarray(offset, offset + 4)
      const out = Buffer.allocUnsafe(payloadLen)
      for (let i = 0; i < payloadLen; i += 1) out[i] = payload[i] ^ mask[i & 3]
      payload = out
    }
    this.buffer = b.subarray(offset + maskLen + payloadLen)
    return { opcode: b[0] & 0x0f, payload }
  }
  writeFrame(opcode, payload) {
    const mask = randomBytes(4)
    const len = payload.length
    let header
    if (len < 126) {
      header = Buffer.alloc(2)
      header[1] = 0x80 | len
    } else if (len < 65536) {
      header = Buffer.alloc(4)
      header[1] = 0x80 | 126
      header.writeUInt16BE(len, 2)
    } else {
      header = Buffer.alloc(10)
      header[1] = 0x80 | 127
      header.writeBigUInt64BE(BigInt(len), 2)
    }
    header[0] = 0x80 | opcode
    const masked = Buffer.from(payload)
    for (let i = 0; i < masked.length; i += 1) masked[i] ^= mask[i & 3]
    this.socket.write(Buffer.concat([header, mask, masked]))
  }
  sendText(text) {
    this.writeFrame(0x1, Buffer.from(text, 'utf8'))
  }
  notify() {
    const waiters = this.waiters
    this.waiters = []
    for (const w of waiters) w()
  }
  frameArrival() {
    return new Promise((resolve) => {
      this.waiters.push(resolve)
    })
  }
  async request(type, payload, { timeoutMs = REQ_TIMEOUT, commandId } = {}) {
    const id = `probe-${Math.random().toString(36).slice(2, 10)}`
    this.sendText(
      JSON.stringify({ id, type, payload, ...(commandId ? { command_id: commandId } : {}) })
    )
    const deadline = Date.now() + timeoutMs
    for (;;) {
      const idx = this.frames.findIndex((f) => {
        try {
          return JSON.parse(f).id === id
        } catch {
          return false
        }
      })
      if (idx !== -1) return JSON.parse(this.frames.splice(idx, 1)[0])
      if (this.closed) throw new Error(`WS closed while awaiting reply to ${type}`)
      if (Date.now() > deadline) throw new Error(`timeout awaiting reply to ${type}`)
      await Promise.race([this.frameArrival(), new Promise((r) => setTimeout(r, 200))])
    }
  }
  async awaitEvent(pred, { timeoutMs = REQ_TIMEOUT } = {}) {
    const deadline = Date.now() + timeoutMs
    for (;;) {
      const idx = this.frames.findIndex((f) => {
        try {
          return pred(JSON.parse(f))
        } catch {
          return false
        }
      })
      if (idx !== -1) return JSON.parse(this.frames.splice(idx, 1)[0])
      if (this.closed) throw new Error('WS closed while awaiting event')
      if (Date.now() > deadline) throw new Error('timeout awaiting event')
      await Promise.race([this.frameArrival(), new Promise((r) => setTimeout(r, 200))])
    }
  }
  close() {
    if (!this.closed && this.socket) {
      try {
        this.writeFrame(0x8, Buffer.alloc(0))
      } catch {}
      this.socket.destroy()
      this.closed = true
    }
  }
}

const sleep = (ms) => new Promise((r) => setTimeout(r, ms))

async function readStatm(pid) {
  const { readFile } = await import('node:fs/promises')
  const s = (await readFile(`/proc/${pid}/stat`)).toString()
  const fields = s.slice(s.lastIndexOf(')') + 2).split(' ')
  return { utime: Number(fields[11]), stime: Number(fields[12]) }
}

async function cpuSample(pid) {
  const a = await readStatm(pid)
  await sleep(5000)
  const b = await readStatm(pid)
  const ticks = b.utime - a.utime + (b.stime - a.stime)
  return { ticks5s: ticks, pct: (ticks / 500) * 100 }
}

async function main() {
  const out = (o) => process.stdout.write(JSON.stringify(o) + '\n')
  const health = await (await fetch(`http://127.0.0.1:${PORT}/health`)).json()
  out({ event: 'health', health })

  const ws = new WsClient()
  await ws.connect(`ws://127.0.0.1:${PORT}/ws`)
  const auth = await ws.request('authenticate', {
    token: TOKEN,
    protocolVersion: PROTO,
    deviceId: 'regression-1391-device',
    clientInstanceId: `reg1391-${Date.now()}`
  })
  out({ event: 'authenticate', ok: auth.ok })

  const serverPid = 596711
  const baseline = await cpuSample(serverPid)
  out({ event: 'cpu-baseline', ...baseline })

  // Ledgered command on /ws uses command_id (snake_case).
  const cid = `reg1391-${Date.now()}`
  const spawn = await ws.request(
    'spawn_agent',
    {
      config: {
        configId: 'opencode-qa',
        name: 'opencode-qa',
        command: '/root/.local/state/termul/acp-registry-binaries/opencode/opencode',
        args: ['acp']
      }
    },
    { commandId: cid }
  )
  out({ event: 'spawn_agent', reply: spawn })

  const cwd = '/tmp/termul-qa-p3/cwd'
  await (await import('node:fs/promises')).mkdir(cwd, { recursive: true })
  const agentId = spawn.payload?.agentId
  const cs = await ws.request(
    'create_session',
    { agentId, cwd },
    { commandId: `reg1391-cs-${Date.now()}` }
  )
  out({ event: 'create_session', reply: { ok: cs.ok, sessionId: cs.payload?.sessionId } })

  // Let the agent fully settle: greeting turn done, nothing pending.
  await sleep(20_000)
  const idle1 = await cpuSample(serverPid)
  out({ event: 'cpu-idle-1', note: 'agent spawned, no prompt sent', ...idle1 })

  await sleep(20_000)
  const idle2 = await cpuSample(serverPid)
  out({ event: 'cpu-idle-2', ...idle2 })

  // CPU cores available for context
  out({ event: 'cores', nproc: (await import('node:os')).cpus().length })

  ws.close()
}
main().catch((e) => {
  process.stderr.write(String(e) + '\n')
  process.exit(1)
})
