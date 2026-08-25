# Roamling MVP gates

Roamling은 한 단계의 체감 품질과 acceptance criteria를 닫고 실제 사용 피드백을 받은
뒤 다음 단계로 이동한다. 다음 단계의 구조를 고려할 수는 있지만 기능을 미리 활성화하지
않는다.

## Completed — MVP 0 + MVP 0.5

2026-08-25 실제 3-display 환경에서 roaming, pointer evade, catch, drag/drop을 검증하고
사용자가 다음 단계 진행을 승인했다.

검증 환경과 확정값:

- 중앙 4K scaled display, 왼쪽 1080p display, 16-inch MacBook Pro display의 비대칭 배치
- walk 40pt/s, pause base 12초, other-display trip 46%
- pointer notice 170pt, catch radius 74pt, catch speed 380pt/s
- catch window 0.35초, hit region 1.12배
- 연결된 display seam evade와 cross-display drag 정상

이 gate의 값은 `Behavior Tuning…`의 **Reset Defaults** 기준이다.

## Current gate — MVP 0.7: It Sleeps

목표는 user input이 한동안 없을 때 Roamling이 방해되지 않는 위치를 찾아 쉬고, 다시
입력이 생기면 자연스럽게 깨어나는 것이다.

```text
user idle 75 s
      |
      v
     sit (2.4 s)
      |
      v
find basic safe zone
      |
      v
travel slowly -> sleep
                    |
            keyboard/pointer input
                    |
                    v
              wake -> stretch -> idle
```

### In scope

- permission-free system idle duration
- `sit -> findSleepSpot -> sleep -> wake -> stretch` behavior transitions
- current display를 우선하는 sleep placement
- `visibleFrame` 기반 menu bar/Dock exclusion
- display corner와 Dock-adjacent safe-zone candidates
- pointer와 가까운 candidate 회피
- Petdex pet의 `sitting`/`sleeping` capability fallback
- sleep 중 2Hz runtime cadence
- display hot-plug, pointer avoidance, catch/drag의 기존 동작 유지
- pure-logic tests for rest timing, placement, and behavior transitions

표준 pet에 sleep/sit animation이 없으면 pet 사용을 막지 않고 idle로 fallback한다.
기본 캐릭터 FatMochi는 현재 gate에서 실제 보이는 동작만 authored frame으로 다듬었다.
walk는 8장 동안 앞·뒤의 대각선 발이 교차하고 몸통과 꼬리가 작게 따라오며, idle blink,
sleep breathing, caught paw wiggle, drop landing, 고양이식 forward stretch도 각각 독립
frame을 쓴다. stretch는
앞발을 뻗고 가슴을 낮추며 엉덩이를 드는 자세이고 사람처럼 앞발을 위로 드는 pose는 쓰지
않는다. Mochi는 아직 네 key pose에서 만든 reversible evaluation cycle을 유지한다.
active travel만 60Hz로 갱신하며 idle/sleep의 저전력 cadence는 그대로다. 이는 MVP 0.7의
creature-quality polish이며 이후 agent integration용 animation이나 source는 미리 만들지 않는다.

실사용 피드백 뒤 FatMochi의 승인된 idle을 캐릭터 기준으로 고정했다. walk는 idle의
174×170px silhouette과 같은 얼굴을 유지한 채 짧은 다리만 교차한다. 첫 walk frame은 idle
원화를 그대로 사용하고 이후 frame도 볼 아래가 좁아졌다가 몸통으로 이어지는 목·어깨선을
유지한다. alpha bounds center 편차를 2px 미만으로 고정해 atlas 안에서 몸이 뒤로 흐르다
되감기는 moonwalk도 막는다. 클릭 직후에는 idle과 같은 첫 frame에서 작은 앞발이 나타나는
caught intro를 한 번 재생한다. 실제 drag 중에는 idle과 같은 짧은 팔다리 길이를 유지한
채 네 발의 대각선 쌍이 빠르게 교대하는 네 frame loop를 반복한다. 짧은 click도 같은
intro와 loop를 한 번 끝까지 보여 준 뒤 landing으로 이어지며, animation 동안 overlay는
즉시 click-through로 돌아간다. idle의 승인된 검은 눈과 half-close/closed blink frame은
변경하지 않는다.

### Explicitly out of scope for this gate

- Accessibility window/focus/caret tracking — MVP 3
- Claude Code/Codex ActivitySource — MVP 1+
- AttentionModel/ReactionPolicy event wiring — MVP 2
- ScreenCaptureKit/visual empty-region detection — MVP 4
- OCR, continuous capture, cloud vision
- window controls를 분석하는 advanced safe placement
- `roamling.json` enhanced animation authoring UI — MVP 5

### Acceptance criteria

- 약 75초 동안 keyboard/pointer input이 없으면 현재 wander를 정리하고 잠시 앉는다.
- 현재 display 안의 corner/Dock-adjacent 후보 중 pointer에서 떨어진 곳을 선택한다.
- sleep spot까지 기존 movement controller로 천천히 이동하며 순간이동하지 않는다.
- visible frame 밖이나 Dock/menu bar 위를 최종 위치로 선택하지 않는다.
- sleep animation이 없는 standard Petdex pet도 idle fallback으로 정상 동작한다.
- keyboard 또는 pointer input이 돌아오면 0.5초 안팎에 wake한다.
- nearby pointer/catch/drag는 sleep보다 높은 우선순위를 가진다.
- sleep 중 불필요한 render wakeup을 2Hz 수준으로 낮춘다.
- 추가 macOS permission prompt가 없다.
- menu bar의 `Stretch Now`로 75초 idle을 기다리지 않고 wake/stretch 품질을 확인할 수 있다.

정상 lifecycle에서 기지개는 75초 input idle 후 sit/safe-zone travel/sleep까지 들어간 다음,
keyboard 또는 pointer input으로 깨어날 때 재생된다. `Stretch Now`는 이 MVP 0.7 동작의
체감 검증 shortcut일 뿐 별도 behavior milestone을 앞당기지 않는다.

## Exit rule

자동 검증과 실제 사용 환경에서 sleep/wake의 timing 및 위치를 확인하기 전에는 MVP 1
agent integration으로 넘어가지 않는다. timing이나 위치가 어색하면 MVP 0.7 안에서만
조정한다.
