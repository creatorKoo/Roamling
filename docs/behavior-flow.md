# Behavior flow: 어떤 상황에 어떤 그림이 뜨는가

`docs/architecture.md`는 모듈 경계를, `docs/pets.md`는 capability와 패키지 사이의 간격을,
`docs/art/mochi-v3-plan.md`의 "완료 — 실제로 만들어진 것"은 지금 시트에 무엇이 그려져
있는지를 다룬다. 이 문서는 그것들을 **시간 순서로 이어 붙인 것**이다 — 펫이 idle에서 시작해서 무엇 때문에
무엇으로 넘어가고, 그 순간 화면에 실제로 어떤 그림이 뜨는가.

행을 새로 그리기 전에 이 문서를 읽는다. 그림의 좋고 나쁨은 그림만 봐서는 판단할 수 없고,
**그 그림이 몇 초 동안 떠 있느냐**에 달려 있기 때문이다. 0.35초 스치는 자리와 40초 도는
자리는 같은 기준으로 그리면 안 된다.

기준 패키지는 `~/.codex/pets/mochi-v3`다 — 표준 `spritesheet.webp` 8열 × 9행에 확장
`roamling.webp` 8열 × 3행이 얹힌다. 내장 마스코트도 같은 바이트를 쓴다.

이 문서는 **지금 그렇게 동작한다**는 기록이다. 그 어휘를 왜 그렇게 쌓았는지는
`docs/state-contract.md`에 있다.

## 1. 상태와 그림 사이에는 번역이 세 번 있다

```text
훅 이벤트  ──▶  BehaviorState (20종)  ──▶  PetCapability (16종)  ──▶  Petdex 행 (9종)
언제           무엇을 하는 중인가        무엇을 보여줄 것인가      그림

ReactionPolicy  BehaviorController    PetCapabilityMapping    AnimationResolver
                                      (exhaustive switch)     (petdexState + borrows)
```

마지막 번역이 이 프로젝트에서 사람이 가장 자주 헷갈리는 지점이다. 상태는 20개인데 펫이
가진 행은 9개라서 **여러 상태가 같은 행 하나로 모인다.** 상태기가 정상으로 돌아도 화면이
안 바뀌는 일이 여기서 생긴다.

어휘를 어떻게 쌓았는지는 `docs/state-contract.md`에 있다. 요약하면 capability 9종은
Petdex 행을 그대로 뜻하고, 나머지 7종은 Petdex에 개념이 없는 확장이다.

## 2. 표준 9행만 있을 때 보여주는 것

| 상태 | capability | 실제 재생되는 행 | 그 그림 | 출처 |
|---|---|---|---|---|
| `idle` | `idle` | row 0 `idle` | 앉아서 눈 깜박 (6프레임) | authored |
| `wander` `evadePointer` `findSleepSpot` `travelToInterest` | `moveRight` / `moveLeft` | row 1 / row 2 | 걷기 8프레임 (왼쪽은 mirror) | authored |
| `observe` | `observe` | row 8 `review` | 고개 좌우 도리도리 | authored |
| `work` | `work` | row 7 `running` | 앞발로 한 점을 꾹꾹 누르는 작업 모션 | authored |
| `waitingForUser` | `paw` | row 6 `waiting` | 앉아서 위 올려다보며 고개 갸웃 | authored |
| `spark` | `spark` | row 4 `jumping` | 제자리 점프 (5프레임) | authored |
| `celebrate` | `celebrate` | row 3 `waving` | 앞발 들어 인사 (4프레임) | authored |
| `sad` | `fail` | row 5 `failed` | 축 처지는 8프레임 | authored |
| `lookAtPointer` | `gaze` | row 0 `idle` | ↑ idle과 **같은 그림** | substituted |
| `sit` `sleep` | `sit` `sleep` | row 0 `idle` | ↑ idle과 **같은 그림** | substituted |
| `caught` `dragged` | `caught` `dragged` | row 6 `waiting` | ↑ `paw`와 **같은 그림** | substituted |
| `wake` `stretch` | `stretch` | row 0 `idle` | ↑ idle과 **같은 그림** | substituted |
| `dropped` | `landing` | row 4 `jumping` | ↑ `spark`와 **같은 그림** | substituted |

authored 9 · substituted 7 · placeholder 0. **아홉 행이 전부 쓰인다** — `waving`이 완료
축하를 맡으면서 마지막 빈자리가 채워졌다.

### 확장 시트가 바꾸는 것

위 표는 **Petdex 9행만 있는 패키지**의 결과다. `roamling.json`이 있으면 그 위에 얹힌다.
Mochi v3는 다섯 항목을 확장 시트에서 직접 그린다.

| 상태 | capability | 확장 후 재생되는 행 | 그 그림 |
|---|---|---|---|
| `lookAtPointer` | `gaze` | row 8 `review` 프레임을 **커서 거리에 따라 1.0~2.0배속** | 꼬리 흔들기 — 가까울수록 빨라진다 |
| `sleep` | `sleep` | 확장 `sleeping` 3프레임 | 바닥에 웅크려 숨쉬기 |
| `caught` | `caught` | 확장 `caught` 4프레임 | 목덜미 잡혀 뒷다리 접힌 자세 |
| `sit` | `sit` | 확장 `sitting` 4프레임 | 앉은 채 눈 감기다 꾸벅 |
| `wake` `stretch` | `stretch` | 확장 `stretching` 8프레임 | 웅크림에서 기지개 켜고 일어나 앉기 |

확장 다섯이 들어오면서 row 6이 감당하는 상태는 다섯(승인 대기·앉기·잠·잡힘·끌림)에서
둘(승인 대기·끌림)로 줄었다. **남은 대체는 `dragged`와 `landing` 둘뿐이고 둘 다 의도된
대여다.** `landing`은 진짜로 hop이라 점프 행이 맞는 그림이고(팩토리가 그 프레임을
역순·논루프로 다시 타이밍한다), `dragged`는 `caught`와 물리적으로 같은 상황이다 —
`BehaviorController`에서 둘의 차이는 잡힌 펫이 움직이느냐뿐이라 같은 그림이 맞다.
**더 그릴 행은 없다.**

`stretching`은 **두 행을 잇는 유일한 행**이다. `sleeping`의 웅크림에서 출발해 `idle`의
앉은 자세로 끝나야 하므로 양 끝이 이웃 행과 맞아야 한다. `wake` 0.7초와 `stretch` 1.0초가
같은 capability이고 player는 capability가 바뀔 때만 트랙을 되감으므로, 여덟 칸이 두 상태에
걸쳐 한 번에 재생된다.

Petdex 9행만 있는 패키지에서 `sit`은 `waiting`이 아니라 **`idle`을 빌린다.** `waiting`의
계약상 뜻이 "사용자에게 막힘 — 승인이나 입력 대기"라, 졸린 펫이 승인을 조르는 신호를
띄우기 때문이다. `waiting`이 마침 앉은 자세인 것은 근거가 못 된다 — Petdex는 행의 뜻만
고정하고 포즈는 펫을 그리는 사람에게 맡긴다.


## 3. 흐름 A — 아무 일도 없을 때

```text
idle (row 0, 앉아서 깜박)
 │
 ├─ 8.4~17.4초 뒤 ──────────────▶ wander (row 1/2, 걷기)
 │   (wanderPause 12 × 0.7~1.45)      │
 │                                    └─ 목적지 도착 ──▶ idle
 │
 └─ 사용자 입력이 75초 없음 ────▶ sit (확장 sitting, 꾸벅)
     + 커서가 170px 밖                │
     + 이동 중 아님                   └─ 2.4초 뒤 ──▶ 취침 자리 판정 (3.1)
                                                        │
                                       제자리 ──────────┤
                                                        │
                                       findSleepSpot ───┤  row 1/2, 0.75배속 걷기
                                       안전지대까지 걸어감    도착하면
                                                        ▼
                                                      sleep (확장 sleeping, 웅크림)
                                                        │
                       입력 0.8초 안에 발생 or 커서 접근 ─┘
                                                        ▼
                                                 wake (확장 stretching 시작)
                                                   └ 0.7초 ─▶ stretch (확장 stretching, 기지개)
                                                                └ 1.0초 ─▶ idle
```

### 3.1 취침 자리 판정

`sit`이 끝나는 순간 한 번만 묻는다 — **지금 자리가 오래 자도 되는 자리인가.** `sleep`은
타이머가 없는 유일한 상태라, 서 있는 동안 "옮길 만큼 나쁘지는 않은" 자리로는 부족하다.
서 있을 때 사용자 작업을 피해 주던 규칙(`PlacementDirector.strollVerdict`의 2.5초 체류
검사)이 휴식이 movement를 가져가는 순간 멈추기 때문이다.

| 지금 자리 | 결정 | 판정 위치 |
|---|---|---|
| 에이전트 옆 — 디렉터가 `.sleepInPlace`를 줌 | 제자리 | `PlacementDirector` 우선순위 8 |
| 밝기 점수 ≥ `holdEmptiness`(0.55) | 제자리 | `BasicSafeZonePlanner.napsInPlace` |
| 덮여 있음 | `findSleepSpot` | 〃 |
| 잴 수 없음 — 화면 기록 권한 없음, 캡처 실패, 캡처된 디스플레이 밖 | `findSleepSpot` | 〃 |

**"못 잰다"가 여기서는 거절이다.** 에이전트 자리에서 캡처가 없는 것은 통과인데
(`PlacementDirector`의 `evaluation?.isHoldable ?? true`) 여기서는 반대다. 자리를 고른
경위가 다르기 때문이다 — 에이전트 자리는 일부러 고른 자리지만, 배회 목적지는 밝기가
없으면 `VisualEmptiness.firstComfortable`이 nil을 주고 후보 목록의 첫 번째로 떨어진다.
즉 아무도 검증한 적 없는 지점이고, 그걸 무기한 깔고 자게 두느니 구석으로 접는 편이 낫다.

권한이 켜져 있으면 이 갈래는 사실상 항상 제자리다. 자리가 덮이면 2.5초 체류 검사가
이미 비켜 주므로, 75초를 채운 자리가 덮여 있을 일이 거의 없다. 어느 갈래를 탔는지는
진단 로그 `rest` 항목에 남는다.

**확장 시트가 들어오기 전에는** 이 네 상태가 전부 이미 보고 있던 두 그림(row 6, row 0)
으로 처리됐다. 75초를 기다려 재운 결과가 "앉은 자세 그대로"라서 펫이 잤는지 아닌지
알 수가 없었다. 지금은 넷이 각자 그림을 갖는다 — 꾸벅이는 `sitting`, 웅크린 `sleeping`,
그 둘을 잇는 `stretching`.

## 4. 흐름 B — 커서가 다가올 때

거리 판정은 `PointerInteractionConfiguration` 기본값 기준이다.

```text
                          커서 거리
idle / wander ─────────── 170px 이내 ──▶ lookAtPointer (gaze — 지금은 row 0 idle)
                                            └ 커서가 170px 밖으로 ─▶ idle
              ─────────── 100px 이내 ──▶ evadePointer (row 1/2, 걷기 1.4배속)
              ─────────── 50px 이내 ──▶ evadePointer (더 빠르게)
              ─────────── 74px 이내 + 커서 속도 380 이상 + 접근 속도 182 이상
                                       ──▶ 클릭 가능 상태 (0.35초 창)
                                            │
                                       mouseDown
                                            ▼
                                        caught (row 6)
                                            │ 4px 이상 끌면
                                            ▼
                                        dragged (row 6, 같은 그림)
                                            │ 버튼 놓음
                                            ▼
                                        dropped (row 4, 점프 그림)
                                            └ 0.84초 ─▶ idle
```

자고 있을 때 커서가 가까이 오면 rest가 즉시 취소되고 `wake`로 간다(흐름 A의 오른쪽 경로).

**예외가 하나 있다 — 사용자 글자 위에서 벗어나는 걸음은 170px 대역을 무시한다.**
배회 중의 탈출이든 활동 중 자리 감시가 내는 `coveringCaret` · `coveringWork` 걸음이든 같고,
커서가 좌석에 앉아 시작된 `seatUnderPointer` 걸음도 같다 — 커서 때문에 떠나는 걸음이 커서를
보느라 멈추면 자기모순이다.
걷던 중이면 멈추지 않고, 응시 중에 자리가 덮이면 응시를 접고 출발한다. 응시는 경로를
취소하기 때문에, 예외가 없으면 펫이 떠나던 문단 한가운데에 커서가 붙잡아 세워 놓게
된다. 100px 안쪽의 회피와 잡기는 예외 없이 그대로 이긴다 — 둘 다 펫을 그 자리에
버려두지 않기 때문이다. 근거는 `docs/placement.md` 3.2.2.

**애정 키 (2026-09-10).** 왼쪽 ⌘(macOS) / 왼쪽 Ctrl(Windows)을 누른 채 커서가 170px 안에
오면 위 표의 회피가 전부 꺼진다. 근접 등급이 무엇이든 `lookAtPointer`로 가서 평소처럼
꼬리를 흔들고(`gaze`), **커서가 펫 위에 올라오면** `paw`(row 6 `waiting` — 앉아서 위
올려다보며 고개 갸웃)를 입는다 — 상태는 응시 그대로고 옷만 다르다
(`capability_for`의 `is_petted`). 배치는 커서 소유로 보아 전부 `None`이고, 응시 인내
(`docs/placement.md` 3.2.5)도 세지 않는다 — 쓰다듬는 손은 지겨워지는 대상이 아니다. 잡기는
실제 근접 판정을 그대로 읽으므로 앉은 펫도 집어 올릴 수 있다. 키를 떼면 그 tick부터 커서
거리에 따라 평소대로 회피한다. 키 상태는 셸이 읽는다 — macOS는
`CGEventSource.keyState`(권한 불필요, `NSEvent.modifierFlags`는 좌우를 구분 못 한다), Windows는
`GetAsyncKeyState(VK_LCONTROL)`.

끌지 않고 그냥 클릭하면 `caught → dragged`를 한 바퀴 돌린 뒤 `dropped`로 끝난다. 즉
**클릭도 드래그도 row 6 → row 4 순서로 같은 그림을 본다.**

## 5. 흐름 C — Claude Code / Codex가 일할 때

훅 이벤트는 `CompanionEvent(kind, intensity)`로 정규화되고, `ReactionPolicy`가 거기에
context 배율(작업 중 = 0.8)을 곱해 반응을 고른다. 어휘는 Petdex 정본에 맞춰져 있다.

| hook 이벤트 | kind | 반응 | 상태 | 화면 | Petdex |
|---|---|---|---|---|---|
| sessionStart · userPromptSubmit | `activityStarted` | `spark` | `spark` | row 4 점프 → 0.84초 뒤 idle | `jumping` |
| preToolUse (Read·Grep·Glob) | `inspecting` | `observe` | `observe` | row 8 → 1.03초 뒤 idle | `review` |
| preToolUse (그 외) | `highIntensity` | `work` | `work` | row 7 작업 모션, 유지 | `running` |
| postToolUse | `positive` 0.06 | 없음 | 유지 | 변화 없음 | `idle` |
| postToolUseFailure (Claude) | `setback` | `sad` | `sad` | row 5 → 1.22초 뒤 idle | `failed` |
| permissionRequest · notification | `attentionRequired` | `paw` **항상** | `waitingForUser` | row 6, **응답까지 유지** | `waiting` |
| stop | `achievement` | `smallCelebrate` | `celebrate` | row 3 인사 → 0.70초 뒤 idle | `waving` |
| stopFailure (Claude) | `negative` | `sad` | `sad` | row 5 | — |
| subagentStart (Codex) | `highIntensity` | `work` | `work` | row 7 | `running` |
| sessionEnd | `activityEnded` | 없음 | 5분 침묵 후 `calm` → idle | — | — |

여기에 더해 **에이전트 창 옆으로 걸어가는 이동**이 있다. 활동 위치가 잡히면
`travelToInterest`로 걸어가고(row 1/2), 도착하면 그 이벤트가 요구한 반응을 그 자리에서
입는다.

**좌석은 커서에서 인식 거리(기본 170px) 밖에서 고른다.** 흐름 B의 응시가 걸음을 멈추기
때문에 커서 옆 좌석에는 도착할 수 없다 — 170px 앞에서 서고, 커서가 조금 움직이면 다시
출발하고, 다시 서는 것이 사용자가 본 "가다 멈추다"였다. 걷는 중에 커서가 목적지에 앉으면
0.5초 review가 다른 좌석으로 돌리고(`seatUnderPointer`), 다른 좌석이 없으면 선 자리가
깨끗할 때 거기서 멈춰 지켜보고, 캐럿이나 글자 위면 커서 반대쪽으로 187px 비켜선다.
커서가 떠나면 다음 review가 원래 규칙대로 자리로 데려간다. 앉아 있는 펫 옆을 지나는
커서는 대상이 아니다 — 그건 응시다. 사다리 전체는 `docs/placement.md` §3.2.

반응 사이에는 1.5초의 최소 간격이 있다. 단 `attentionRequired`·`negative`·`setback`은
이 간격을 무시하고 즉시 나온다.

### 자리를 지키는 동안 무엇을 입는가

`holdSeat`은 매 tick마다 현재 반응을 다시 입힌다. **다시 입히는 것은 지속되는 조건뿐이다**
(`work`와 `paw`). 순간인 반응 — 시작 점프, 파일을 읽는 동안의 눈길 — 은 한 번 보여주고
끝나고, 펫은 그냥 그 자리에 있는다.

이걸 구분하지 않아서 1.03초짜리 `review`가 세션 내내 도는 루프가 됐었다.


### Petdex 정본

같은 아홉 행이 Codex 쪽에서도 재생된다. 정본은
`petdex-desktop-native/src/hook_runner.zig:235-250`의 `stateForEvent`다.

```text
user-prompt · session-start        → jumping   "Thinking…"
pre + Read·Grep·Glob               → review    "Reading …"
pre + 그 외 tool                    → running
post                               → idle
notification · approval-request    → waiting   "Waiting for you…"
approval-response · subagent-start → running
stop · session-end · assistant     → waving    "Done."
tool-failure                       → failed
```

**`jumping`은 축하가 아니라 시작 신호이고, `waving`이 완료 신호다.** 그리고 `waving` ·
`failed` · `review` · `jumping`은 duration state라 한 번 재생하고 물러나며, `idle` ·
`running` · `waiting` · 걷기는 다음 이벤트까지 유지된다 (`main.zig:1955`).

§5의 표가 이 정본에 맞춰져 있다. 남은 차이는 하나뿐이다 — **`post`에서 Petdex는 `idle`로
되돌리고 Roamling은 아무것도 하지 않는다.** 툴 하나가 끝날 때마다 자세가 튀면 산만하다는
판단이고, 의도된 차이다.

어휘를 어떻게 맞췄는지는 `docs/state-contract.md`에 있다.

### 승인 대기가 이 흐름의 시금석이다

```text
permissionRequest ─▶ waitingForUser (row 6) ─ 다음 훅 이벤트까지 유지 ─▶ …
```

에이전트는 사용자에게 막혀 있다. 그래서 펫은 계속 물어봐야 하고, Petdex도 `waiting`을
steady로 분류한다. 이전에는 1.2초 만에 `observe`로 넘어가서, 승인 프롬프트를 띄워 놓고
사용자가 실제로 오래 본 그림은 앞발이 아니라 `review`였다.

승인 요청은 확률로 굴리지도 않는다. 세 번에 두 번만 물어보는 펫은 신뢰를 못 얻는다.

## 5b. 흐름 D — 지정한 앱에서 일할 때

에이전트가 아닌 활동 source가 하나 있다. **사용자가 메뉴에서 고른 앱**이다(한글·한쇼·
파워포인트처럼 코딩과 무관한 앱을 쓰는 사람을 위한 것이다). 셸이 0.5초마다 세 가지만
읽고(`RoamlingRuntime.sampleWorkingApplication`) — 앞에 있는 앱의 id, 그 앱이 지정 앱인지,
마지막 키 입력 후 초 — 그것이 무슨 뜻인지는 `rust/roamling-core/src/focus_activity.rs`가
전부 정한다.

**이 source는 §5와 달리 사건을 내지 않는다.** 매 샘플 "지금 이렇다"를 선언하고
(`StateDeclaration`), 반응은 그 선언이 **바뀔 때** 정해진다. 상태형 source의 설계 전체는
`docs/state-sources.md`에 있고, 2026-09-12 이전의 사건형 구조와 그때 붙어 있던 우회 코드의
기록은 `docs/history/focus-activity-flow.md`에 있다.

### 상태 넷과 이정표 둘

`source_state.rs`의 낱말이다. 왼쪽이 source가 말하는 것, 오른쪽이 펫이 입는 것이다.

| 등급 | 뜻 | 도착 반응 | 유지 반응 |
|---|---|---|---|
| `Away` | 이 source는 볼 것이 없다 | — (좌석 반납) | — |
| `Beside` | 앞에 있지만 하는 일은 모름 | `calm` (걸어와 앉기) | 없음 — 그냥 앉아 있음(row 0) |
| `Active` | 일하는 중 | `work` | `work` (row 7) |
| `Paused` | 하다 멈춤 | `paw` | `paw` (row 6, 갸웃) |

| 이정표 | 언제 | 반응 |
|---|---|---|
| `SittingStarted` | 그 세션의 첫 키 입력 | `spark` (점프, row 5) |
| `SittingEnded` | 3분 이상 친 세션이 끝남 | `smallCelebrate` (손 흔들기, row 3) |

이정표는 **등급이 그대로여도 전이로 친다**(`SourceStates::declare`). 그리고 등급 반응을
이긴다 — 첫 키 입력은 `Active`(= `work`)로 가는 전이지만 점프가 나간다(`reaction_for`).
점프는 **도착 반응이라 한 번만** 나가고, 그 뒤 펫이 계속 입는 것은 **유지 반응** `work`다
(`sustained_reaction`). 순서를 지키는 것이 타이머가 아니라 이 구분이다.

### 사용자가 한 일 → 무엇이 나가는가

| 사용자가 한 일 | 선언 | 화면 |
|---|---|---|
| 지정 앱이 앞으로 옴 | `Beside` | 창 옆으로 걸어와 그냥 앉음. 점프·갸웃 없음 |
| 그 세션의 첫 키 입력 | `Active` + `SittingStarted` | 점프, 이어서 row 7 |
| 세션 안의 다음 키 입력 | `Active` | row 7, 유지 |
| 마지막 키 입력 후 10초 | `Paused` | row 6 (갸웃) |
| 갸웃 중 키 입력 | `Active` | row 7 |
| 갸웃 5초 | `Away` | 자리를 비우고 원래대로 돌아다님 |
| 돌아다니는 중 다시 키 입력 | `Active` | 걸어와서 row 7, 점프 없음 |
| 키 없이 보기만 함 | `Beside`를 0.5초마다 다시 | 옆에 계속 앉아 있음 |
| 다른 앱이 3초 미만 앞에 옴 | 같은 등급, `focused: false` | 앉은 채. 포커스만 놓는다 |
| 다른 앱이 3초 이상 앞에 옴 | `Away` (+ 누적 3분이면 `SittingEnded`) | 있는 자리에서 인사하고 자리 비움 |
| 지정 앱 → 다른 지정 앱 | 한 샘플에 옛 것 `Away` + 새 것 `Beside` | 새 창으로 걸어가 앉음. 그 앱의 첫 키에 점프 |
| 펫 자신이 앞에 옴 (메뉴·설정) | **직전 선언을 이정표만 떼고 다시** | 아무 일 없음. nil은 "모름"이지 "떠남"이 아니다 |
| agent가 자리를 지킴 | 선언은 그대로 계속 | agent 옆에 그대로 — 아래 "agent와 같이" |

**선언이 2초(`STATE_EXPIRY`) 동안 갱신되지 않으면 `Away`로 만료한다**
(`SourceStates::expire`, 매 틱 `pet_runtime`에서 부른다). 셸이 말을 멈춘 것을 감지하는 유일한
장치다. 앞의 앱을 모를 때(nil) 코어가 직전 선언을 다시 내주는 것도 이 만료 때문이다 — 안 그러면
메뉴를 2초 넘게 연 것만으로 세션이 끝난다.

### 왜 앱이 아니라 키 입력에 반응하는가

앱이 앞에 온 것만으로는 사용자가 무엇을 하려는지 알 수 없다. 문서를 읽는 사람 옆에서 10초마다
갸웃하는 펫은 "never annoying"을 정면으로 어긴다 — 첫 구현(v1)이 그랬고, 써 보고 바로 나온
수정이 이것이다. 그래서 앱이 앞에 오면 옆에 앉기만 하고(`Beside`), 점프는 **그 세션의 첫 키
입력**에, 갸웃은 **타이핑이 멈췄을 때**만 나온다. 갸웃도 5초(`WAITING_BEFORE_RELEASE`)면 자리를
돌려주고 원래대로 돌아다닌다 — 질문이지 감시가 아니다.

**세션은 앱 앞에 있는 한 이어진다.** 거르는 단계가 둘이다. **3초**(`FOCUS_GRACE`)가 "떠났는가"를
정하고, **2분**(`BREAK`)이 "돌아온 것이 새 세션인가"를 정한다. 2분 안에 같은 앱으로 돌아오면 같은
세션이라 다음 키 입력은 점프 없이 바로 일하고, 2분 넘게 떠났다 오거나 다른 지정 앱으로 바뀌면 새
세션이라 첫 키 입력의 점프가 다시 걸린다. 유예가 없으면 브라우저를 1초 보고 온 것도 "끝 → 시작"이
되어 펫이 종일 일어섰다 앉는다.

**Cmd-Tab과 Alt-Tab도 키 입력이다.** 그래서 그 앱이 **끊김 없이 앞에 있기 시작한 첫 샘플보다
나중에** 눌린 키만 타이핑으로 센다(`now - seconds_since_key > in_front_since`). 다른 앱이나 nil이
한 샘플이라도 끼면 다시 센다. **갸웃까지의 10초도 셀 수 있는 마지막 키부터 잰다**
(`last_counted_key_at`, `keys_stopped`). 원시 `seconds_since_key`로 재면 타이핑 중 Cmd-Tab으로 다른
창을 보고 돌아오는 키 두 번이 10초 창을 다시 시작시켰다.

### 손 흔들기 — 3분 이상 친 세션이 끝날 때

떠남(3초 유예 뒤)에 이번 세션의 타이핑 누적이 3분(`WAVE_AFTER_TYPING`) 이상이면 `Away` 선언에
`SittingEnded`를 싣는다(`end_milestone`). **phase와 무관하다** — 앉아 있든, 갸웃 중이든, 갸웃
5초 뒤 자리를 돌려주고 돌아다니는 중이든. 보통은 타이핑을 멈추고 한참 뒤에 앱을 닫기 때문에,
"펫이 그 앱 옆에 있을 때만"으로 두면 손 흔들기가 거의 안 나온다. director는 좌석을 비우기
전에 **펫이 있는 자리에서** 그 반응을 입힌다(`apply_state_transitions`) — 걸어오지 않는다.
누적은 떠나는 순간 0이 된다: 흔들었든 아래 규칙으로 안 흔들었든, 곧 다시 떠나도 또 흔들지 않는다.

누적은 **`Active`인 동안에만** 쌓인다(`typed_seconds += elapsed`). 읽기만 한 시간은 안 센다.

### 쉬는 펫

director가 **전이를 보관한다**(`state_transitions`). 쉬는 펫에게는 아무것도 입히지 않고, 깨는
첫 순간에 그때 유효한 것을 입힌다. source는 펫이 쉬는지 알 필요가 없다 — 그냥 계속 선언한다.
예외는 손 흔들기 하나다: `SittingEnded`가 대기 중이면 director가 `CancelRest`를 내서 펫을
먼저 깨운다. 3분 일한 끝의 인사는 늦게라도 보여야 하기 때문이다.

### agent와 같이 있을 때 — agent가 우선이다 (2026-09-11 사용자 확정)

Claude Code에 메시지를 보내고 편집기에서 타이핑했더니 20초 동안 반응이 없다가 점프 없이
`running`이 됐다. attention 점수로 방금 시작된 agent 턴 ≈ 62와 첫 키 입력의 점프 ≈ 61이 이력
여유(12) 안이라 점프가 밀렸고, 거꾸로 갸웃(긴급 · 100)은 일하는 agent의 자리를 곧바로 뺏었다.
타이핑은 지고 멈추면 이기는 상태였다.

**규칙: agent가 자리를 지키는 동안 지정 앱의 상태 전이는 펫에게 가지 않는다**
(`apply_state_transitions`의 첫 분기, 술어는 `agent_on_duty`). "지키는 동안"은 30초(attention이
이벤트를 세는 시간) 안에 들은 agent 이벤트가 있거나, 자리 주인이 agent이고 5분 침묵 만료 전인
것이다 — 뒤의 절반이 도구 하나가 긴 agent의 자리를 지킨다. attention(점수 · 이력 여유 · dwell ·
쿨다운)은 그대로다.

막혀 있는 동안 source가 다시 말할 필요는 없다. **선언은 어차피 0.5초마다 갱신되므로**, agent가
자리를 놓는 순간 다음 샘플에 지정 앱이 이긴다. 그동안 보관된 전이는 둘로 갈린다:

- **인사(`SittingStarted`)는 보관한다.** 등급이 `Paused`로 넘어가도 이정표를 새 전이에 옮겨
  단다(`declare_state`). 자리를 받은 그때 점프한다 — 못 본 점프는 쓴 것이 아니다.
- **작별(`SittingEnded`)은 버린다.** agent 옆에서 끝난 세션은 펫의 것이 아니었다. 남겨 두면
  agent가 몇 초 뒤 끝났을 때 그 축하 직후에 늦게 튀어나온다(실제로 그랬다).
- `Away` 전이는 인사를 **덮는다** — 세션이 끝난 뒤의 늦은 인사는 없다.

### 그 밖에

**키 입력 판정에 훅을 쓰지 않는다.** macOS는 `CGEventSource.secondsSinceLastEventType(_:
eventType: .keyDown)`으로 "얼마나 지났는지"만 묻는다. 무엇을 눌렀는지는 아무도 볼 수 없고
권한도 필요 없다. Windows 쪽 대응은 `docs/windows.md` W8에 있다.

**`CompanionEventKind::present`는 이 source가 사건형이던 때 늘어난 kind인데, 이제 아무도 만들지
않는다.** 지우지 않은 이유는 `activity.rs`의 주석에 있다 — kind가 FFI를 인덱스로 건너서
differential 픽스처 10개가 위치로 이름을 부른다.

기본값은 **빈 목록**이다. 사용자가 메뉴에서 앱을 고르기 전까지 이 흐름은 아무것도 하지
않는다 — 여기서 잘못 짐작하는 것은 "never annoying"을 정면으로 어기는 쪽이다.

## 6. 전이 시간표

| 상태 | 얼마나 머무는가 | 무엇이 끝내는가 | 정의 위치 |
|---|---|---|---|
| `idle` | 8.4~17.4초 | 배회 스케줄러 | `RuntimeTuning.wanderDelay` |
| `wander` | 목적지까지 (160pt/s) | 도착 | `MovementController` |
| `sit` | 2.4초 | 고정 타이머 | `RestConfiguration.sittingDuration` |
| `findSleepSpot` | 목적지까지 (120pt/s) — 자리가 검증되면 아예 건너뜀 (3.1) | 도착 | `beginRestTravel` |
| `sleep` | 무제한 | 입력 0.8초 or 커서 접근 | `updateRestLifecycle` |
| `wake` | 0.7초 | 고정 | `BehaviorTiming.wake` |
| `stretch` | 1.0초 | 고정 | `BehaviorTiming.stretch` |
| `dropped` | **0.84초** | 고정 — 점프 행을 빌리므로 Petdex `jumping` 표준 | `BehaviorTiming.dropped` |
| `spark` | **0.84초** | 고정 — Petdex `jumping` 표준 | `BehaviorTiming.spark` |
| `observe` | **1.03초** | 고정 — Petdex `review` 표준 | `BehaviorTiming.observe` |
| `celebrate` | **0.70초** | 고정 — Petdex `waving` 표준 | `BehaviorTiming.celebrate` |
| `sad` | **1.22초** | 고정 — Petdex `failed` 표준 | `BehaviorTiming.sad` |
| `waitingForUser` | **무제한** | 다음 훅 이벤트 (승인·거부). 지정 앱은 source가 5초 뒤 자리를 비운다 | 타이머 없음 — Petdex steady |
| `work` | **무제한** | 다음 이벤트·커서·배회·휴식 | 타이머 없음 — Petdex steady |
| `lookAtPointer` | 커서가 170px 안에 있는 동안 | 커서 이탈 | `handlePointer` |
| `caught` / `dragged` | 마우스를 놓을 때까지 | mouseUp | `petOverlayMouseUp` |
| 휴식 진입 조건 | 사용자 입력 **75초** 무발생 | — | `RestConfiguration.idleBeforeRest` |
| 지정 앱 타이핑 유지 | 마지막 키 입력 후 **10초** | 키가 멈춤 → 갸웃 | `focus_activity.rs` `TYPING_WINDOW` |
| 지정 앱 갸웃 | **5초** (제안값) | 키 입력 → 타이핑, 아니면 자리 해제 | `focus_activity.rs` `WAITING_BEFORE_RELEASE` |
| 지정 앱 이탈 유예 | **3초** | 다른 앱이 계속 앞에 있음 | `focus_activity.rs` `FOCUS_GRACE` |
| 새 세션 기준 | **120초** 이상 떠나 있었거나 다른 지정 앱 | — | `focus_activity.rs` `BREAK` |
| 손 흔들기 기준 | 이번 세션 `Active` 누적 **180초**, 떠날 때 등급 무관(자리를 비운 뒤도) | 180초 이상이면 떠나는 순간 누적 0(agent가 자리를 지켜 버려졌어도). 그 미만은 짧은 휴식을 넘어 이어짐 | `focus_activity.rs` `WAVE_AFTER_TYPING` |
| 지정 앱 선언 만료 | **2초** 갱신 없음 → `Away` | 셸이 다시 선언 | `source_state.rs` `STATE_EXPIRY` |
| 지정 앱 샘플 간격 | **0.5초** | — | `RoamlingRuntime.focusSampleInterval` |
| 지정 앱 앞의 앱을 모름(nil) | 모르는 동안. **직전 선언을 이정표만 떼고 다시 낸다** — 만료를 막을 뿐 아무 일도 일어나지 않는다. 시계는 흐름 | 앱이 다시 보임 → 그때 상태에 맞는 선언 | `focus_activity.rs` `observe` |
| 인사 점프 | **0.84초** (도착 반응, 한 번) — Petdex `jumping` 표준. 끝나면 유지 반응 `work`로 | `BehaviorTiming.spark` | `source_state.rs` `reaction_for` |

무제한인 상태는 이제 둘뿐이고, 둘 다 Petdex가 steady로 분류한 것이다 — **작업 중**과
**사용자를 기다리는 중**. 나머지는 전부 유한하고, **Petdex 어휘를 쓰는 것은 Petdex와 같은
길이다.** 규격대로 그려진 남의 펫을 가져와도 제작자가 의도한 속도로 재생되어야 하기
때문이고, 그건 시계가 맞을 때만 참이다.


## 7. 그래서 어떤 그림을 오래 보는가

에이전트를 켜 놓고 일하는 하루 기준, 화면 점유 시간 순서다.

1. **row 7 `running`** — 검색 외 모든 tool 실행 중. 타이머가 없어 툴이 도는 내내 돈다.
2. **row 0 `idle`** — 기본값. 확장 시트가 없는 패키지에서는 `wake` · `stretch` · 커서 응시가 모두 여기로 떨어진다.
3. **row 1/2 걷기** — 배회, 이동, 도망, 잠자리 찾기(자리가 안 좋을 때만).
4. **row 6 `waiting`** — 승인 대기(응답까지)와 끌림. 앉기·잠·잡힘은 확장 행으로 나갔다.
5. **row 8 `review`** — 파일을 읽거나 검색할 때 1.03초씩.
6. **row 3 `waving`** — 턴 완료 0.70초.
7. **row 4 `jumping`** — 턴 시작 0.84초. 착지도 이제 0.84초를 채운다(아래 참조).
8. **row 5 `failed`** — 실패 1.22초.

이전과 비교하면 순위가 뒤집혔다. `review`는 1위에서 5위로 내려갔고(커서 응시가 빠지고
타이머가 생겼다), `waiting`은 1.2초짜리에서 **응답까지 유지**로 올라갔다.

**오래 도는 그림은 이제 `running` 하나다.** 나머지는 전부 짧게 스친다. 작화 기준도 그렇게
갈린다 — `running`은 "수십 초 반복해도 안 거슬리는가", 나머지는 "1~2초 안에 읽히는가".

### 착지는 아무도 못 봤다 — 고쳐졌다 (발견 2026-09-03, 수정 2026-09-04)

**놓으면 착지 동작 없이 곧장 커서 회피로 들어갔다.** Windows에서 처음 눈에 띄었지만 공유
코어의 동작이라 macOS도 똑같았다. 아래는 진단이고, 수정은 이 절 끝에 있다.

```text
놓음 -> finish_drop -> MouseReleased -> BehaviorState::Dropped
                                        timing::DROPPED = 0.840초 유지되도록 되어 있고
                                        landing 트랙은 0.50초짜리로 authored돼 있다

그런데 매 tick이 BehaviorInput::Pointer(근접도)를 넣는다 (pet_runtime.rs)
handle_pointer는 is_held()일 때만 거부한다 -- Dropped는 held가 아니다
-> 다음 tick에 LookAtPointer / EvadePointer 로 전이
```

**놓은 직후에는 커서가 펫 위에 있을 수밖에 없다** — 방금 거기에 놓았기 때문이다. 그래서
근접도가 `Far`일 수가 없고 `Dropped`는 한 tick도 버티지 못한다. 0.84초짜리 상태와 0.5초짜리
authored 트랙이 **구조적으로 도달 불가능**하다.

이것이 의도가 아니라는 증거는 `MascotPetFactory`의 주석이다 — *"Without this, `landing`
falls through to jumping and the pet throws a full celebration every time it is dropped."*
누군가 "떨어뜨릴 때마다 축하 동작이 나온다"를 보고 landing을 따로 그렸다. 그 수정이 의미가
있으려면 landing이 보여야 한다.

**핵심은 이것이다: 놓은 직후의 커서 근접은 신호가 아니라 그 조작의 부산물이다.** 사용자가
다가간 것이 아니라 거기에 놓은 것뿐이다. `is_held()`가 잡는 중에 포인터를 무시하는 것과 같은
이유가 놓은 직후에도 잠깐 서야 한다.

#### 고치는 자리에 따라 파장이 다르다

| 위치 | 다시 만들어야 하는 것 |
|---|---|
| `behavior.rs`의 `handle_pointer` | `behave` fixture 3,654 케이스까지 |
| **`pet_runtime.rs` — 착지 중에는 Pointer를 먹이지 않는다** | **녹화 트레이스만** |

후자가 훨씬 작다. 런타임에 이미 `caught_animation_until` · `click_reaction_until` 같은
"until" 타이머가 있고 `finish_drop`이 그것들을 비우므로, 같은 모양으로 하나 더 두면 된다.

#### 정한 것

- **드롭만 보호한다.** `Spark` · `Celebrate` · `Sad`도 같은 구조지만 그쪽은 커서가 우연히
  온 것이라 반응하는 편이 맞다. 드롭만 커서 위치가 조작의 부산물이다.
- **`timing::DROPPED`(0.84초) 전체를 보호한다.** 착지하고 한 박자 뒤에 사용자를 알아채는
  편이 자연스럽고, 이미 있는 숫자라 새 상수가 생기지 않는다.

#### 어떻게 고쳤나 (2026-09-04)

`pet_runtime`에 `landing_until`을 두고 그 동안 `BehaviorInput::Pointer`를 넣지 않는다.
길이는 `timing::DROPPED` 그대로라 새 상수가 생기지 않았다. 녹화 세션에서 펫이 25 tick
(0.833초) 자세를 유지하고 그 다음 tick에 커서를 알아챈다.

**녹화가 이 버그를 잡고 있지 않았다는 것도 같이 드러났다.** 드래그 중 가짜 커서가
따라가지 않아 놓는 순간 커서가 347포인트 밖에 있었고, 그것이 착지가 방해받지 않는 유일한
거리였다. 커서를 드래그에 붙인 뒤에야 트레이스에 버그가 나타났다.

#### 왜 맥에서 하는가

공유 코어라 macOS도 같이 바뀌고 `RuntimeTrace.txt`(40초 녹화)가 달라진다. `CLAUDE.md`는
"통과시키려고 다시 만들지 않는다"고 못박아 두었는데, **의도한 동작 변경은 정당하게 다시 만드는
경우**다 — 다만 그 재생성과 실사용 확인이 맥에서만 된다.


## 8. 이 흐름을 보고 알 수 있는 것

- **row 6이 겸하는 상태가 다섯에서 둘로 줄었다.** 남은 것은 승인 대기와 끌림이고, 앉기·
  잠·잡힘은 확장 행이 가져갔다. 이제 집어 올리면 화면이 바뀐다.
- 대체로 도는 상태 중 **그릴 그림이 없는 것은 없다.** `gaze`는 커서 거리로 `review` 행의
  재생 속도를 바꾸는 방식으로, 나머지는 확장 시트로 해결됐다.
- **빈 셀은 표준 15칸 · 확장 5칸이다.** 행 단위로는 꽉 찼지만 셀 단위로는 그렇지 않다.

  ```text
  표준 spritesheet.webp (8×9, 57/72)          확장 roamling.webp (8×3, 19/24)
  0 idle           XXXXXX..                    9  sleeping+caught  XXXXXXX.
  1 running-right  XXXXXXXX                    10 sitting          XXXX....
  2 running-left   XXXXXXXX                    11 stretching       XXXXXXXX
  3 waving         XXXX....
  4 jumping        XXXXX...
  5 failed         XXXXXXXX
  6 waiting        XXXXXX..
  7 running        XXXXXX..
  8 review         XXXXXX..
  ```

  확장은 표준 시트의 빈 칸이 아니라 **`roamling.json`이 가리키는 자기 시트**에 둔다.
  그래야 `pet.json`과 `spritesheet.webp`가 9행 계약 그대로 남는다. 인덱스는 패키지 격자
  끝에서 이어지므로 72번이 확장 시트의 첫 칸이다 — 그래서 한 트랙이 두 시트를 섞을 수
  있고, `gaze`가 표준 64~69를 빌리는 것이 그 경우다.
- **지면선이 모든 행에서 175로 맞았다.** `jumping`의 시작·끝 프레임이 25px 가라앉아 턴이
  시작될 때마다 바닥에 잠기던 것이 이때 고쳐졌다. `pet_qa.py`의 면제도 행 단위(`--allow-airborne 4`)
  에서 프레임 단위(`--allow-airborne r4c1`)로 좁혀져서, 지금 면제되는 여덟 프레임은 전부
  실제로 공중에 있다. 정식 게이트는 57프레임 0실패다 — 명령은 `docs/art/mochi-v3-plan.md` 0.5절.


## 9. 확인 방법

- **메뉴 → 펫**에 로드된 패키지의 커버리지(authored / 대체 / 대체 불가)가 뜬다.
- **메뉴 → 진단 기록 복사**로 `pet` 카테고리 전이를 보면 상태는 도는데 그림이 안 바뀌는
  상황이 그대로 드러난다. 위 표의 "같은 그림" 항목들이 여기서 확인된다.
