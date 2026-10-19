/**
 * Pass 4 — chat-agent QA against a real opencode agent over the nightly
 * server's /ws relay. Logs every frame to chat-agent-qa.jsonl and asserts:
 *   1. spawn_agent ok
 *   2. create_session ok (throwaway cwd)
 *   3. send_prompt (command_id === payload.turnId) resolves after end_turn
 *   4. message_chunk events arrived, role==='agent' text contains "OK"
 *   5. prompt_complete arrived
 *   6. close_session ok, kill_agent ok
 * Relay pings (type "ping" requests) are answered to survive pongTimeoutMs.
 */
import { randomBytes } from 'node:crypto'
import { appendFileSync, mkdirSync } from 'node:fs'
import net from 'node:net'

const PORT = 19617
const TOKEN = 'nightly-audit'
const PROTO = 7
const RUN = 'goal-routine-runs/20261019-auto2'
const LOG = `${RUN}/chat-agent-qa.jsonl`
const CWD = '/tmp/termul-qa-p4-cwd'
mkdirSync(CWD, { recursive: true })

class WsClient {
  constructor() {
    this.buffer = Buffer.alloc(0)
    this.frames = []
    this.waiters = []
    this.closed = false
  }
  // Events captured by drain() during request polling.
  captured = { chunks: [], promptComplete: null }
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
  drain() {
    // Log + react to every unclaimed frame (relay pings, session events).
    while (this.frames.length > 0) {
      const raw = this.frames.shift()
      let f
      try {
        f = JSON.parse(raw)
      } catch {
        continue
      }
      appendFileSync(LOG, JSON.stringify({ t: Date.now(), dir: 'in', ...f }) + '\n')
      // Capture session events (flat or batched) for assertions after the turn.
      const flat = f.type === 'events' && Array.isArray(f.events) ? f.events : [f]
      for (const ev of flat) {
        if (ev.type === 'message_chunk')
          this.captured.chunks.push({
            role: ev.payload?.role,
            text: ev.payload?.content?.text ?? '',
            seq: ev.seq
          })
        else if (ev.type === 'prompt_complete') this.captured.promptComplete = ev
      }
      if (f.type === 'ping') {
        // Relay keepalive request — answer so pongTimeoutMs never fires.
        this.sendText(JSON.stringify({ id: `pong-${Date.now()}`, type: 'ping', payload: {} }))
        appendFileSync(
          LOG,
          JSON.stringify({ t: Date.now(), dir: 'out', note: 'answered relay ping' }) + '\n'
        )
      }
    }
  }
  async request(type, payload, { timeoutMs = 120_000, commandId } = {}) {
    const id = `qa-${Math.random().toString(36).slice(2, 10)}`
    this.sendText(
      JSON.stringify({ id, type, payload, ...(commandId ? { command_id: commandId } : {}) })
    )
    appendFileSync(
      LOG,
      JSON.stringify({
        t: Date.now(),
        dir: 'out',
        id,
        type,
        payload,
        ...(commandId ? { command_id: commandId } : {})
      }) + '\n'
    )
    const deadline = Date.now() + timeoutMs
    for (;;) {
      const idx = this.frames.findIndex((f) => {
        try {
          const p = JSON.parse(f)
          return p.id === id
        } catch {
          return false
        }
      })
      if (idx !== -1) {
        const reply = JSON.parse(this.frames.splice(idx, 1)[0])
        appendFileSync(LOG, JSON.stringify({ t: Date.now(), dir: 'in', ...reply }) + '\n')
        return reply
      }
      this.drain()
      if (this.closed) throw new Error(`WS closed while awaiting reply to ${type}`)
      if (Date.now() > deadline) throw new Error(`timeout awaiting reply to ${type}`)
      await Promise.race([this.frameArrival(), new Promise((r) => setTimeout(r, 150))])
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
const log = (o) => appendFileSync(LOG, JSON.stringify({ t: Date.now(), dir: 'meta', ...o }) + '\n')
const assert = (cond, msg) => {
  log({ assert: msg, pass: Boolean(cond) })
  if (!cond) throw new Error(`ASSERT FAILED: ${msg}`)
}

async function main() {
  log({
    event: 'pass4-start',
    server: `ws://127.0.0.1:${PORT}/ws`,
    model: 'sepintu/moonshot/kimi-k3 (provider-configured)'
  })

  const health = await (await fetch(`http://127.0.0.1:${PORT}/health`)).json()
  log({ event: 'health', health })

  const ws = new WsClient()
  await ws.connect(`ws://127.0.0.1:${PORT}/ws`)
  // First frame after connect is auth_required (may share the handshake chunk);
  // WsClient routes leftovers through onData so it is logged by drain().
  const auth = await ws.request('authenticate', {
    token: TOKEN,
    protocolVersion: PROTO,
    deviceId: 'chat-agent-qa-device',
    clientInstanceId: `p4-${Date.now()}`
  })
  assert(auth.ok === true, 'authenticate ok')

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
    { commandId: `p4-spawn-${Date.now()}` }
  )
  assert(spawn.ok === true, 'spawn_agent ok')
  const agentId = spawn.payload.agentId
  log({ event: 'agent-spawned', agentId })

  const t0 = Date.now()
  const create = await ws.request(
    'create_session',
    { agentId, cwd: CWD },
    { commandId: `p4-cs-${Date.now()}` }
  )
  const createMs = Date.now() - t0
  assert(create.ok === true, 'create_session ok')
  const sessionId = create.payload.sessionId

  // Subscribe to the session's event stream (live-only) so message_chunk /
  // prompt_complete frames are forwarded to this connection.
  const sub = await ws.request('subscribe', { sessionId }, { timeoutMs: 30_000 })
  assert(sub.ok === true, 'subscribe ok')

  // send_prompt: command_id must equal payload.turnId; reply resolves after end_turn.
  const turnId = `p4-turn-${Date.now()}`
  const t1 = Date.now()
  const prompt = await ws.request(
    'send_prompt',
    { agentId, sessionId, turnId, text: 'Reply with exactly OK and nothing else.' },
    { commandId: turnId, timeoutMs: 180_000 }
  )
  const promptMs = Date.now() - t1
  assert(prompt.ok === true, `send_prompt ok (end_turn), ${promptMs} ms`)

  // Events were captured live by drain() while request() polled send_prompt.
  const chunks = ws.captured.chunks
  let promptComplete = ws.captured.promptComplete
  const remaining = []
  for (const raw of ws.frames.splice(0)) {
    try {
      remaining.push(JSON.parse(raw))
    } catch {}
  }
  for (const f of remaining) {
    appendFileSync(LOG, JSON.stringify({ t: Date.now(), dir: 'in', ...f }) + '\n')
    const flat = f.type === 'events' && Array.isArray(f.events) ? f.events : [f]
    for (const ev of flat) {
      if (ev.type === 'message_chunk')
        chunks.push({ role: ev.payload?.role, text: ev.payload?.content?.text ?? '', seq: ev.seq })
      else if (ev.type === 'prompt_complete') promptComplete = ev
    }
  }

  const agentText = chunks
    .filter((c) => c.role === 'agent')
    .map((c) => c.text)
    .join('')
  const thoughtText = chunks
    .filter((c) => c.role === 'thought')
    .map((c) => c.text)
    .join('')
  log({
    event: 'chunks-summary',
    nChunks: chunks.length,
    agentText: agentText.slice(0, 200),
    thoughtLen: thoughtText.length,
    promptComplete: promptComplete !== null,
    endTurnPayload: prompt.payload
  })
  assert(chunks.length > 0, 'message_chunk events arrived')
  assert(
    agentText.includes('OK'),
    `agent text contains OK (got: ${JSON.stringify(agentText.slice(0, 120))})`
  )
  assert(promptComplete !== null, 'prompt_complete arrived')
  assert(
    String(prompt.payload) === 'end_turn' || prompt.payload === 'end_turn' || endTurnSeen,
    `send_prompt reply resolves after end_turn (payload: ${JSON.stringify(prompt.payload).slice(0, 80)})`
  )

  const close = await ws.request(
    'close_session',
    { agentId, sessionId },
    { commandId: `p4-close-${Date.now()}` }
  )
  assert(close.ok === true, 'close_session ok')

  const kill = await ws.request('kill_agent', { agentId })
  assert(kill.ok === true, 'kill_agent ok')

  log({
    event: 'pass4-summary',
    spawn: 'ok',
    createMs,
    promptMs,
    agentText: agentText.slice(0, 120),
    chunks: chunks.length,
    promptComplete: true,
    close: 'ok',
    kill: 'ok',
    verdict: 'pass'
  })
  ws.drain()
  ws.close()
  console.log(
    JSON.stringify({ verdict: 'pass', createMs, promptMs, agentText: agentText.slice(0, 120) })
  )
}

main().catch((e) => {
  appendFileSync(
    LOG,
    JSON.stringify({ t: Date.now(), dir: 'meta', event: 'fatal', error: String(e) }) + '\n'
  )
  console.error(String(e))
  process.exit(1)
})
