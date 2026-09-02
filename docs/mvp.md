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
  <!-- 2026-08-31 개정: completion은 Petdex `waving` 표준인 0.70초로 바뀌었다. 규격대로
  그려진 남의 펫을 가져오면 4프레임 인사가 0.7초짜리인데 2.2초를 쥐면 세 번 반복된다.
  근거는 docs/state-contract.md, 당시 기준은 이 줄에 그대로 남긴다. -->
  승인된 idle silhouette로 복귀
- 사용자 피드백에 따라 Mochi도 FatMochi와 같은 7-row authored capability set으로 교체:
  idle blink, four-paw walk left/right, sleep breathing, caught/drag, feline stretch, landing/celebrate

2026-08-27 실사용 검토에서 이 첫 Mochi authored atlas는 승인되지 않았다. 생성 strip을
균등 분할하면서 다른 frame의 작은 조각이 함께 잘려 들어갔고, idle blink에는 eye-region
patch 합성이 사용됐다. 56 frame 중 22 frame에서 detached alpha component를 확인했다.
2026-08-29에 `hatch-pet` workflow로 9개 row를 한 줄씩 승인받아 다시 만든 8×9 standard
atlas가 그 자리를 대신했고, rejected asset은 저장소에서 제거했다. 새 Mochi에는 sit,
sleep, stretch row가 없어 `AnimationResolver`가 idle로 fallback한다. 그 세 동작은 다음
remaster 때 채운다.
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
  <!-- 2026-08-31 개정: 0.70초(Petdex `waving` 표준)로 변경. 위 주석 참조. -->
- 62개의 pure/transport test와 signed release app build를 통과한다.
- 실제 Claude와 Codex session에서 각각 start → tool use → completion을 한 번 체감 확인한다.

2026-08-28 실사용 확인을 마치고 사용자가 다음 단계 진행을 승인했다. gate 안에서 세 가지를
고쳤다. Claude handler를 Codex와 같은 command hook으로 바꿔 Roamling이 꺼져 있을 때 session
종료마다 노출되던 hook error를 없앴고, loopback request 상한을 256 KiB에서 1 MiB로 올려 큰
tool payload가 거절되지 않게 했으며, agent hook으로 이동하는 동안 idle frame으로 미끄러지던
animation 두 경로를 고쳤다. 1 MiB보다 큰 payload가 필요해지면 상한만 올려서는 안 된다는
점은 `docs/architecture.md`에 기록했다.

## Completed — MVP 3: It Watches Where You Work

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

2026-08-29에 사용자가 실사용에서 동작을 확인하고 다음 단계 진행을 승인했다. 정밀한
계측이 아닌 체감 확인이었으므로, MVP 4에서 배치가 어색하면 visual score뿐 아니라 caret
회피 쪽 원인도 함께 확인한다. gate 안에서 `MacWindowProvider`의 AX 승격을 마치며
`FocusSnapshot`의 `windowID`를 `windowFrame`으로 바꿨다. private API 없이 창을
`CGWindowNumber`로 매칭할 수 없어 id가 계속 nil이었기 때문이다.

## Completed — MVP 4: It Finds Empty Space

지금까지의 배치는 창 geometry만 안다. 창 안 어디가 비어 있는지는 모르므로 문서 한가운데,
코드 위, 버튼 위에 앉을 수 있다. 이 gate는 Screen Recording을 opt-in으로 켰을 때만 화면
스냅샷 한 장을 축소해 시각적으로 빈 영역을 찾는다.

내용을 읽지 않는다. OCR도 LLM도 쓰지 않고 edge density, local variance, temporal
stability만 본다. "글자를 읽어 피하는" 것이 아니라 "복잡한 곳을 피하는" 방식이며, 이
구분이 지금까지의 privacy 원칙과 이어지는 지점이다.

```text
ScreenCaptureKit single snapshot
        |
        v
   downsampled luminance field (OS 비의존 domain value)
        |
        v
CandidateScore =
  visualEmpty + caretDistance + controlDistance + edgePreference
  + stability + contextPreference + petComfort
  - pointerProximity - obstructionPenalty
```

`caretDistance`와 `controlDistance`는 MVP 3에서 이미 만들었고, `PositionCandidate`의
`visualEmptyScore` 자리도 비어 있는 채로 있다.

### In scope

- `MacCaptureProvider` — ScreenCaptureKit single snapshot과 권한 게이팅
- downsampled luminance field에서 빈 영역을 점수화하는 pure 함수
- `VisualSafeZoneProvider` — 그 점수를 후보 배치에 연결
- confidence가 낮으면 중앙 후보를 만들지 않고 window/display corner로 fallback
- 권한을 켤 때만 제시하는 설명과 요청, 거부·철회 시 MVP 3 경로로 복귀
- snapshot 비용이 pet frame timing을 끊지 않도록 하는 호출 시점 제한
- 창 하단 한 줄이 아니라 여러 행을 훑는 후보 sweep과 caret 진행 방향 회피
- 앉은 자리를 1초 주기로 재확인해 실제로 가려졌을 때만 옮기는 seat hold
- agent event마다 재계획하지 않는 hysteresis — 새 자리가 확실히 나을 때만 이동
- 좋은 자리에서 user idle이 이어지면 그 자리에서 그대로 잠드는 rest 경로
- wander 목적지도 emptiness로 거른다 — agent를 보지 않는 시간이 펫 일과의 대부분이다

### Explicitly out of scope for this gate

- OCR, 연속 capture, cloud vision, LLM 판단
- capture 이미지의 disk write 또는 log 기록
- game/media source — 별도 milestone
- `roamling.json` animation 저작 UI — MVP 5
- gallery/network pet installer

### Acceptance criteria

- Screen Recording 권한 없이 실행하면 MVP 3과 동일하게 동작한다.
- 권한 승인 후 문서 본문이나 조밀한 UI 위가 아니라 빈 영역에 앉는다.
- capture 이미지를 disk에 쓰지 않고 log/metadata에 남기지 않는다.
- 권한을 철회하면 다음 event부터 MVP 3 배치로 돌아간다.
- capture가 실패하거나 느려도 pet 동작이 끊기지 않는다.
- 같은 창을 보는 동안 agent event가 반복돼도 자리를 지키고, 가려졌을 때만 옮긴다.
- 입력이 없어도 화면이 바뀌어 펫이 덮이면 알아채고 비킨다. capture 1회가 62ms라
  주기가 곧 지연이다 — agent를 보는 중 3초, 배회 중 6초를 예산으로 잡는다. 캐럿을
  덮은 경우만 주기를 기다리지 않는다.
- 좋은 자리에 앉아 user idle이 이어지면 corner로 걸어가지 않고 그 자리에서 잔다.
- agent를 보고 있지 않은 평상시 배회에서도 본문 위에 앉지 않는다.
- 자는 동안 routine agent event로는 깨지 않고 결과·요청 event에만 깬다.
- 빈 영역 점수화를 pure test로 검증하고 전체 suite를 통과한다.

2026-09-02 사용자가 실사용에서 펫이 본문과 UI를 피해 앉는 것을 확인하고 다음 단계
진행을 승인했다. gate 안에서 계획과 갈린 곳이 셋이다. seat hold의 재확인 주기는 위
in-scope의 1초가 아니라 capture 1회 62ms 실측에 맞춘 3초(agent 관찰)·6초(배회·휴식)로
잡았다. 실사용에서 잡은 결함 둘 — 배회 중 졸던 자리를 두고 display corner까지 걸어가
자던 것, 자리가 글자로 덮여 비키려는 걸음을 곁에 있는 커서가 취소하던 것 — 은 capture가
검증한 자리에서만 그대로 자는 `napsInPlace`와 커서의 시선보다 우선하는 `.escape`
intent로 고쳤다(`docs/placement.md` 3.2.2). 마지막으로 "routine event로는 깨지 않는다"는
규칙이 `RoamlingRuntime`의 private 함수라 어떤 테스트도 지키지 않았는데, 닫으면서
`CompanionEventKind.wakesRestingPet`으로 Core에 옮겨 pure test가 잡게 했다. 앱의
두뇌가 macOS 모듈에 있어 검증이 닿지 않는 문제의 작은 사례이고, `docs/windows.md`의
B1이 같은 문제를 크게 가리킨다.

## Current gate — W2: 이미지 파이프라인 탈-CoreGraphics (`docs/windows.md`)

**MVP 사다리는 4에서 멈춘다.** 이후 게이트는 이 문서가 아니라 `docs/windows.md`의 W 사다리다.
W1(`RoamlingRuntime`을 AppKit 모듈 밖 `RoamlingEngine`으로 빼는 동작 변화 0의 리팩터)은
2026-09-02에 구현과 실사용 확인이 같은 날 끝나 닫혔다. 현재 게이트는 W2이고, 구현은 같은 날
끝나 실사용 확인만 남았다 — 정의와 실제로 실린 것은 그쪽 4절에 있다. 디코더(B3)는 W2b로
갈라 언어 결정과 같은 자리에 두었다.

MVP 5로 적어 두었던 "펫 저작 UI"는 이름만 있던 항목이라 여기서 정의하지 않는다. 필요해지는
시점에 — 사용자가 실제로 펫을 만들려 할 때 — 그때의 요구로 쓴다. game/media source와
gallery installer는 별개 milestone으로 남아 있고 사다리 밖이다.

## Exit rule

**리팩터 게이트(W1·W2)는 사용자가 달라진 점을 못 느껴야 통과한다.** 기존 테스트가 전부
통과하고, 서명 빌드를 실사용해서 배회 · 포인터 회피 · 잡기 · 드래그 · agent 착석 · 취침 ·
디스플레이 변경 · 권한 프롬프트 · scale 변경이 전과 같아야 다음으로 넘어간다. W2는 여기에
렌더된 프레임의 바이트 비교가 더해진다. 리팩터 중에는 동작·타이밍·기본값을 같이 고치지
않는다 — 섞이면 회귀가 리팩터 탓인지 튜닝 탓인지 구분할 수 없다.

MVP 4까지의 규칙도 그대로 선다. 사용자가 실사용으로 확인하기 전에는 다음 게이트로 넘어가지
않고, 권한을 주지 않은 사용자의 경험을 나쁘게 만드는 변경은 받지 않는다.
