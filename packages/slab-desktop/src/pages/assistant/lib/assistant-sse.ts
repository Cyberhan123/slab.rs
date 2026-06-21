export type AssistantSseMessage = {
  data: string
  id: string | null
}

type ReadAssistantSseStreamOptions = {
  lastEventId?: number | null
  onMessage: (message: AssistantSseMessage) => void
  onOpen?: () => void
  signal?: AbortSignal
}

export function isAbortError(error: unknown) {
  return error instanceof DOMException && error.name === 'AbortError'
}

export function nextReconnectDelayMs(attempt: number) {
  const baseDelayMs = Math.min(30_000, 1_000 * 2 ** Math.max(0, attempt))
  return baseDelayMs + Math.round(Math.random() * 300)
}

export async function readAssistantSseStream(
  url: string,
  { lastEventId, onMessage, onOpen, signal }: ReadAssistantSseStreamOptions
) {
  const headers = new Headers()
  if (typeof lastEventId === 'number' && Number.isFinite(lastEventId)) {
    headers.set('Last-Event-ID', String(lastEventId))
  }

  const response = await fetch(url, {
    headers,
    signal,
  })
  if (!response.ok) {
    throw new Error(`${response.status} ${response.statusText}`)
  }
  if (!response.body) {
    throw new Error('SSE response did not include a readable body.')
  }

  onOpen?.()

  const reader = response.body.getReader()
  const decoder = new TextDecoder()
  let buffer = ''
  let eventId: string | null = null
  let dataLines: string[] = []

  const flushEvent = () => {
    if (dataLines.length === 0) {
      eventId = null
      return
    }

    onMessage({
      data: dataLines.join('\n'),
      id: eventId,
    })
    eventId = null
    dataLines = []
  }

  const processLine = (line: string) => {
    if (line === '') {
      flushEvent()
      return
    }
    if (line.startsWith(':')) {
      return
    }

    const separatorIndex = line.indexOf(':')
    const field = separatorIndex >= 0 ? line.slice(0, separatorIndex) : line
    const rawValue = separatorIndex >= 0 ? line.slice(separatorIndex + 1) : ''
    const value = rawValue.startsWith(' ') ? rawValue.slice(1) : rawValue

    if (field === 'id') {
      eventId = value
      return
    }
    if (field === 'data') {
      dataLines.push(value)
    }
  }

  try {
    while (true) {
      // eslint-disable-next-line no-await-in-loop
      const { done, value } = await reader.read()
      if (done) {
        break
      }

      buffer += decoder.decode(value, { stream: true })
      let newlineIndex = buffer.search(/\r?\n/)
      while (newlineIndex >= 0) {
        const rawLine = buffer.slice(0, newlineIndex)
        const line = rawLine.endsWith('\r') ? rawLine.slice(0, -1) : rawLine
        const newlineLength = buffer[newlineIndex] === '\r' && buffer[newlineIndex + 1] === '\n' ? 2 : 1
        buffer = buffer.slice(newlineIndex + newlineLength)
        processLine(line)
        newlineIndex = buffer.search(/\r?\n/)
      }
    }

    buffer += decoder.decode()
    if (buffer.length > 0) {
      processLine(buffer.endsWith('\r') ? buffer.slice(0, -1) : buffer)
    }
    flushEvent()
  } finally {
    reader.releaseLock()
  }
}
