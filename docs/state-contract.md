# State contract: Petdex를 아래에 두고 Roamling을 위에 얹는다

`docs/behavior-flow.md`가 "지금 어떻게 흐르는가"라면 이 문서는 **"어휘를 어떻게 쌓는가"**
다. 그림을 새로 그리기 전에 이것부터 확정한다. 어휘가 흔들리면 어떤 그림을 그려도 제자리를
못 찾는다.

**구현 완료.** 아래는 제안이 아니라 현재 코드의 설계이고, 타입 이름과 파일 위치는 실제
구현을 가리킨다.

## 0. 진단 — 층이 없어서가 아니라 대응의 근거가 없어서다

층 구조는 이미 있다. `docs/architecture.md:200`이 해상 순서를 적고, `roamling.json`
확장 매니페스트도 loader에 구현돼 있다(`PetLoader.swift:80`).

문제는 **대응이 "비슷해 보이는 것"으로 쓰여 있다**는 것이다. `PetAnimation.swift`의
`candidates` 표는 capability마다 이름 목록만 갖는다.

```swift
.celebrate: ["celebrate", "jumping", "jump", "bounce", "waving"]
```

이 줄에는 `jumping`이 무엇을 뜻하는지가 없다. Petdex에서 `jumping`은 **시작 신호**이고
`waving`이 **완료 신호**인데, 이름만 보고 "축하니까 점프"로 골랐다. 그래서 Roamling은
Petdex 규격으로 그려진 모든 펫에서 **완료 축하에 시작 그림을 쓴다.** 같은 이유로
`landing`은 `celebrate`를 경유하게 되어 있어서, 위 순서를 고치는 순간 착지가 인사
그림으로 바뀐다.

**어휘의 뜻이 코드 어디에도 없다는 것**이 근본 원인이다. 아래 설계는 그것부터 없앤다.

## 1. 네 개의 층과 세 개의 규칙

```text
L0  Petdex state (9)        정본. 그림이 무엇을 뜻하는가. 우리가 정하지 않는다.
L1  Roamling capability     L0의 상위집합. 무엇을 보여줄 것인가.
L2  BehaviorState (19)      무엇을 하는 중인가.
L3  이벤트 → L2             ReactionPolicy. 언제 그렇게 되는가.
```

규칙 셋. 이게 이 문서의 전부다.

1. **L0는 우리가 바꾸지 않는다.** upstream(`hook_runner.zig` · `pet-states.ts` ·
   `main.zig`)에서 포팅해 오고, 출처 줄 번호를 주석에 남긴다. 상류가 바뀌면 다시 읽어
   포팅한다.
2. **L1의 모든 case는 자기가 L0의 무엇인지 선언한다.** L0에 대응이 있으면 그 state를
   지목하고, 없으면 `extension`이라고 명시한다. 지목이 없는 case는 존재할 수 없다
   (컴파일이 강제).
3. **확장이 L0를 빌릴 때는 어떤 뜻을 빌리는지 같이 적는다.** 문맥의 뜻인지, 그림의
   물리적 동작인지. 이 구분이 없어서 `landing`과 `celebrate`가 엉켰다.

## 2. L0 — 정본을 코드에 박는다

`Sources/RoamlingPet/PetdexState.swift`

```swift
public enum PetdexState: String, CaseIterable, Sendable {
    case idle, runningRight = "running-right", runningLeft = "running-left"
    case waving, jumping, failed, waiting, running, review

    var row: Int                       // 행 번호
    var meaning: String                // 문맥의 뜻 (hook_runner.zig:235)
    var standardDuration: TimeInterval // pet-states.ts durationMs
    var isTransient: Bool              // main.zig:1955 isDurationState
    var aliases: [String]              // Codex model.rs가 받는 별칭
    var trackNames: [String]           // rawValue + aliases
}
```

Core는 `RoamlingPet`을 import할 수 없으므로 상태기가 쓰는 두 길이는
`Sources/RoamlingCore/BehaviorTiming.swift`에 복사돼 있고, 테스트
`BehaviorTiming matches the Petdex contract`가 둘이 어긋나지 않게 붙잡는다. 그 테스트가
깨지면 상류가 움직인 것이므로 **완화하지 말고 포팅한다.**

## 3. L1 — capability가 L0를 지목한다

```swift
public extension PetCapability {
    enum Borrow { case meaning(PetCapability), motion(PetCapability) }

    var petdexState: PetdexState?   // nil이면 Petdex에 개념이 없는 확장
    var authoredNames: [String]     // 패키지가 자기 이름으로 그렸을 때 쓰는 이름
    var borrows: Borrow?            // 자기 그림이 없을 때 무엇을 빌리는가
}
```

`meaning`과 `motion`은 resolver에게 똑같이 읽히고 사람에게 다르게 읽힌다. 그게 목적이다 —
`landing`은 **동작**을, `celebrate`는 **뜻**을 빌린다. 이 구분을 안 적었기 때문에 landing이
celebrate 뒤에 매달렸고, celebrate의 뜻을 고치는 순간 모든 착지가 작별 인사가 될 뻔했다.

resolver는 이 선언에서 후보를 **생성한다.** 손으로 쓴 이름 목록은 없앴다.

```text
1. roamling.json의 explicit mapping
2. capability 자기 이름 (sleeping · sitting · caught · gaze …)
3. petdexState가 지목한 행 이름 + 별칭
4. borrows가 가리키는 capability로 한 단계 내려가 2부터 반복
5. idle (placeholder — 메뉴에 그대로 노출)
```

`.celebrate`가 `.waving`을 지목하는 순간 완료 축하가 맞아떨어지고, `.landing`이
`.motion(.spark)`를 들고 있으므로 celebrate를 경유하지 않는다. **두 사고가 같은 변경으로
사라졌다.** 테스트 `every borrow chain ends at idle`이 사슬이 순환하거나 idle에 닿지 못하는
경우를 막는다.

## 4. 개정 대응표

| L1 capability | grounding | 근거 | 지금 |
|---|---|---|---|
| `idle` | `.petdex(.idle)` | 동일 | 같음 |
| `moveRight` / `moveLeft` | `.petdex(.runningRight/.runningLeft)` | 동일 | 같음 |
| `work` | `.petdex(.running)` | tool 실행 중 | 같음 |
| `observe` | `.petdex(.review)` | 검토·읽기 | 같음 |
| `paw` | `.petdex(.waiting)` | 승인·입력 대기 | 같음 |
| `fail` | `.petdex(.failed)` | 실패 | 같음 |
| `celebrate` | `.petdex(.waving)` | **완료 인사** | ✗ `jumping` |
| `spark` (신설) | `.petdex(.jumping)` | **시작 신호** | 없음 |
| `landing` | `.motion(.spark)` | 진짜 hop | ✗ `celebrate` 경유 |
| `gaze` (신설) | `.meaning(.idle)` | 커서 응시 — L0에 개념 없음 | ✗ `review` 대여 |
| `sit` | `.meaning(.paw)` | 앉은 기대 자세 | 같음 |
| `sleep` | `.meaning(.sit)` | | 같음 |
| `stretch` | `.motion(.idle)` | | 같음 |
| `caught` | `.meaning(.paw)` | 올려다보는 자세 | 같음 |
| `dragged` | `.meaning(.caught)` | | 같음 |

바뀐 건 넷이고 그중 둘은 신설이다. 설치된 `mochi-v2`에서의 결과.

```text
idle      -> idle          [authored]     gaze      -> idle    [substituted(idle)]
moveLeft  -> running-left  [authored]     paw       -> waiting [authored]
moveRight -> running-right [authored]     spark     -> jumping [authored]
sit       -> waiting  [substituted(paw)]  celebrate -> waving  [authored]
sleep     -> waiting  [substituted(paw)]  fail      -> failed  [authored]
work      -> running       [authored]     stretch   -> idle    [substituted(idle)]
observe   -> review        [authored]     caught·dragged -> waiting [substituted(paw)]
                                          landing   -> jumping [substituted(spark)]

authored 9 / substituted 7 / placeholder 0
```

**`waving` 행이 처음으로 쓰인다.** 전에는 어느 상태도 그 행에 닿지 못했다.

### `spark` — Petdex의 `jumping` 자리

Petdex는 프롬프트 제출·세션 시작에 `jumping`을 재생한다("Thinking…"). Roamling에는
그 뜻의 capability가 없어서 지금은 `observe`(도리도리)가 나온다. `spark`를 만들면
9:9로 맞고, Codex 창과 데스크탑 펫이 같은 순간에 같은 그림을 보여준다.

`BehaviorState.spark`(0.84초 — `jumping`의 표준 길이)와 `CompanionReaction.spark`가 같이
들어갔다. `PetCapabilityMapping`이 exhaustive switch라 빠뜨리면 빌드가 깨진다.

### `gaze` — 잘못된 대여의 대표

`lookAtPointer`(커서가 170px 안)가 지금 `observe`를 거쳐 `review`를 재생한다. 그런데
`review`는 **1.03초 단발로 설계된 "파일 읽는 중"** 이다. 커서를 40초 응시하는 개념은
Petdex에 아예 없다. 즉 이건 대체가 아니라 **오용**이고, 데스크탑 펫의 확장이어야 한다.

`gaze`는 지금 `idle`로 degrade되므로 커서가 지나가도 조용하다. 빈 셀에 그리면 진짜 그림이
생긴다 — **빈 셀 15칸의 첫 번째 수요다.** 패키지가 `watching`·`gaze`·`looking` 중 아무
이름으로나 그려 두면 resolver가 바로 집는다.

## 5. duration / steady를 Roamling도 지킨다

L0가 이미 분류를 갖고 있다(`main.zig:1955`).

| | Petdex | 이전 | 지금 |
|---|---|---|---|
| `waving` · `jumping` · `failed` · `review` | 재생 후 idle 복귀 | celebrate 2.2s · sad 1.5s만 유한, **observe는 무한** | **네 개 모두 Petdex 표준 길이** |
| `idle` · `running` · `waiting` · 걷기 | 다음 이벤트까지 유지 | `paw`가 1.2초 만에 이탈 | **타이머 없음** |

길이는 `BehaviorTiming`에 모였고, **transient 넷은 Petdex 표준과 정확히 같다.**

한때 `celebrate`를 2.2초, `sad`를 1.5초로 늘려 뒀었다. 데스크탑 어디에나 있는 펫은
터미널 구석의 마스코트보다 눈에 띄는 데 오래 걸린다는 이유였고, 그건 우리 펫에 대해서는
맞는 말이었다. 문제는 **우리 펫에 대해서만** 맞다는 것이다. 규격대로 그려진 남의 펫을
가져오면 그 4프레임 인사가 0.7초짜리인데 2.2초를 쥐고 있으니 세 번 반복된다 — 손을 못
내리는 펫이 된다. 반대로 짧게 쥐면 몸짓 도중에 잘린다.

**어떤 Petdex 펫을 가져와도 제작자가 의도한 대로 보여야 하고, 그건 시계가 맞을 때만
참이다.** steady 상태는 양쪽 다 타이머가 없으므로 패키지가 루프 길이를 마음대로 정해도
되고, Roamling은 그대로 재생한다.

`dropped`도 같은 이유로 0.84초다. `landing` 그림이 없는 펫은 점프 행을 빌리는데, 다른
길이를 주면 공중에서 잘린다.

타이머만으로는 부족했다. `holdSeat`이 매 tick마다 현재 반응을 다시 입히고 있어서, 유한한
`observe`는 idle과 1초 주기로 깜빡이게 된다. 그래서 **지속되는 조건만 다시 입힌다**
(`CompanionReaction.isOngoing` — `work`와 `paw`뿐). 순간은 한 번 보여주고 끝난다.

## 6. 확장을 어디에 담는가 — `roamling.json`과 자기 시트

`pet.json`의 `animations`에 `sleeping` 같은 규격 밖 이름을 넣어도 로더는 받는다. 하지만
그러면 **Petdex 갤러리에 제출할 파일에 규격 밖 키가 섞인다.** 층 규칙대로 확장은 우리
파일에 두고, **그림도 자기 시트에 둔다.**

```json
{
  "schemaVersion": 1,
  "spritesheetPath": "roamling.webp",
  "frame": { "columns": 8, "rows": 2 },
  "behaviors": { "sleep": "sleeping", "gaze": "gaze" },
  "animations": {
    "sleeping": { "frames": [72, 73, 74, 75], "fps": 2,    "loop": true },
    "gaze":     { "frames": [76, 77],         "fps": 0.91, "loop": true },
    "caught":   { "frames": [78, 79],         "fps": 4,    "loop": true },
    "landing":  { "frames": [34, 35, 36],     "fps": 8.57, "loop": false }
  }
}
```

- `pet.json`과 `spritesheet.webp`가 **9행 계약 그대로 남는다.** 갤러리 검증기가 선언되지
  않은 칸을 어떻게 보는지 물을 일이 없다.
- 인덱스는 패키지 격자 끝에서 **이어진다.** 8×9면 72번이 확장 시트의 첫 칸이다. 그래서
  트랙이 두 시트를 섞어도 되고, 위 `landing`은 패키지의 점프 프레임을 그대로 빌려 새로
  그리는 게 없다.
- **셀 크기는 패키지의 것을 쓴다.** 확장 격자에는 열·행 수만 적는다. 두 배율로 그려진
  펫은 한 펫이 아니고, 선언한 격자와 픽셀 수가 어긋나는 시트는 거부한다.
- Codex는 `roamling.json`을 읽지 않는다. 확장 트랙은 Codex에서 그냥 안 보일 뿐이다.
- 확장 애니메이션은 `pet.json`의 것 **다음에** 설치된다. 그래서 트랙을 더할 수도, 하나를
  고쳐 쓸 수도 있다.
- **행별 프레임 수는 여전히 늘리거나 줄일 수 없다.** Petdex 데스크탑 렌더러가 행마다
  고정된 앞 N칸만 재생하기 때문이다(`sprite.zig`).
- 스키마는 **버전 1**이다. 출시 전이라 이전 모양은 버렸다. 다른 버전은 경고 한 줄만 남기고
  무시하며 펫은 9행 그대로 그려진다. 파일이 없을 때가 여전히 가장 중요한 호환 경로다.

빈 셀(15칸)을 쓰는 방법도 동작하고 격자도 안 바뀐다. 다만 확장이 15칸으로 묶이고, 위의
미확인 위험이 남는다.

## 7. L3 — 이벤트 층 정렬

`CompanionEventKind`가 하나 늘었다. `activityStarted`는 이제 **턴이 열렸다**만 뜻하고,
툴 실행은 `inspecting`(읽기·검색)과 `highIntensity`(그 외)로 갈린다. 셋이 Petdex의 세 행과
일대일로 대응한다.

| 훅 | Petdex | 지금 Roamling | 비고 |
|---|---|---|---|
| `sessionStart` · `userPromptSubmit` | `jumping` | `activityStarted` → `spark` → `jumping` | 일치 |
| `preToolUse` (Read·Grep·Glob) | `review` | `inspecting` → `observe` → `review` | 일치 |
| `preToolUse` (그 외) | `running` | `highIntensity` → `work` → `running` | 일치 |
| `postToolUse` | `idle` | 반응 없음(유지) | 의도적 차이 — 매 툴마다 idle로 튀면 산만하다 |
| `permissionRequest` · `notification` | `waiting` (steady) | `paw` → `waiting`, **항상**, 타이머 없음 | 일치 |
| `stop` | `waving` | `celebrate` → `waving` | 일치 |
| `postToolUseFailure` · `stopFailure` | `failed` | `fail` → `failed` | 일치 (Codex엔 이벤트 없음) |
| `subagentStart` | `running` | `highIntensity` → `work` → `running` | 일치 |
| `sessionEnd` | — | 5분 침묵 후 `.calm` → idle | Roamling 전용 |

읽는 것은 tool의 **이름**뿐이고, 그것도 `ToolActivity`의 고정 목록과 대조하기 위해서다.
프롬프트·전사·툴 인자·결과는 여전히 모델에 없다 — 정규화기가 지키겠다고 적어 둔 선은
내용에 관한 것이고, `Read`라는 이름은 훅 이벤트 이름 이상을 말하지 않는다.

`ReactionPolicy`의 확률·강도·1.5초 throttle은 **그대로 뒀다.** Roamling은 상태 표시등이
아니라 동반자이고, 매 이벤트에 정확히 반응하면 "never annoying"을 어긴다. 맞춰야 하는 것은
**어휘이지 타이밍이 아니다.** 예외는 승인 요청 하나다. 그건 사용자에게 물어보는
행위이므로 굴리지 않고 항상 앞발을 든다.

## 8. 무엇이 들어갔나

| | 파일 |
|---|---|
| L0 타입 | `RoamlingPet/PetdexState.swift` (신설) |
| capability 선언 + resolver | `RoamlingPet/PetAnimation.swift` |
| `lookAtPointer` → `gaze`, `spark` | `RoamlingPet/PetCapabilityMapping.swift` |
| 내장 마스코트의 완료 동작 | `RoamlingPet/MascotPetFactory.swift` |
| 확장 매니페스트 v2 | `RoamlingPet/PetManifest.swift` · `PetLoader.swift` |
| 상태·반응·길이 | `RoamlingCore/BehaviorController.swift` · `BehaviorTiming.swift` (신설) |
| 이벤트 종류 | `RoamlingCore/Activity.swift` · `ReactionPolicy.swift` · `AttentionModel.swift` |
| tool 이름 분류 | `RoamlingSources/Shared/ToolActivity.swift` (신설) + 두 정규화기 |
| 지속 조건만 재적용 | `RoamlingMac/RoamlingRuntime.swift` |

내장 마스코트는 `waving` 행이 없거나(7행 레이아웃) 대기 자세에서 합성된 것이라, 완료
동작을 Roamling 자기 이름인 `celebrate` 트랙으로 authored 해 뒀다. `authoredNames`가
Petdex 행보다 먼저 조회되므로 뜻을 왜곡하지 않고 이긴다. 9행짜리 내장 Mochi는 진짜 wave
행이 있으므로 그 행으로 완료를 그린다.

## 9. 남은 것

그림이다. `docs/art/mochi-v2-animation-spec.md`의 행별 목표를 확정하고 제작한다. 어휘가
고정됐으므로 이제 각 행이 **언제 · 얼마나 오래** 떠 있는지가 확정돼 있고, 그게 작화 기준이다.

- `waving`은 0초에서 **완료 축하**로 바뀌었다. 재설계 결과를 이제 눈으로 볼 수 있다.
- `review`는 `observe` 전용이 되었고 1.03초 뒤 물러난다. 커서 응시가 빠져나갔다.
- `gaze`는 그릴 그림이 아직 없다. 빈 셀 15칸의 첫 수요.
- `jumping`의 시작·끝 프레임이 지면선보다 25px 아래인 결함은 그대로다. 이제 이 행은
  `spark`(턴 시작)와 `landing`(착지) 둘 다에 쓰이므로 더 자주 보인다.
