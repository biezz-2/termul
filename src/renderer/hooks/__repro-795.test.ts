import { renderHook } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import { flushDetachedOutputNow, useTerminalDetachedOutput } from '@/hooks/use-terminal-detached-output'
import { seedTerminalStore } from '@/lib/test-utils/store'
import { mockTerminal } from '@/lib/test-utils/terminal'
import { MAX_TRANSCRIPT_CHARS, useTerminalStore } from '@/stores/terminal-store'

// Real store, real hook; only the PTY event channel is faked.
let emitData: ((ptyId: string, data: Uint8Array) => void) | undefined

vi.mock('@/lib/api', () => ({
  terminalApi: {
    onData: (cb: (ptyId: string, data: Uint8Array) => void) => {
      emitData = cb
      return () => {}
    },
    onExit: () => () => {}
  }
}))

describe('integration: clipped head never reaches the transcript', () => {
  it('store transcript stays fragment-free after a surrogate-splitting clip', () => {
    seedTerminalStore([
      mockTerminal({
        id: 't1',
        name: 'Terminal 1',
        projectId: 'project-1',
        ptyId: 'pty-a',
        rendererAttachmentCount: 0
      })
    ])

    const { unmount } = renderHook(() => useTerminalDetachedOutput())

    const cap = MAX_TRANSCRIPT_CHARS
    // An oversized chunk whose cap-clip position splits the astral character
    // at positions 101/102: without alignment the buffered head begins with
    // the orphaned low surrogate, the stream has no line break for the
    // store's own trim to align to, and the delta lands at exactly cap chars
    // — so the transcript head stores an invalid partial character that a
    // cached-xterm replay then writes as replacement glyphs (#795).
    emitData?.(
      'pty-a',
      new TextEncoder().encode('x'.repeat(101) + '𝐀' + 'y'.repeat(cap - 1))
    )
    flushDetachedOutputNow()

    const transcript = useTerminalStore.getState().peekTranscript('pty-a')
    // The invariant that matters for #795: the transcript head is a complete
    // character, never an orphaned low surrogate left by the cap clip.
    const head = transcript.charCodeAt(0)
    expect(head < 0xdc00 || head > 0xdfff).toBe(true)
    unmount()
  })
})
