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

## Completed — MVP 0.7: It Sleeps

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
기본 캐릭터 FatMochi는 이 gate에서 실제 보이는 동작을 authored frame으로 다듬었다.
walk는 8장 동안 앞·뒤의 대각선 발이 교차하고 몸통과 꼬리가 작게 따라오며, idle blink,
sleep breathing, caught paw wiggle, drop landing, 고양이식 forward stretch도 각각 독립
frame을 쓴다. stretch는
앞발을 뻗고 가슴을 낮추며 엉덩이를 드는 자세이고 사람처럼 앞발을 위로 드는 pose는 쓰지
않는다. 당시 Mochi는 네 key pose에서 만든 reversible evaluation cycle을 유지했다.
active travel만 60Hz로 갱신하며 idle/sleep의 저전력 cadence는 그대로다. 이는 MVP 0.7의
creature-quality polish이며 이후 agent integration용 animation이나 source는 미리 만들지 않는다.

실사용 피드백 뒤 FatMochi의 승인된 idle을 캐릭터 기준으로 고정했다. walk는 idle의
174×170px silhouette과 같은 얼굴을 유지한 채 짧은 다리만 교차한다. 첫 walk frame은 idle
원화를 그대로 사용하고 이후 frame도 볼 아래가 좁아졌다가 몸통으로 이어지는 목·어깨선을
유지한다. alpha bounds center 편차를 2px 미만으로 고정해 atlas 안에서 몸이 뒤로 흐르다
되감기는 moonwalk도 막는다. 클릭 직후에는 idle과 같은 첫 frame에서 작은 앞발이 나타나는
caught intro를 한 번 재생한다. 실제 drag 중에는 이전의 배가 보이는 caught silhouette을
유지하고, 긴 팔만 몸 가까이 붙은 짧은 네 발로 바꾼 네 frame loop를 반복한다. 짧은
click도 같은 intro와 loop를 한 번 끝까지 보여 준 뒤 landing으로 이어지며, animation
동안 overlay는 즉시 click-through로 돌아간다. idle의 승인된 검은 눈과
half-close/closed blink frame 및 caught intro 32–35는 변경하지 않는다.

atlas 전체 frame을 alpha connected-component로 검사한다. 현재 mascot pose에는 몸에서
분리된 effect sprite가 없으므로 frame마다 visible artwork가 하나의 연결된 component여야
한다. 이 검사로 walk 합성 조각과 stretch/landing에 남은 cell-edge debris를 회귀 테스트한다.

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

2026-08-26 자동 검증, 실제 사용 확인, mascot 동작 보정을 마치고 사용자가 다음 단계
진행을 승인했다. click/drag pose의 남은 어색함은 알려진 visual polish 항목으로 기록하되
MVP 1의 activity integration 범위를 넓히지 않는다.

## Completed implementation — MVP 1: It Notices Work

첫 source로 현재 가장 안정적인 공식 lifecycle surface를 제공하는 Claude Code hooks를
구현했다. 설정 변경은 menu의 명시적 install 뒤에만 일어나며, 실제 사용자 session의
체감 검증은 MVP 2의 두-source 검증과 함께 수행한다. 사용자가 Claude와 Codex를 한 번에
진행하도록 승인했으므로 구현 gate를 닫고 다음 gate로 이동했다.

```text
Claude lifecycle hook
        |
        v
authenticated 127.0.0.1 receiver
        |
        v
CompanionEvent
        |
        +-- active ----------> wake / travel / work
        +-- permission -----> paw / observe
        +-- completed ------> small celebrate
        +-- failure --------> sad
        +-- session end ----> calm / free roam
```

### In scope

- menu bar에서 명시적으로 설치·복구·제거하는 Claude Code user hooks
- 기존 `~/.claude/settings.json` key와 다른 hook을 보존하는 idempotent merge
- 최초 변경 전 `settings.json.roamling-backup` 생성
- token으로 인증하고 `127.0.0.1:47831`에만 bind하는 HTTP receiver
- `SessionStart`, `UserPromptSubmit`, `PreToolUse`, `PostToolUse`,
  `PostToolUseFailure`, `PermissionRequest`, `Notification`, `Stop`,
  `StopFailure`, `SessionEnd` normalization
- prompt/tool input/output/transcript/source content를 model·metadata·log에 넣지 않음
- active app의 제목이나 내용을 읽지 않는 coarse window-bounds location hint
- sleep 중 activity가 오면 기존 wake/stretch를 마친 뒤 source로 이동
- work/attention/completion/failure의 MVP 1 reaction
- pointer evade, catch/drag가 activity travel보다 높은 우선순위 유지
- menu bar의 `Test Reaction` shortcut

### Explicitly out of scope for this gate

- Codex source와 여러 agent 사이의 선택 — MVP 2에서 구현
- `AttentionModel`/확률형 `ReactionPolicy` runtime wiring — MVP 2에서 구현
- Accessibility focused element/caret 추적 — MVP 3
- 정확한 terminal tab/session mapping
- Claude prompt, tool arguments, output, transcript 읽기
- 자동 hook 설치 또는 기존 settings 전체 덮어쓰기

### Acceptance criteria

- 설치 전에는 Claude settings를 변경하지 않는다.
- 설치/재설치를 반복해도 Roamling handler가 중복되지 않는다.
- 제거하면 Roamling handler만 사라지고 다른 settings/hook은 남는다.
- 잘못된 token과 malformed payload는 event를 만들지 않는다.
- Roamling이 꺼져 있거나 receiver가 실패해도 Claude 작업은 block되지 않는다.
- Claude 작업 시작 시 sleep을 깨우고, 가능한 경우 현재 작업 창의 보수적인 아래쪽
  edge로 자연스럽게 이동한 뒤 work/observe한다.
- permission request는 주의를 끌고, Stop은 작은 완료 반응, StopFailure는 실패 반응을 한다.
- 추가 macOS permission prompt 없이 동작한다.
- 전체 automated test suite를 통과한다.

## Completed — MVP 2: Claude + Codex

Codex 0.147.0의 stable hook registry를 사용해 Claude와 Codex를 같은 generic event path로
연결한다. `notify`나 app-server를 기본 transport로 쓰지 않는다. `notify`는 완료 event만
표현하고 사용자의 기존 command와 충돌할 수 있으며, app-server는 실행 중인 임의 session을
관찰하는 안정적인 attach surface가 아니기 때문이다.

```text
Claude command hooks ------+
                           |
Codex command hooks -------+--> authenticated loopback receivers
                                      |
                                      v
                                CompanionEvent
                                      |
                          AttentionModel + ReactionPolicy
                                      |
              travel / observe / work / paw / celebrate / sad
```

### In scope

- `~/.codex/hooks.json`에 명시적으로 설치·복구·제거하는 Codex user hooks
- 기존 Codex hooks와 `config.toml`/`notify`를 보존하는 idempotent merge
- 최초 변경 전 `hooks.json.roamling-backup` 생성
- stdin JSON을 `curl`로 authenticated `127.0.0.1:47832` receiver에 전달
- receiver가 없을 때 0.3초 안에 실패를 삼켜 Codex 작업을 방해하지 않는 command
- `SessionStart`, `UserPromptSubmit`, `PreToolUse`, `PostToolUse`,
  `PermissionRequest`, `Stop`, `SessionEnd` normalization
- Claude와 Codex event를 같은 `AttentionModel`에 넣는 multi-source selection
- minimum dwell, priority, hysteresis, revisit cooldown
- event kind/intensity/context/cooldown을 사용하는 `ReactionPolicy` runtime wiring
- routine low-intensity tool completion을 visible reaction/attention switch에서 제외
- meaningful completion은 확률에 묻히지 않고 최소 작은 축하로 acknowledge
- FatMochi와 Mochi 모두 completion state 전체 약 2.2초 동안 실제 frame motion을 유지하고
  승인된 idle silhouette로 복귀
- 사용자 피드백에 따라 Mochi도 FatMochi와 같은 7-row authored capability set으로 교체:
  idle blink, four-paw walk left/right, sleep breathing, caught/drag, feline stretch, landing/celebrate

2026-08-27 실사용 검토에서 이 첫 Mochi authored atlas는 승인되지 않았다. 생성 strip을
균등 분할하면서 다른 frame의 작은 조각이 함께 잘려 들어갔고, idle blink에는 eye-region
patch 합성이 사용됐다. 56 frame 중 22 frame에서 detached alpha component를 확인했다.
현재 asset은 replacement 전까지 개발 참고본이며, 이후 제작은 `docs/art/mochi-animation-handoff.md`
규격에 따라 frame마다 완전한 고양이를 그린 source만 받는다. 허용되는 local 처리는
complete-frame chroma removal, uniform scale/center, full-frame mirror, atlas packing뿐이다.
- source별 menu status, install/remove, `Test Reaction`
- 두 integration 모두 pointer evade/catch/drag보다 낮은 우선순위 유지

Paseo 0.5.2는 설치된 Claude Code/Codex CLI를 그대로 실행하므로 같은 user hook 설정을
사용한다. 2026-08-26에 Paseo daemon과 두 provider가 Ready인 상태, Roamling의 두 loopback
listener, Claude/Codex 완료 payload의 HTTP 204 수신을 확인했다. hook 설치 전에 이미 실행
중이던 agent에는 시작 event가 소급되지 않으므로 실제 체감 검증은 설치 후 새 Paseo agent로
수행한다.

### Explicitly out of scope for this gate

- Accessibility focused element/caret 추적 — MVP 3
- 정확한 terminal tab, IDE pane, Codex thread와 window의 mapping
- Codex app-server daemon lifecycle 소유 또는 실행 중 session attach
- prompt, tool input/output, transcript, assistant message 읽기
- hook trust를 자동 승인하거나 `--dangerously-bypass-hook-trust` 사용
- ScreenCaptureKit/visual safe-zone — MVP 4
- game/media source

### Acceptance criteria

- 설치 전에는 `~/.claude/settings.json`과 `~/.codex/hooks.json`을 변경하지 않는다.
- 재설치해도 Roamling handler가 중복되지 않고, 제거하면 자기 handler만 사라진다.
- 기존 Codex `notify`, `config.toml`, sibling hooks를 보존한다.
- Codex hook은 stdin payload를 loopback으로 전달하고 receiver가 없어도 agent를 block하지 않는다.
- 두 normalizer 모두 content-bearing field를 decode model/metadata/log에 남기지 않는다.
- active work를 보고 있을 때 다른 background work 때문에 target을 즉시 바꾸지 않는다.
- 다른 source의 permission request는 dwell을 깨고 관심을 가져갈 수 있다.
- routine tool completion마다 축하하거나 monitor를 왕복하지 않는다.
- Stop/completion은 intensity에 맞는 약 2.2초 반응을 하고 다음 active source가 있으면 이어서 본다.
- 62개의 pure/transport test와 signed release app build를 통과한다.
- 실제 Claude와 Codex session에서 각각 start → tool use → completion을 한 번 체감 확인한다.

2026-08-28 실사용 확인을 마치고 사용자가 다음 단계 진행을 승인했다. gate 안에서 세 가지를
고쳤다. Claude handler를 Codex와 같은 command hook으로 바꿔 Roamling이 꺼져 있을 때 session
종료마다 노출되던 hook error를 없앴고, loopback request 상한을 256 KiB에서 1 MiB로 올려 큰
tool payload가 거절되지 않게 했으며, agent hook으로 이동하는 동안 idle frame으로 미끄러지던
animation 두 경로를 고쳤다. 1 MiB보다 큰 payload가 필요해지면 상한만 올려서는 안 된다는
점은 `docs/architecture.md`에 기록했다.

## Current gate — MVP 3: It Watches Where You Work

MVP 1/2의 coarse window hint는 frontmost process의 layer-0 bounds만 알기 때문에 창 아래쪽
어딘가로만 갈 수 있었다. 이 gate는 Accessibility로 focused window, focused element, caret
위치를 얻어 펫이 실제 작업 지점 근처에 자리잡되 그 지점을 가리지 않게 한다.

permission은 사용자가 menu에서 켠 순간에만 설명과 함께 요청한다. 승인하지 않으면 MVP 2까지의
동작이 그대로 유지된다.

```text
AXUIElement
   |
   +-- focused window / element bounds
   +-- AXSelectedTextRange -> AXBoundsForRange -> caret rect
                |
                v
          FocusSnapshot (OS 비의존 domain value)
                |
                v
   BasicInterestPositionPlanner + BasicSafeZoneProvider
                |
                v
          caret을 피해 앉는 destination
```

### In scope

- `MacFocusProvider` — focused app/window/element bounds와 caret rect 조회
- `FocusSnapshot` domain 타입과 이를 obstacle로 받는 pure planner 확장
- `MacWindowProvider`의 AX detail 승격, 권한이 없으면 기존 `CGWindowList` coarse path 유지
- AX 경로일 때 `LocationHint.confidence` 상향
- menu에서 켤 때만 제시하는 permission 설명과 요청
- 권한 거부·철회 시 즉시 coarse path 복귀
- 개발과 배포 모두에서 TCC grant를 유지하는 stable code signing identity

### Explicitly out of scope for this gate

- ScreenCaptureKit / `VisualSafeZoneProvider` — MVP 4
- OCR, 연속 capture, cloud vision
- text 값, window title, document 내용 읽기
- 정확한 terminal tab, IDE pane mapping
- game/media source

### Acceptance criteria

- Accessibility 권한 없이 실행하면 MVP 2와 동일하게 동작한다.
- 권한 승인 후 caret과 focused control을 가리지 않는 위치에 앉는다.
- text 값, window title, document 내용을 model/metadata/log에 남기지 않는다.
- 권한을 철회하면 다음 event부터 coarse path로 돌아간다.
- AX 조회가 실패하거나 응답하지 않아도 pet 동작이 멈추지 않는다.
- planner 확장을 pure test로 검증하고 전체 suite를 통과한다.
- `ROAMLING_CODESIGN_IDENTITY`로 서명한 빌드에서 리빌드 후에도 권한이 유지된다.

## Exit rule

사용자가 실제 편집 session에서 펫이 caret을 가리지 않는지, 위치가 MVP 2보다 나아졌는지
확인하기 전에는 MVP 4 visual placement로 넘어가지 않는다. AX 조회 비용이나 응답 지연으로
펫 동작이 끊기는 문제는 MVP 3 안에서 수정한다. 권한을 주지 않은 사용자의 경험을 나쁘게
만드는 변경은 이 gate에서 받지 않는다.
