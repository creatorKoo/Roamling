# Roamling MVP gates

Roamling은 한 단계의 체감 품질과 acceptance criteria를 닫고 실제 사용 피드백을 받은
뒤 다음 단계로 이동한다. 다음 단계의 구조를 고려할 수는 있지만 기능을 미리 활성화하지
않는다.

## Current gate — MVP 0 + MVP 0.5 refinement

현재 목표는 **화면에 살아 있는 펫**과 **pointer interaction**의 품질을 다듬는 것이다.
AI/agent integration이 아니라 아래 장면이 자연스러운지가 첫 검증 기준이다.

```text
idle pause -> wander -> pointer notice -> evade -> catch -> drag -> drop
                         |
                         +-> connected display transition
```

### In scope

- Petdex/Codex-compatible pet loading and animation fallback
- transparent, non-activating, mostly click-through overlay
- global logical-point desktop coordinates and display hot-plug handling
- random wandering across multiple displays
- 이동 사이의 짧고 충분한 idle pause
- pointer awareness, slow/fast evade, capped escape speed
- fast approach catch, forgiving catch window, sprite-sized hit region
- click, drag, drop, and cross-display dragging
- MVP 0.5 값을 live tuning하는 작은 behavior panel
- pure-logic tests for thresholds, routes, catch, and display transitions

`idle pause`는 현재 단계의 pacing이며 sleep이 아니다. idle animation만 재생하고 위치를
고르는 safe-zone logic은 실행하지 않는다.

### Explicitly out of scope for this gate

- sit, sleep, findSleepSpot, BasicSafeZoneProvider — MVP 0.7
- Claude Code/Codex ActivitySource — MVP 1+
- AttentionModel/ReactionPolicy의 실제 event wiring — MVP 2
- Accessibility, focus, caret tracking — MVP 3
- ScreenCaptureKit/VisualSafeZoneProvider — MVP 4
- enhanced Roamling-only pet animations — MVP 5

Core enum이나 protocol에 후속 개념이 존재하더라도 현재 runtime에서 활성화하지 않는다.

## Current hands-on feedback baseline

2026-08-25 첫 검증 환경:

- 3 displays: 중앙 외부 4K display를 확대된 logical resolution으로 사용, 왼쪽 1080p,
  16-inch MacBook Pro panel이 상단/측면으로 연결된 비대칭 배치
- roaming은 이동 비율이 너무 높고 조금 빠르게 느껴짐
- pointer evade 자체는 좋지만 더 먼 거리에서 먼저 알아보면 좋음
- pointer로 display 경계까지 밀었을 때 현재 display에 갇힘
- 자연 상태에서도 display exploration이 잘 느껴지지 않음
- trackpad fast approach catch가 보통 3~4회 필요함
- drag와 cross-display drop은 정상

Retina/해상도 혼합은 AppKit logical point로 정규화한다. backing scale은 render metadata로
유지하며 pointer threshold와 이동 속도를 backing pixel에 곱하지 않는다.

## Refinement acceptance criteria

- 한 번 이동한 뒤 눈에 띄는 idle 구간이 존재한다.
- 기본 walk speed가 기존 prototype보다 차분하다.
- 3-display 환경에서 가만히 두면 display를 실제로 탐험한다.
- 연결된 경계 쪽으로 계속 evade하면 이웃 display로 이동한다.
- 물리적으로 이어지지 않은 display gap에서는 임의 teleport하지 않는다.
- trackpad fast approach 후 1~2회 안에 안정적으로 잡을 수 있다.
- catch가 아닌 동안 underlying UI click-through가 유지된다.
- tuning panel에서 speed, pause, exploration, pointer distance, catch window,
  catch threshold, hit region을 실행 중 조절하고 기본값으로 복구할 수 있다.
- 기존 drag/drop과 display-change 동작이 회귀하지 않는다.

## Exit rule

위 항목의 자동 검증과 실제 3-display 재검증이 끝나기 전에는 MVP 0.7 sleep 구현으로
넘어가지 않는다. 다음 gate 진입은 hands-on feedback 뒤 명시적으로 결정한다.

## Implementation checkpoint — 2026-08-25

현재 feedback pass에 구현한 항목:

- 기본 walk speed 40pt/s, 이동 후 randomized idle 약 8.4–17.4초
- 같은 display의 지나치게 긴 wander leg 제한
- 다른 display 방문 기본 확률 46%, target 경계 가까운 도착점 선택
- 220pt pointer notice, trackpad 친화적인 catch radius/speed/window
- 1.12배 hit ellipse와 catch arm 중 60Hz input sampling
- 실제로 맞닿은 display seam의 방향성 evade transition
- movement/pointer/catch 값을 live 적용하고 저장하는 `Behavior Tuning…` 창
- asymmetric three-display topology와 disconnected-gap 회귀 테스트

pure test/build 검증 뒤에도 이 gate는 사용자의 실제 3-display 재검증 전까지 열린 상태다.
재검증에서 값을 조절했다면 숫자 대신 tuning panel의 **Reset Defaults** 기준값과 달라진
항목만 기록하면 된다.
