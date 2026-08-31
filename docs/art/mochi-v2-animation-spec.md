# Mochi v2 애니메이션 기술서 — 지금 무엇이 그려져 있는가

재제작(v3)의 입력 문서다. **행마다 지금 무엇이 그려져 있고, 그것이 두 계약 각각에서
언제 재생되며, 어디가 어긋나는지**를 실측으로 적는다. 그림을 새로 만들기 전에 이 표를
확정한다.

- 흐름과 상태 전이는 `docs/behavior-flow.md`
- 어휘를 어떻게 쌓았는지는 `docs/state-contract.md`
- **무엇을 어떻게 다시 만드는가는 `docs/art/mochi-v3-plan.md`** — 이 문서의 실측이 그 계획의 근거다
- capability 사슬과 대체 규칙은 `docs/pets.md`
- 프레임 작화 불변식은 `docs/art/mochi-animation-handoff.md`

측정 대상: `~/.codex/pets/mochi-v2/spritesheet.webp` (1536 × 1872, 8열 × 9행, 셀 192 × 208).

## 1. 이 시트는 두 계약을 동시에 만족해야 한다

같은 9행이 두 곳에서 재생되는데, **두 곳의 의미가 다르다.**

### Petdex 정본 (검증된 소스)

`petdex-desktop-native/src/hook_runner.zig:235-250`의 `stateForEvent`가 정본이다.

| 훅 phase | 조건 | 재생 행 | 말풍선 |
|---|---|---|---|
| `user-prompt` · `session-start` | — | `jumping` | "Thinking…" |
| `pre` | tool이 Read · Grep · Glob | `review` | "Reading …" |
| `pre` | 그 외 모든 tool | `running` | "Running …" |
| `post` | — | `idle` | "Read …" |
| `tool-failure` | — | `failed` (1220ms 고정) | "… failed" |
| `notification` · `approval-request` · `waiting` | — | `waiting` | "Waiting for you…" |
| `approval-response` · `subagent-start` | — | `running` | — |
| `stop` · `session-end` · `assistant` | — | `waving` | "Done." |
| `subagent-stop` | — | `idle` | — |

**즉 `jumping`은 축하가 아니라 시작 신호이고, `waving`이 완료 신호다.**

그리고 상태에는 두 종류가 있다 (`main.zig:1955` `isDurationState`).

| 종류 | 행 | 동작 |
|---|---|---|
| duration | `waving` · `failed` · `review` · `jumping` | 표준 길이만큼 재생하고 **`idle`로 되돌아간다** (최소 250ms) |
| steady | `idle` · `running` · `waiting` · `running-left/right` | **다음 훅 이벤트까지 유지된다** |

Codex가 설치하는 훅은 다섯 개다 (`agent_hooks.zig:256`): `UserPromptSubmit` ·
`PreToolUse` · `PostToolUse` · `PermissionRequest` · `Stop`. **Codex에는 tool-failure
이벤트가 없어서 Petdex에서 `failed` 행은 Codex로는 절대 안 뜬다.**

`waiting` 진입 시 Petdex는 시스템 알림음도 낸다 (`main.zig`, 에이전트가 사용자에게
막혀 있다는 뜻이므로).

### Roamling

Roamling은 Codex의 상태 이름을 소비하지 않는다. 훅 이벤트를 `CompanionEvent`로 정규화하고
자체 상태기를 돌린 뒤, `PetCapability`를 거쳐 행을 고른다. **그 capability가 어떤 Petdex
행을 뜻하는지는 코드에 선언돼 있고**(`docs/state-contract.md`), duration/steady 분류와
표준 길이도 따라간다.

두 계약은 이제 어휘가 맞다. 남은 차이는 둘이고, 둘 다 의도된 것이다.

| | Petdex | Roamling | 왜 |
|---|---|---|---|
| `post` | `idle`로 되돌림 | 아무것도 안 함 | 툴마다 자세가 튀면 산만하다 |
| `waving` · `failed` 길이 | 0.7초 · 1.22초 | 2.2초 · 1.5초 | 데스크탑 어디에나 있는 펫은 눈에 띄는 데 더 걸린다 |

따라서 **행마다 두 개의 재생 길이가 있다.** 작화는 둘 다 만족해야 한다.

### 프레임 수는 두 벌이 아니다

소비자가 셋인데 타이밍을 읽는 곳과 안 읽는 곳이 갈린다.

| | 매니페스트 타이밍 | 프레임 수 |
|---|---|---|
| Roamling | 읽는다 (`pet.json` → `roamling.json` 순) | 매니페스트 따름 |
| Codex `model.rs` | 읽는다 (`frames`·`fps`·`loop`) | 매니페스트 따름 |
| **Petdex 데스크탑 `sprite.zig`** | **무시한다** | **코드에 고정** |

`sprite.zig`는 행마다 프레임 수와 길이를 하드코딩하고 그 행의 **앞 N칸만** 재생한다.

```text
idle 6 · running-right 8 · running-left 8 · waving 4 · jumping 5
failed 8 · waiting 6 · running 6 · review 6
```

**N보다 많이 그리면 뒤는 아무도 못 보고, 적게 그리면 빈 칸이 한 프레임 깜빡인다.** 같은
표가 `docs/research.md:79`에 ChatGPT.app의 공식 계약에서 확인한 값으로 이미 있다. 공식
최소 매니페스트에는 `animations` 필드가 없다.

반대로 **각 행의 뒤쪽 빈 칸은 어느 소비자도 읽지 않는다.** Roamling 확장 행을 거기 넣는
것이 안전한 이유다.

## 2. 행별 기술

각 행의 "실측"은 알파 채널 bounding box와 프레임 간 픽셀 차이로 계산한 값이다.
"변화율"은 프레임 0 대비 달라진 픽셀 수 ÷ 프레임 0의 전체 픽셀 수.

### row 0 — `idle` (6프레임, 2칸 비어 있음)

```text
매니페스트  frames [0×12, 1, 2, 3, 4, 5] @10fps  loop  → 1.7초 (프레임 0을 1.2초 정지)
Petdex 표준  6프레임 1100ms  steady  "Neutral breathing and blinking loop"
실측        높이 148 고정 · baseline 177 전 프레임 동일 · 중심 91.5~92.9 · 변화율 15~25%
```

앉은 정면 자세. 프레임 2에서 눈이 감긴다(깜박). **다만 "눈만" 깜박이지 않는다** — 중심이
91.5~92.9로 움직이고 프레임마다 실루엣 전체가 1px 내외로 흔들려서 변화율이 15~25%로
나온다. 조용한 호흡처럼 읽히므로 결함은 아니지만, "눈만 깜박이는 idle"이 목표라면 지금
그림은 그 정의가 아니다.

**Roamling 사용처**: `idle` 상태 + `wake` · `stretch` · **`gaze`(커서 응시)** 대체.
배회 사이 8.4~17.4초마다. `gaze`가 여기로 떨어지므로 이전보다 더 자주 보인다.
**판정**: 유지. 커서 응시를 이 그림에서 떼어내려면 `gaze`를 빈 셀에 그린다.

### row 1 · row 2 — `running-right` / `running-left` (각 8프레임)

```text
매니페스트  8프레임 @11fps  loop  → 0.73초        Petdex 표준  8프레임 1060ms  steady
실측        높이 127~143 · baseline 158~174 (편차 16px) · 1px 조각 최대 11개/프레임
            row 2는 row 1의 정확한 mirror (프레임별 차이 수치가 완전히 동일)
```

**시트에서 정합성이 가장 나쁜 행이다.** baseline이 158~174로 흔들려 걷는 동안 펫이 위아래로
떠오르고, chroma 키잉 잔여물인 1px 조각이 프레임마다 최대 11개 붙어 있다.
`docs/research.md`의 "펫이 떠 보인다"가 이것이다.

**Roamling 사용처**: `wander` · `evadePointer` · `findSleepSpot` · `travelToInterest`.
**판정**: 그림 내용은 문제없다. 정합성(baseline · detached) 때문에 재제작 대상.

### row 3 — `waving` (4프레임, 4칸 비어 있음)

```text
매니페스트  4프레임 @8fps  loop  → 0.5초         Petdex 표준  4프레임 700ms  duration
실측        f3이 f0과 완전히 동일 (실제로는 3장) · baseline 174~175
            f2에서 중심이 +3.0px 이탈 (앞발을 들며 몸이 오른쪽으로 밀림)
```

앉은 자세에서 오른쪽 앞발을 들어 올린다. 프레임 2가 정점이고 몸이 같이 기울어 중심이
흐트러진다.

**Petdex 의미**: `stop` · `session-end` — "Done." 완료 인사. 재생 후 idle 복귀.
**Roamling 사용처**: **`celebrate`** — 턴 완료. Roamling에서는 2.2초 유지한다.
0초에서 여기로 바뀌었다.
**판정**: 재설계. 손 들기를 버리고 "다 했어!"로 읽히는 완료 인사로. **2.2초를 버텨야 하므로
4프레임 단발로는 부족하다** — 나갔다 돌아와 정지 자세로 끝나는 구성이 필요하다.

### row 4 — `jumping` (5프레임, 3칸 비어 있음)

```text
매니페스트  5프레임 @10fps  loop  → 0.5초        Petdex 표준  5프레임 840ms  duration
실측        baseline  f0 201 · f1 172 · f2 137 · f3 174 · f4 201
```

**시작 프레임과 끝 프레임이 공통 지면선(176)보다 25px 아래에 있다.** 웅크림-도약-정점-
하강-착지의 아크 자체는 정상인데, 시작·끝이 땅 밑에서 시작한다. 그래서 이 행이 재생될
때마다 펫이 25px 가라앉았다가 돌아온다.

`scripts/pet_qa.py`의 `--allow-airborne 4`가 이 행을 통째로 면제하기 때문에 게이트가
이걸 못 잡는다. `docs/pets.md`의 "jumping도 시작·끝 프레임은 176으로 복귀해야 한다"는
규칙이 검사되지 않고 있다.

**Petdex 의미**: `user-prompt` · `session-start` — "Thinking…" 출발 점프.
**Roamling 사용처**: **`spark`**(턴 시작 0.84초) + `landing`(드롭 착지 0.35초). 완료 축하는
`waving`으로 옮겨 갔다.
**판정**: **baseline 수정 필수.** 두 용도 모두 짧게 스치므로 시작·끝이 지면선에서 25px
어긋나 있으면 가라앉았다 튀는 것으로 읽힌다.

### row 5 — `failed` (8프레임)

```text
매니페스트  8프레임 @8fps  loop:false  → 1.0초    Petdex 표준  8프레임 1220ms  duration
실측        baseline 174 균일 · 높이 139 → 109 → 139 · 변화는 머리와 몸통에 집중
```

눈을 뜬 채 고개와 어깨가 서서히 내려갔다 돌아온다. 네 발은 지면에 붙어 있다.

**Petdex 의미**: tool 실패. Codex에는 해당 이벤트가 없어 Codex에서는 안 뜬다.
**Roamling 사용처**: `sad` (1.5초). Claude의 `postToolUseFailure` · `stopFailure`.
**판정**: 유지. 우리 재생이 1.0초라 Petdex 표준 1.22초보다 짧다.

### row 6 — `waiting` (6프레임, 2칸 비어 있음)

```text
매니페스트  6프레임 @6fps  loop  → 1.0초         Petdex 표준  6프레임 1010ms  steady
실측        baseline 175 균일 · 높이 144~146 · 변화가 머리에 집중
            (머리 2000~2260 / 몸통 800~900 / 다리·꼬리 600~980)
```

앉아서 위를 올려다보며 고개를 한쪽으로 기울였다 돌아온다. 몸통은 거의 고정.

**Petdex 의미**: 승인 요청 · 알림. **steady** — 사용자가 응답할 때까지 유지된다.
**Roamling 사용처**: `paw`(승인 대기, **응답까지 유지**) + `sit` + `sleep` + `caught` +
`dragged`. 행 하나가 다섯 상태를 겸한다.
**판정**: 그림은 좋다. 이제 오래 떠 있으므로 **루프가 거슬리지 않는지**가 기준이 된다.
1.2초 타이머는 제거됐다.

### row 7 — `running` (6프레임, 2칸 비어 있음)

```text
매니페스트  6프레임 @8fps  loop  → 0.75초        Petdex 표준  6프레임 820ms  steady
실측        baseline 175 균일 · 높이 139 → 104 → 138 · 변화율 33~76%
```

3/4 시점으로 두 앞발을 한 점에 대고 상체를 눌렀다 올린다. 뒷발과 지면선은 고정.

**Petdex 의미**: 검색 외 모든 tool 실행 중, 승인 응답 후, 서브에이전트 시작. steady.
**Roamling 사용처**: `work`. 종료 타이머 없음.
**판정**: 유지. 다만 Roamling에서는 오래 도는 두 그림 중 하나라 강도 재검토 여지.

### row 8 — `review` (6프레임 그려짐, 5프레임만 사용, 2칸 비어 있음)

```text
매니페스트  frames [64,65,66,65,64,67,68,67,64] @5fps  loop  → 1.8초
Petdex 표준  6프레임 1030ms  duration (재생 후 idle 복귀)
실측        baseline 175 균일 · 변화가 머리에 집중 · f4에서 폭 105 (좌우로 크게 돌림)
사용 안 함  frame 69 — 눈을 감고 웃는 얼굴이 그려져 있으나 매니페스트가 참조하지 않는다
```

가운데 → 왼쪽 → 가운데 → 오른쪽 → 가운데. 소위 "도리도리".

**Petdex 의미**: `pre` + Read · Grep · Glob — 파일을 읽거나 검색하기 직전. **1.03초 단발.**
**Roamling 사용처**: `observe` — 파일 읽기·검색. **1.03초 뒤 물러난다.** 커서 응시는
`gaze`로 빠져나갔다.
**판정**: 두 계약의 길이가 같아졌으므로 **1초 안에 읽히는 동작**이면 된다. 지금 재생은
9스텝 1.8초라 표준보다 길다. 도리도리를 유지할지는 열려 있지만, 1.03초에 맞추려면 프레임
구성을 줄여야 한다.

## 3. 시트 전체 QA 실측

```sh
./scripts/pyimg.sh scripts/pet_qa.py ~/.codex/pets/mochi-v2/spritesheet.webp --baseline 176
```

| 항목 | 목표 | 실측 |
|---|---|---|
| baseline | 전 행 176, 편차 0 | 137~201 (걷기 158~174, 점프 시작·끝 201) |
| detached component | 프레임당 1개 | 걷기 두 행에 1px 조각 최대 11개/프레임 |
| 중심 x | 95.5 ± 1 | 대부분 통과, `waving` f2만 +3.0 이탈 |
| visible height | 행별 목표 | 104~148 |

57프레임 중 52프레임이 게이트를 통과하지 못한다. 대부분은 `-1` / `-2`의 계통 오차라
전체를 1~2px 내리면 사라지고, 실제 결함은 **걷기 두 행의 baseline 흔들림 · 1px 조각**과
**점프의 25px 침하** 둘이다.

## 4. 남아 있는 자산

```text
0 idle           XXXXXX..     3 waving   XXXX....     6 waiting  XXXXXX..
1 running-right  XXXXXXXX     4 jumping  XXXXX...     7 running   XXXXXX..
2 running-left   XXXXXXXX     5 failed   XXXXXXXX     8 review    XXXXXX..
                                                      빈 셀 15칸
```

`PetLoader.swift:51`은 임의 이름의 애니메이션과 `columns × rows` 안의 임의 프레임 인덱스를
받는다. 8×9 비율이 유지되므로 Petdex 검증도 통과한다. 즉 Roamling이 영영 못 가진다고
적었던 `sleeping` · `sitting` · `caught` · `landing`을 **이 시트 안에서** 채울 수 있다
(4+2+2+3 = 11칸 ≤ 15칸). Codex는 선언하지 않은 이름을 무시하므로 양쪽이 깨지지 않는다.

`review`의 frame 69(눈 감고 웃는 얼굴)도 이미 그려져 있고 쓰이지 않는다.

## 5. 다음 단계

어휘는 고정됐다(`docs/state-contract.md`). 이제 각 행이 **언제, 얼마나 오래** 떠 있는지가
확정돼 있으므로 그것이 작화 기준이다.

1. 위 표에서 **행별 목표 기술**을 확정한다 (무엇을 그릴지 문장으로).
   - `waving` — 2.2초를 버티는 완료 인사. 손 들기는 버린다.
   - `review` — 1.03초 안에 읽히는 "읽는 중".
   - `jumping` — baseline 결함 수정. 시작·끝이 176으로 복귀.
   - `gaze` (신설) — 빈 셀. 커서를 오래 응시하는 조용한 그림.
2. 확정된 문장으로 PixelLab 프롬프트를 만든다.
3. 생성 → `pet_qa.py` 게이트 → 아틀라스 합성 → 승인 기록.
4. 확장 행은 `roamling.json`과 그 파일이 가리키는 자기 시트에 둔다. `pet.json`과
   `spritesheet.webp`는 9종 그대로 둔다.

`output/hatch-pet/mochi-row-review/run/qa/approvals.json`이 승인 상태의 유일한 기록이다.
반려된 후보를 다시 제안하기 전에 사용자에게 확인한다.
