import { describe, expect, it } from 'vitest'

import { dispatchA2uToolCall } from '../a2u-dispatcher'

describe('dispatchA2uToolCall', () => {
  it('maps workspace.open to a workspace surface reveal payload', () => {
    expect(dispatchA2uToolCall('workspace.open', '{"path":"src/main.rs"}')).toEqual({
      riskLevel: 'allow',
      surface: {
        type: 'workspace',
        payload: {
          revealPath: 'src/main.rs',
        },
      },
    })
  })

  it('maps plugin.launch as an ask-risk plugin surface', () => {
    const result = dispatchA2uToolCall(
      'plugin.launch',
      '{"plugin_id":"video-subtitle-translator","surface":"editor","payload":{"taskId":"task-1"}}'
    )

    expect(result?.riskLevel).toBe('ask')
    expect(result).toEqual({
      riskLevel: 'ask',
      surface: {
        type: 'plugin',
        payload: {
          pluginId: 'video-subtitle-translator',
          surface: 'editor',
          payload: {
            taskId: 'task-1',
          },
        },
      },
    })
  })

  it('leaves non-a2u tools on the ThoughtChain fallback path', () => {
    expect(dispatchA2uToolCall('read_file', '{"path":"src/main.rs"}')).toBeNull()
    expect(dispatchA2uToolCall('workspace.open', 'not-json')).toEqual({
      riskLevel: 'allow',
      surface: {
        type: 'workspace',
        payload: {
          revealPath: undefined,
        },
      },
    })
  })

  it('drops unsafe workspace paths before opening a2u surfaces', () => {
    expect(dispatchA2uToolCall('workspace.open', '{"path":"C:/Users/example/.ssh/id_rsa"}')).toEqual({
      riskLevel: 'allow',
      surface: {
        type: 'workspace',
        payload: {
          revealPath: undefined,
        },
      },
    })
    expect(dispatchA2uToolCall('review.show', '{"path":"../outside.rs","diff":"+ added"}')).toEqual({
      riskLevel: 'allow',
      surface: {
        type: 'review',
        payload: {
          diff: '+ added',
          path: undefined,
        },
      },
    })
  })
})
