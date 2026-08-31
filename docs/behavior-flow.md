# Behavior flow: 어떤 상황에 어떤 그림이 뜨는가

`docs/architecture.md`는 모듈 경계를, `docs/pets.md`는 capability와 패키지 사이의 간격을,
`docs/art/mochi-v2-animation-spec.md`는 지금 시트에 무엇이 그려져 있는지를 다룬다. 이
문서는 그것들을 **시간 순서로 이어 붙인 것**이다 — 펫이 idle에서 시작해서 무엇 때문에
무엇으로 넘어가고, 그 순간 화면에 실제로 어떤 그림이 뜨는가.

행을 새로 그리기 전에 이 문서를 읽는다. 그림의 좋고 나쁨은 그림만 봐서는 판단할 수 없고,
**그 그림이 몇 초 동안 떠 있느냐**에 달려 있기 때문이다. 0.35초 스치는 자리와 40초 도는
자리는 같은 기준으로 그리면 안 된다.

기준 패키지는 `~/.codex/pets/mochi-v2` (8열 × 9행)이다.

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

## 2. mochi-v2가 실제로 보여주는 것

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
Mochi v3는 네 항목을 확장 시트에서 직접 그린다.

| 상태 | capability | 확장 후 재생되는 행 | 그 그림 |
|---|---|---|---|
| `lookAtPointer` | `gaze` | row 8 `review` 프레임을 **커서 거리에 따라 1.0~2.0배속** | 꼬리 흔들기 — 가까울수록 빨라진다 |
| `sleep` | `sleep` | 확장 `sleeping` 3프레임 | 바닥에 웅크려 숨쉬기 |
| `caught` | `caught` | 확장 `caught` 4프레임 | 목덜미 잡혀 뒷다리 접힌 자세 |
| `sit` | `sit` | 확장 `sitting` 4프레임 | 앉은 채 눈 감기다 꾸벅 |

확장 넷이 들어오면서 row 6이 감당하는 상태는 다섯(승인 대기·앉기·잠·잡힘·끌림)에서
둘(승인 대기·끌림)로 줄었다. 남은 대체는 셋인데, **성격이 같지 않다.**

`landing`과 `dragged`는 의도된 대여다. `landing`은 진짜로 hop이라 점프 행이 맞는 그림이고
(팩토리가 그 프레임을 역순·논루프로 다시 타이밍한다), `dragged`는 `caught`와 물리적으로
같은 상황이다 — `BehaviorController`에서 둘의 차이는 잡힌 펫이 움직이느냐뿐이라 같은
그림이 맞다.

**`stretch`는 아직 안 그린 것이다.** 잠에서 깨는 `wake` 0.7초와 `stretch` 1.0초가 둘 다
`stretch` capability로 가고, 그릴 행이 없어 idle로 떨어진다. 즉 **1.7초 동안 idle만 나온다.**
`sleeping`을 그리기 전에는 자는 자세도 앉은 자세여서 티가 안 났지만, 지금은 바닥에 웅크린
상태에서 곧장 앉아 깜박이는 그림으로 튄다. 기지개는 그 사이를 잇는 그림이므로 이 행은
확장 시트가 생긴 뒤 오히려 더 필요해졌다. 확장 시트에 84~87 네 칸이 비어 있다.

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
     + 이동 중 아님                   └─ 2.4초 뒤 ──▶ findSleepSpot (row 1/2, 0.75배속 걷기)
                                                        │  안전지대까지 걸어감
                                                        │  (에이전트 옆 좋은 자리면 제자리)
                                                        └─ 도착 ──▶ sleep (확장 sleeping, 웅크림)
                                                                     │
                                    입력 0.8초 안에 발생 or 커서 접근 ─┘
                                                                     ▼
                                                              wake (row 0)
                                                                └ 0.7초 ─▶ stretch (row 0, 같은 그림)
                                                                             └ 1.0초 ─▶ idle
```

**여기서 눈으로 확인되는 사실**: 잠들기·자기·깨기·기지개가 전부 이미 보고 있던 두 그림
(row 6, row 0)으로 처리된다. 75초를 기다려 재운 결과가 "앉은 자세 그대로"라서, 지금
빌드에서는 펫이 잤는지 아닌지 사용자가 알 수 없다.

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

## 6. 전이 시간표

| 상태 | 얼마나 머무는가 | 무엇이 끝내는가 | 정의 위치 |
|---|---|---|---|
| `idle` | 8.4~17.4초 | 배회 스케줄러 | `RuntimeTuning.wanderDelay` |
| `wander` | 목적지까지 (160pt/s) | 도착 | `MovementController` |
| `sit` | 2.4초 | 고정 타이머 | `RestConfiguration.sittingDuration` |
| `findSleepSpot` | 목적지까지 (120pt/s) | 도착 | `beginRestTravel` |
| `sleep` | 무제한 | 입력 0.8초 or 커서 접근 | `updateRestLifecycle` |
| `wake` | 0.7초 | 고정 | `BehaviorTiming.wake` |
| `stretch` | 1.0초 | 고정 | `BehaviorTiming.stretch` |
| `dropped` | **0.84초** | 고정 — 점프 행을 빌리므로 Petdex `jumping` 표준 | `BehaviorTiming.dropped` |
| `spark` | **0.84초** | 고정 — Petdex `jumping` 표준 | `BehaviorTiming.spark` |
| `observe` | **1.03초** | 고정 — Petdex `review` 표준 | `BehaviorTiming.observe` |
| `celebrate` | **0.70초** | 고정 — Petdex `waving` 표준 | `BehaviorTiming.celebrate` |
| `sad` | **1.22초** | 고정 — Petdex `failed` 표준 | `BehaviorTiming.sad` |
| `waitingForUser` | **무제한** | 다음 훅 이벤트 (승인·거부) | 타이머 없음 — Petdex steady |
| `work` | **무제한** | 다음 이벤트·커서·배회·휴식 | 타이머 없음 — Petdex steady |
| `lookAtPointer` | 커서가 170px 안에 있는 동안 | 커서 이탈 | `handlePointer` |
| `caught` / `dragged` | 마우스를 놓을 때까지 | mouseUp | `petOverlayMouseUp` |
| 휴식 진입 조건 | 사용자 입력 **75초** 무발생 | — | `RestConfiguration.idleBeforeRest` |

무제한인 상태는 이제 둘뿐이고, 둘 다 Petdex가 steady로 분류한 것이다 — **작업 중**과
**사용자를 기다리는 중**. 나머지는 전부 유한하고, **Petdex 어휘를 쓰는 것은 Petdex와 같은
길이다.** 규격대로 그려진 남의 펫을 가져와도 제작자가 의도한 속도로 재생되어야 하기
때문이고, 그건 시계가 맞을 때만 참이다.


## 7. 그래서 어떤 그림을 오래 보는가

에이전트를 켜 놓고 일하는 하루 기준, 화면 점유 시간 순서다.

1. **row 7 `running`** — 검색 외 모든 tool 실행 중. 타이머가 없어 툴이 도는 내내 돈다.
2. **row 0 `idle`** — 기본값. 여기에 `wake` · `stretch` · **커서 응시**가 모두 포함된다.
3. **row 1/2 걷기** — 배회, 이동, 도망, 잠자리 찾기.
4. **row 6 `waiting`** — 승인 대기(응답까지), 앉기 2.4초, 잠, 잡힘.
5. **row 8 `review`** — 파일을 읽거나 검색할 때 1.03초씩.
6. **row 3 `waving`** — 턴 완료 0.70초.
7. **row 4 `jumping`** — 턴 시작과 착지 각 0.84초.
8. **row 5 `failed`** — 실패 1.22초.

이전과 비교하면 순위가 뒤집혔다. `review`는 1위에서 5위로 내려갔고(커서 응시가 빠지고
타이머가 생겼다), `waiting`은 1.2초짜리에서 **응답까지 유지**로 올라갔다.

**오래 도는 그림은 이제 `running` 하나다.** 나머지는 전부 짧게 스친다. 작화 기준도 그렇게
갈린다 — `running`은 "수십 초 반복해도 안 거슬리는가", 나머지는 "1~2초 안에 읽히는가".


## 8. 이 흐름을 보고 알 수 있는 것

- **row 6이 여전히 다섯 상태를 겸한다.** 승인 대기·앉기·잠·잡힘·끌림이 한 그림이다.
  집어 올려도, 자고 있어도 화면이 안 바뀐다.
- **`stretch`는 그릴 그림이 없다.** 기지개가 idle과 같다. (`gaze`는 커서 거리로 `review`
  행의 재생 속도를 바꾸는 방식으로 해결됐다.)
  `gaze`는 조용해진 대신 아무 반응이 없어졌으므로, 빈 셀의 첫 수요다.
- **빈 셀이 15칸 있다.** 행 단위로는 꽉 찼지만 셀 단위로는 그렇지 않다.

  ```text
  0 idle           XXXXXX..     3 waving   XXXX....     6 waiting  XXXXXX..
  1 running-right  XXXXXXXX     4 jumping  XXXXX...     7 running   XXXXXX..
  2 running-left   XXXXXXXX     5 failed   XXXXXXXX     8 review    XXXXXX..
  ```

  다만 확장은 이 칸이 아니라 **`roamling.json`이 가리키는 자기 시트**에 둔다. 그래야
  `pet.json`과 `spritesheet.webp`가 9행 계약 그대로 남고 확장이 15칸에 묶이지 않는다.
- **`jumping` 행의 시작·끝 프레임이 지면선보다 25px 아래다.** 이제 이 행은 턴 시작과 착지
  둘 다에 쓰이므로 더 자주 보인다. `pet_qa.py`의 `--allow-airborne 4`가 이 행을 면제해서
  게이트가 잡지 못한다. 실측은 `docs/art/mochi-v2-animation-spec.md`에 있다.


## 9. 확인 방법

- **메뉴 → 펫**에 로드된 패키지의 커버리지(authored / 대체 / 대체 불가)가 뜬다.
- **메뉴 → 진단 기록 복사**로 `pet` 카테고리 전이를 보면 상태는 도는데 그림이 안 바뀌는
  상황이 그대로 드러난다. 위 표의 "같은 그림" 항목들이 여기서 확인된다.
