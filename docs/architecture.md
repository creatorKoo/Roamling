# Roamling architecture

## Product boundary

Roamling은 agent status overlay가 아니라 desktop creature runtime이다. coding agent는
초기 관심 신호 중 하나일 뿐이다. 따라서 의존 방향은 항상 다음과 같다.

```text
External activity                       Operating system
Claude / Codex / future game/media      AppKit / AX / ScreenCaptureKit
          |                                      |
          v                                      v
    ActivitySource                        PlatformServices
          |                                      |
          +------------ domain values -----------+
                             |
                             v
       Context + Attention + Reaction + Behavior
                             |
                  Movement + DesktopWorld
                             |
                             v
                 PetCapabilities / Renderer
```

`RoamlingCore`의 compile-time dependency에는 OS framework나 product SDK가 없다.
현재 Swift가 pure core도 담지만, Windows port를 시작할 때 측정 결과가 필요하면 이
module만 추출할 수 있다. 선행 Rust/C ABI는 만들지 않는다.

## Modules

### RoamlingCore

- `Geometry`: `WorldPoint`, `WorldVector`, `WorldRect` (Double, top-left/y-down)
- `DesktopWorld`: immutable display/window/pointer/focus/safe-zone snapshot
- `DisplayTopology`: display graph, seam/portal, continuous route waypoints
- `MovementController`: acceleration, capped speed, arrival/deceleration
- `PointerInteractionModel`: proximity, approach speed, catch arming, escape vector
- `BehaviorController`: explicit creature FSM
- `BasicSafeZonePlanner`: permission-free rest candidates and destination scoring
- `RestConfiguration`: MVP 0.7 idle/sit/wake timing
- `CompanionEvent`, `UserContext`, `ActivitySource`
- `AttentionModel`, `ReactionPolicy`, candidate scoring
- platform protocols containing domain values only

### RoamlingPet

- `PetManifest`: Codex/Petdex package decode and validation
- `PetAsset`: atlas and animation tracks
- `PetCapabilities`: semantic actions used by behavior
- `AnimationResolver`: custom mapping -> aliases -> standard fallback
- `PetAnimationPlayer`: timing only; no source/platform knowledge
- `PetCatalog`: local read-only discovery
- `MascotPetFactory`: authored FatMochi atlas 선택 + Mochi key-pose evaluation fallback
- `PlaceholderPetFactory`: resource failure에도 앱이 뜨는 procedural emergency fallback

### RoamlingMac

- `MacDisplayProvider`: `NSScreen` -> display snapshots and coordinate transform
- `MacPointerProvider`: `NSEvent` global point sampling
- `MacUserIdleProvider`: elapsed time since any local input, without an event tap
- `MacBasicSafeZoneProvider`: visible-frame corner/Dock candidates
- `PetOverlayPanel`: transparent, non-activating, all-Spaces sprite window
- `RoamlingRuntime`: main-actor orchestration and the only owner of input gating
- menu-bar controls and lifecycle

MVP 3/4에서 `MacWindowProvider`, `MacFocusProvider`, `MacCaptureProvider`를 이 module에
추가한다.

## Domain events, not agent events

```swift
struct CompanionEvent {
    let sourceID: String
    let sourceType: ActivitySourceType
    let timestamp: TimeInterval
    let kind: CompanionEventKind
    let intensity: Double        // clamped 0...1
    let context: UserContext?
    let locationHint: LocationHint?
    let metadata: [String: ScalarMetadata]
}
```

`metadata`는 scalar만 허용하며 core가 그 key를 분기하지 않는다. source adapter가
Claude/Codex payload를 일반 event로 정규화한다. `ActivitySource`는 event stream을
내놓고 product-specific detail은 adapter 안에서 끝난다.

## Context

`UserContext`는 source와 독립적이다.

```text
signals from focus / idle / playback / future telemetry
                         |
                         v
                  ContextResolver
                         |
      working | gaming | watchingMedia | browsing | idle
```

MVP 0.7은 system-wide input idle duration을 rest trigger로만 사용한다. 이를 아직
`UserContext`로 승격하거나 coding activity와 합성하지 않는다. ContextResolver wiring은
activity source가 들어오는 후속 milestone에서 한다.
Context가 source 이름에서 직접 결정되지 않으므로, 같은 game event도 fullscreen,
focus, media playback 상황에 따라 다른 reaction budget을 가질 수 있다.

## AttentionModel

후보별 base priority의 초기값은 다음과 같다.

| event | priority |
| --- | ---: |
| attention required | 100 |
| negative/failure | 90 |
| major achievement | 80 |
| achievement/positive | 65 |
| active work | 50 |
| background/calm | 30 |
| idle | 0 |

최종 score는 priority, intensity, recency, location confidence를 합친다. 현재 target은
minimum dwell 동안 유지된다. 새 후보는 hysteresis margin을 넘을 때만 교체하고,
방금 떠난 source는 cooldown을 받는다. attention/failure처럼 urgent한 event만 dwell을
깨는 것이 가능하다. 이 구조가 monitor 왕복을 막는다.

## ReactionPolicy

policy 입력은 event kind/intensity, context, current behavior, cooldown, personality다.
출력은 `PetAction`이지 animation 이름이 아니다.

```text
achievement(0.2) -> glance / paw / small celebrate (weighted)
achievement(0.9) -> celebrate, occasionally large celebrate
setback          -> fail/sad, context에 따라 짧게
attention        -> observe/paw, 반복 cooldown 적용
```

random unit을 호출자가 주입할 수 있게 하여 테스트를 deterministic하게 만든다.
gaming/media context의 reaction budget은 낮추고 중앙 회피를 강화할 수 있다.

## Pet runtime and capability resolution

Behavior는 `perform(.sleep)`처럼 semantic capability만 요청한다.

```text
requested capability
    -> roamling.json explicit mapping
    -> manifest custom animation / aliases
    -> standard Petdex row
    -> related capability fallback
    -> idle
```

예:

```text
sleep      -> sleeping -> sleep -> idle
work       -> working -> running -> idle
observe    -> watching -> review -> waiting -> idle
celebrate  -> celebrate -> jumping -> waving -> idle
caught     -> caught -> waiting -> idle
```

v2 pointer look은 0°=up, 90°=right인 16방향 frame으로 quantize한다. deadzone에서는
idle을 사용한다. v1 pet은 look row가 없어도 idle/review fallback으로 정상 동작한다.

Petdex row는 완성 동작을 담는 frame slot이다. renderer가 한 장을 위아래로 흔들어 걷는
규격이 아니므로, 발·몸통·꼬리의 변화는 pet 제작자가 각 frame에 그린다. Roamling의
standard package loader도 atlas frame을 그대로 재생하며 임의 관절 animation을 합성하지
않는다. Built-in FatMochi의 7-row atlas는 현재 MVP에서 필요한 authored 동작을 담는 내부
resource이고, 외부 Petdex v1/v2 package 계약을 바꾸지 않는다.

### Optional Roamling extension

표준 package를 수정하지 않고 root에 `roamling.json`을 선택적으로 추가한다.

```json
{
  "schemaVersion": 1,
  "behaviors": {
    "sleep": "sleeping",
    "work": "typing",
    "observe": "watching",
    "paw": "pawing",
    "caught": "caught",
    "dragged": "dragged",
    "landing": "landing"
  }
}
```

unknown behavior/key는 무시한다. mapping이 가리킨 track이 없으면 표준 fallback으로
돌아간다. 이 파일이 없을 때가 가장 중요한 compatibility path다. schema version을
도입한 이유는 Petdex manifest를 fork하지 않고 확장을 독립적으로 진화시키기 위해서다.

## DesktopWorld and coordinates

Core world는 모든 display를 하나의 top-left/y-down logical-point plane으로 표현한다.
negative x/y도 허용한다. `NSScreen`, backing pixel, `CGDirectDisplayID`는 platform
snapshot 생성 뒤 core에 남지 않는다.

```text
Mac AppKit point (bottom-left/y-up)
               |
       DesktopCoordinateSpace
               |
Core WorldPoint (top-left/y-down)
```

`visibleFrame`은 wander destination과 drop clamp에, full `frame`은 display seam/path에
사용한다. display hot-plug notification을 받으면:

1. 새 snapshot과 transform을 만든다.
2. 현재 core point를 새 transform에 무작정 재해석하지 않고, 기존 AppKit screen point를
   새 world로 변환한다.
3. live display 밖이면 가장 가까운 visible frame으로 clamp한다.
4. active path를 취소하고 re-plan한다.

### Cross-display path

각 display는 graph node다. rectangle이 맞닿으면 overlap midpoint를 seam portal로,
떨어져 있으면 두 rectangle의 closest boundary points를 exit/entry portal로 쓴다.
route는 다음 waypoint를 가진다.

```text
current -> exit A -> entry B -> ... -> destination
```

gap에서도 좌표는 연속적으로 진행한다. 실제 panel은 물리적 display가 없는 구간에서
잠깐 보이지 않을 수 있지만 entry로 순간이동하지 않는다. 위/아래, 부분 seam, 음수
origin, disconnected layout을 같은 planner가 처리한다.

## Movement

복잡한 physics engine 대신 seek/arrival controller를 쓴다.

- 기본 wander speed는 40 logical pt/s이고 acceleration 제한이 있다.
- destination 근처에서는 `sqrt(2*a*d)` 기반으로 감속한다.
- animation direction은 velocity의 x sign으로 결정한다.
- evade speed도 hard cap을 가진다.
- pointer가 떠나면 즉시 무작위 이동으로 튀지 않고 짧게 안정된 뒤 wander한다.

MVP 0/0.5 기본 pacing은 한 이동 뒤 약 8.4–17.4초 idle이며, 같은 display의 한 leg는
최대 약 520pt로 제한한다. 다른 display를 선택할 기본 확률은 46%이고 목적지는 target
display 경계에서 너무 멀지 않은 곳으로 잡는다. 이 조합은 계속 걷는 인상을 줄이면서도
실제 display exploration이 보이게 한다. 이 idle은 sleep/safe-zone behavior가 아니다.

wander 목적지는 display edge/lower safe area에 편향시키되 hard-coded corner 하나가
아니다. Sleep destination도 `BasicSafeZonePlanner`의 candidate를 기존 movement planner에
넣으므로 별도 이동 시스템을 만들지 않는다.

## Behavior FSM

```text
idle <-> wander
  |        |
  +-> lookAtPointer -> evadePointer --+
                     |                 |
                     +-> caught -> dragged -> dropped -> idle

idle -> sit -> findSleepSpot -> sleep
  ^                              |
  +---- stretch <- wake <---------+ meaningful input

wake -> travelToInterest -> observe
                            | work / attention / celebrate / sad
```

MVP 0.7까지 idle, wander, look, evade, caught, dragged, dropped, sit, findSleepSpot,
sleep, wake, stretch를 실행한다. agent reaction state는 enum/event 경계에만 두고 source가
생기는 milestone에서 활성화한다. transition은 UI event handler에 흩어놓지 않고 pure
controller에서 검증한다.

## Pointer interaction and non-interference

초기 configuration(모두 settings로 이동 가능):

```text
distance > 170 pt       ignore
100...170 pt            look
50...100 pt             slow evade
< 50 pt                 faster evade
fast closing < 74 pt    arm catch for 0.35 s
```

단순 pointer speed가 아니라 이전 distance와 비교한 closing speed도 사용한다. 따라서
pet 근처에서 옆으로 빠르게 움직였다고 잡히지 않는다. evade velocity는 pet에서 pointer
반대 방향이며 cap을 넘지 않는다. 기본 fast-approach threshold는 pointer speed
380pt/s와 closing speed 약 182pt/s다. catch radius를 fast-evade radius보다 넓게 두어
trackpad가 sprite에 도착하기 전에 잠깐 멈춰 잡을 기회를 준다.

연결된 display seam에 계속 밀리면 현재 위치에서 약 320pt 안의 portal을 골라 현재
display 안전 경계를 따라간 뒤 이웃 display 안쪽까지 짧은 evade route를 만든다. 실제
gap이 있는 display는 이 경로의 후보에서 제외하므로
pointer evade가 보이지 않는 공간을 순간이동하지 않는다. 일반 wander만 기존의 연속 gap
route를 사용할 수 있다.

입력 모드 writer는 `RoamlingRuntime` 하나다.

```text
normal/look/evade  -> window click-through
catch armed AND pointer in pet ellipse -> interactive
mouseDown          -> caught
mouseDragged       -> dragged; global pointer point로 panel 이동 + caught paw cycle 계속 재생
mouseUp            -> nearest visible frame clamp, dropped, click-through
```

window가 sprite 크기이고 hit ellipse가 투명 margin을 제외하므로 interactive 순간에도
가리는 면적이 작다. 기본 hit ellipse scale은 1.12이며 window bounds보다 커지지 않는다.
향후 alpha-mask hit test를 추가해도 이 ownership은 바뀌지 않는다.

`RuntimeTuning`과 menu bar의 **Behavior Tuning…** 창은 MVP 0/0.5의 체감 검증 값만
노출한다. walk speed, idle pause, 다른 display 방문 확률, notice/catch thresholds,
catch window와 hit region이 실행 중 반영되고 `UserDefaults`에 저장된다. MVP 0.7의
rest timing은 첫 체감 검증 전까지 별도 고정 configuration으로 두며 기존 tuning 값을
섞지 않는다.

FatMochi의 walk row는 frame마다 alpha silhouette을 독립적으로 중앙 정렬한다. renderer가
world position을 이동시키므로 atlas cell 안의 torso centroid는 2px 이상 좌우로 움직이지
않아야 한다. 이 invariant와 가로/세로 silhouette 비율은 asset test로 고정해 moonwalk나
다시 홀쭉해지는 regression을 막는다.

## Safe-zone design

### BasicSafeZoneProvider

MVP 0.7 구현은 권한 없이 screen `visibleFrame`, Dock/menu-bar exclusion, edge/corner
preference, current-display stability, pointer distance, travel distance를 쓴다. 네 corner
region을 만들고 Dock이 차지한 left/right/bottom inset을 감지하면 인접 후보에 작은 bonus를
준다. 최종 center는 pet size를 반영해 visible frame 안으로 clamp한다.

`MacBasicSafeZoneProvider`는 macOS snapshot을 이 pure planner에 전달하는 얇은 adapter다.
MVP 3에서 Accessibility가 생기면 focused window/element, control/caret bounds를 obstacle로
추가하되 현재 fallback path를 유지한다.

### VisualSafeZoneProvider

Screen Recording opt-in일 때만 single snapshot을 downsample하여 edge density, local
variance, temporal stability를 점수화한다. OCR/LLM/network/disk write는 없다.

```text
CandidateScore =
  visualEmpty + caretDistance + controlDistance + edgePreference
  + stability + contextPreference + petComfort
  - pointerProximity - obstructionPenalty
```

confidence가 낮으면 중앙 후보를 만들지 않고 window/display corner로 fallback한다.
caret은 관심 위치지만 avoid radius 안에서는 강한 obstacle이다. minor caret movement는
head/look pose로만 반응하고 dwell/hysteresis가 지나야 위치를 바꾼다.

## Permission model

| permission | 기능 |
| --- | --- |
| 없음 | render, wander, multi-monitor, pointer evade/catch/drag, basic sleep |
| Accessibility | focused app/window/element, caret hint, smarter basic placement |
| Screen Recording | opt-in visual empty-region placement |

MVP 0/0.5와 MVP 0.7은 추가 permission을 요청하지 않는다. `MacUserIdleProvider`는 event
tap이나 input 내용을 수집하지 않고 마지막 local input 이후 경과 시간만 조회한다.
후속 permission prompt는 사용자가 해당 기능을 켠 순간에만 설명과 함께 제시한다.

## Performance model

- active movement/evade/travel: 약 60 Hz, caught/drop: 약 30 Hz
- catch가 arm된 짧은 구간: 60 Hz input gate
- animated idle/look: 약 10–12 Hz 또는 다음 frame deadline
- sleep: 2 Hz (wake input은 최대 약 0.5초 안에 감지)
- display/AX/window tree: notification/debounce 기반
- screen capture: placement 시 single shot만
- image atlas는 한 번 decode하고 frame crop을 cache

renderer는 event source를 모르고 `frame, position, direction, scale, visibility`만 받는다.
occlusion/static 상태에서 redraw를 중단할 수 있게 animation clock과 world clock을 분리한다.

## Decisions

### Native AppKit overlay

**Options:** Electron full-screen overlay, Tauri WebView, AppKit small panel.

**Chosen:** AppKit small panel. macOS의 Spaces/fullscreen/input semantics를 직접 제어하고
상주 비용을 줄일 수 있다. settings에는 SwiftUI를 나중에 섞을 수 있다.

### Swift core now, extraction later

**Options:** Rust core + FFI 선행, Swift pure module, app code에 직접 구현.

**Chosen:** Swift pure module. boundary와 tests는 얻되 미확인 Windows 요구를 위해 FFI를
선행하지 않는다.

### SpriteKit versus AppKit drawing

**Options:** SpriteKit scene, Core Animation/AppKit view.

**Chosen for MVP:** 작은 AppKit view가 atlas frame을 draw한다. 한 sprite의 translation과
frame swap에 scene graph가 필요하지 않다. particle/effect가 복잡해질 때 SpriteKit을
재평가한다.

### Local packages before gallery API

**Options:** Petdex network catalog 내장, local discovery, asset fork.

**Chosen:** local discovery. existing CLI와 Codex가 설치한 package를 modification 없이
읽고, network/gallery는 별도 installer milestone로 둔다.

### Authored default mascot with pose-derived fallback

**Options:** gallery pet 번들, 빈 화면, generated key-pose sheets, code-drawn cat.

**Chosen:** 실제 96×104pt overlay에서 승인된 FatMochi의 얼굴과 silhouette을 고정한 뒤,
MVP 0.7에서 보이는 walk/idle/sleep/caught/stretch/landing만 authored atlas로 교체한다.
고양이식 forward stretch를 사용하며 인간처럼 앞발을 드는 후보는 폐기했다. Mochi는
key-pose 기반 evaluation atlas를 유지하고 code-drawn cat은 resource failure 전용 fallback이다.
후속 agent reaction row를 미리 만들지 않아 milestone 범위를 지키며, third-party asset
license에 의존하지 않고 Petdex loader는 사용자 package와 fixtures로 계속 검증한다.

## Future migration

Windows 구현은 아래 domain protocol을 채운다.

```text
DisplayProvider    NSScreen             -> EnumDisplayMonitors
WindowProvider     CGWindow/AX          -> HWND/Win32
PointerProvider    NSEvent              -> GetCursorPos
UserIdleProvider   CGEventSource        -> GetLastInputInfo
FocusProvider      AXUIElement          -> UI Automation
SafeZoneProvider   visibleFrame/work area candidates
OverlayProvider    NSPanel              -> layered click-through window
CaptureProvider    ScreenCaptureKit     -> Windows Graphics Capture
```

HWND/UIAutomation COM type은 adapter를 넘지 않는다. pure geometry/movement/event/
reaction test가 그대로 통과하는지 확인한 후에만 언어 추출을 논의한다.

Game/media도 동일하다. official telemetry/local event를 먼저 쓰며 occasional visual
detection은 opt-in fallback이다. injection, process memory, anti-cheat-sensitive hook은
Roamling의 관찰자 모델과 맞지 않아 금지한다.
