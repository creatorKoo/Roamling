# Placement: where the pet decides to stand

`docs/architecture.md`는 모듈 경계를, `docs/mvp.md`는 게이트를 설명한다. 이 문서는 그
사이에 빠져 있던 것을 다룬다 — **펫이 "어디에 있을지"를 정하는 결정이 실제로 어떻게
흐르는가**, 그리고 그 흐름이 왜 지금 형태로는 계속 버그를 만들어내는가.

MVP 4 게이트 안에서 발견된 결함 다섯 개가 전부 이 흐름에 있었기 때문에 만든 문서다.
1장은 재정비 전 구조, 2장이 진단, 3장이 그 진단대로 바꾼 **현재 구조**다. 1·2장을 남겨
두는 이유는 3장의 규칙마다 그것이 어떤 결함에서 나왔는지가 근거이기 때문이다. 상수 하나를
완화하기 전에 2장의 표를 먼저 읽는다.

## 1. 재정비 전 (as-is)

### 1.1 결정 지점이 네 개다

펫의 위치를 정하는 코드는 `RoamlingRuntime`(1617줄, 함수 60개) 안에 네 곳으로 흩어져
있다. 서로를 호출하지 않고, 각자 입력을 따로 모으고, 같은 가변 필드를 읽고 쓴다.

```mermaid
flowchart TD
    T["tick() 매 프레임"] --> W["watchSeatWhileParked<br/>1초 주기 · 앉아 있을 때"]
    T --> CH{"catch armed?"}
    CH -->|yes| CATCH["포인터가 소유"]
    CH -->|no| EV{"evade 중?"}
    EV -->|yes| EVADE["회피가 소유"]
    EV -->|no| R["updateRestLifecycle<br/>휴식 · 수면"]
    R -->|"true = 휴식이 소유"| DONE1["끝"]
    R -->|false| P{"포인터 근접"}
    P -->|"watching/catchable"| LOOK["쳐다보기"]
    P -->|"slow/fastEvade"| APPLY["회피"]
    P -->|far| A["updateActivityLifecycle"]
    A -->|"활동 있음"| A2["이동 또는 착석 유지"]
    A -->|"활동 없음"| RM["updateRoaming → beginWander"]

    EVT["Claude/Codex 이벤트"] --> HAE["handleActivityEvent"]
    HAE --> DAE["dispatchActivityEvent"]
    DAE --> BAT["beginActivityTravelIfPossible"]
    A2 --> RS["reseatIfBetterSeatAvailable<br/>0.5초 주기 · 이동 중"]

    BAT -.->|"자리 결정 ①"| PLAN["planActivityTravel"]
    RS -.->|"자리 결정 ②"| PLAN2["destination()"]
    W -.->|"자리 결정 ③"| PLAN
    RM -.->|"자리 결정 ④"| RND["randomDestination"]

    classDef decision fill:#fde,stroke:#b47
    class PLAN,PLAN2,RND decision
```

네 개의 결정 지점(분홍)이 서로 다른 규칙을 쓴다. ①②③은 `BasicInterestPositionPlanner`를
쓰고 ④는 `VisualEmptiness`만 쓴다. ④가 늦게 합류한 이유가 2.2에 있다.

### 1.2 우선순위가 두 곳에 따로 적혀 있다

위 그림에서 `watchSeatWhileParked`만 `tick()` 맨 위, **else-if 사슬 바깥**에 있다. 잠든
펫도 화면 변화를 알아채야 해서 그렇게 뒀는데, 그 결과 우선순위 표현이 두 벌이 됐다.

- 사슬 안쪽: catch → evade → rest → pointer → activity → roaming 순서
- 사슬 바깥: 자리 감시가 `pointerOwnedStates`라는 **별도 집합**으로 스스로 양보

두 규칙이 같은 것을 말하지 않는다. `catchArmedUntil`이 켜지는 tick에서는 behavior state가
아직 `.observe`라 감시가 통과하고, 같은 tick 뒷부분의 catch 분기가 방금 깐 경로를 취소한다.
한 tick짜리 창이라 증상은 작지만, **"누가 펫을 소유하는가"의 답이 두 군데 있고 서로
다르다**는 것이 문제다. 결함 5가 여기서 나왔다.

### 1.3 그 네 곳이 공유하는 가변 상태

```mermaid
flowchart LR
    subgraph paths["결정 경로"]
        BAT["beginActivityTravel<br/>IfPossible"]
        RS["reseatIfBetter<br/>SeatAvailable"]
        W["watchSeat<br/>WhileParked"]
        RM["updateRoaming"]
        RL["updateRest<br/>Lifecycle"]
    end
    subgraph state["RoamlingRuntime의 가변 필드"]
        H["activityHint"]
        D["activityDestination"]
        SH["isSeatHoldable"]
        SC["activitySeatSawCapture"]
        LW["lastSeatWatchAt"]
        LR["lastReseatCheckAt"]
        NW["nextWanderAt"]
        CL["cachedLuminance"]
        CF["cachedFocus"]
        RD["restDestination"]
        AS["activeActivitySourceID"]
    end
    BAT --> H & D & SH & SC & NW
    RS --> D & SC & LR
    W --> H & SH & LW & CL
    RM --> NW & CL
    RL --> RD & NW
    H --> W & RS & RL
    SH --> RL
    SC --> RS & W
    CL --> BAT & RS & W & RM
```

읽는 쪽과 쓰는 쪽이 M:N이다. 어떤 경로가 어떤 필드를 세워야 하는지는 코드 어디에도
적혀 있지 않고, 빠뜨려도 컴파일러가 잡지 못하며, 대부분 순수 테스트로도 잡히지 않는다.

### 1.4 캡처와 focus는 비동기·캐시로 들어온다

```mermaid
sequenceDiagram
    participant Ev as agent event
    participant RT as RoamlingRuntime
    participant Cap as MacCaptureProvider
    participant Pl as BasicInterestPositionPlanner

    Ev->>RT: activityStarted
    RT->>Cap: requestLuminanceRefresh (Task 생성)
    Note over RT,Cap: 즉시 반환 — 결과는 나중에
    RT->>Pl: destination(luminance: nil)
    Pl-->>RT: 창 바닥 4개 후보뿐 → 구석
    RT->>RT: 이동 시작
    Cap-->>RT: cachedLuminance 도착 (수백 ms 뒤)
    RT->>Pl: reseat 재평가 (luminance 있음)
    Pl-->>RT: 창 안 빈 자리
    RT->>RT: 경로 수정
```

측정값(실제 데스크톱, 1728×1117): 캡처 없이 계획하면 화면 왼쪽 끝에서 **66pt**, 있으면
**332pt**. 앞쪽 자리는 emptiness 0.51로 실제로 글자 위였다.

## 2. 여기서 나온 결함들

MVP 4 안에서 발견된 다섯 개는 서로 다른 증상이었지만 원인 형태가 같다 — **경로 A가
세워야 할 필드를 세우지 않았고, 경로 B가 그걸 읽는다.**

| # | 증상 | 원인 | 형태 |
|---|---|---|---|
| 1 | 세션 내내 화면 갱신에 무반응 | `planActivityTravel`이 false를 반환한 경로에서 `activityHint` 미설정 → `watchSeatWhileParked`가 영구 정지 | 플래그 누락 |
| 2 | 잘못된 자리에서 잠듦 | 도착 시 `isSeatHoldable = true`로 단정 | 검증 없는 낙관 |
| 3 | 구석에 고정 | 캡처 도착 여부를 기록하지 않아 눈감고 내린 결정이 hysteresis로 고정 | 결정의 출처 정보 소실 |
| 4 | 평상시 본문 위에 앉음 | 배회 경로가 다른 세 경로와 완전히 분리돼 emptiness를 아예 안 봄 | 규칙 이중화 |
| 5 | 커서 근처에서 판정 정지 | 이동 금지와 판정 금지를 같은 가드로 처리 | 관심사 혼합 |

### 2.1 순수 테스트가 잡지 못한다

1·2·3·5는 전부 상태 전이 타이밍 버그라 `RoamlingLogicTests`에서 재현할 수 없었다.
결정 로직이 `@MainActor` 클래스의 가변 필드에 얹혀 있고, 그 필드를 세우는 것이 부수효과이기
때문이다. 실제로 이 넷은 전부 **사용자가 앱을 실행해서 발견**했다. 이게 지금 구조가
치르고 있는 가장 큰 비용이다.

### 2.2 규칙이 두 벌 존재한다

MVP 4의 "빈 공간에 앉는다"는 규칙은 `BasicInterestPositionPlanner`에만 들어갔고, 배회
목적지는 `randomDestination()`이 별도 규칙으로 골랐다. 그런데 Claude Code는 턴마다
`Stop`을 쏘고 → `clearActiveActivity` → 2초 뒤 배회가 시작된다. 즉 **펫이 실제로 보내는
시간의 대부분이 규칙 바깥**이었다. 측정하니 무작위 목적지가 빈 자리에 앉을 확률은 49%였다.

### 2.3 좋은 자리에서도 계속 움직인다 (3.2의 5번으로 해결)

유지 판정이 임계치 하나(`holdEmptiness = 0.55`)이고 체류 시간 개념이 없다. 에이전트가
출력하는 동안 펫 밑 점수가 0.56 ↔ 0.54로 오가면 1초마다 자리를 옮기고, 옮길 때마다
"그 순간의 최선"을 새로 고르니 자리가 계속 튄다. 눈에는 멀쩡한 자리인데 24pt 셀 점수가
한 번 내려간 것이다.

`docs/architecture.md`의 `VisualSafeZoneProvider` 절은 이미 "dwell/hysteresis가 지나야
위치를 바꾼다"고 적어 두었다. 설계 의도는 처음부터 있었고 구현만 빠져 있었다 — 결정
지점이 네 개라 넣을 자리가 정해지지 않았기 때문이다. 지금 상태로 고치면 같은 히스테리시스를
네 곳에 네 번 넣게 된다.

## 3. 지금 구조

`Sources/RoamlingCore/PlacementDirector.swift`가 3장 전체다. 결정 표는
`PlacementDirector.decide(_:)` 하나에 우선순위 순서 그대로 들어가 있고,
`Tests/RoamlingLogicTests/CoreLogicTests.swift`가 각 행을 케이스로 고정한다.

### 3.1 결정을 한 곳으로

```mermaid
flowchart LR
    subgraph mac["RoamlingMac — 어댑터"]
        GATHER["상황 수집<br/>pointer · focus · capture<br/>activity · behavior · 시각"]
        APPLY["의도 적용<br/>route · behavior · movement"]
    end
    subgraph core["RoamlingCore — 순수"]
        SIT["PetSituation<br/>값 타입"]
        DIR["PlacementDirector<br/>결정 상태기"]
        INT["PlacementIntent<br/>enum"]
    end
    GATHER --> SIT --> DIR --> INT --> APPLY
    DIR -.->|"자리 채점"| PL["BasicInterestPositionPlanner<br/>VisualEmptiness"]
```

`RoamlingRuntime.tick()`은 **수집 → 결정 → 적용** 세 단계다 — `makeSituation` →
`placement.decide` → `apply`. 1.3의 17개 필드 중 배치 결정에 쓰이던 것은 director 안의
`seat` / `travel` 두 값으로 흡수됐고, 런타임에 남은 것은 어댑터가 원래 소유해야 하는
캐시(`cachedFocus`, `cachedLuminance`)와 배치 밖에서도 쓰이는 페이싱(`nextWanderAt`)뿐이다.

결정은 **소유권과 무관하게 매 tick 돌아간다.** 1·2번 우선순위는 `.none`을 돌려주지만
그 앞의 판정은 이미 끝나 있어서, 포인터가 손을 떼는 tick에 곧바로 최신 답으로 움직인다.
판정과 이동을 같은 가드로 막은 것이 결함 5였다.

**그래서 판정은 자기가 쓰는 상태를 소모하면 안 된다.** 버려질 답을 만드느라 타이머를
되감으면, 손을 떼는 tick에 남아 있는 것이 최신 답이 아니라 처음부터 다시 시작한 대기다.
10'번이 `parkedSince`를 이렇게 소모했다 — 3.2.1 참조.

### 3.2 결정 표

`PlacementDirector`가 답하는 질문은 하나다 — *지금 어디 있어야 하는가.* 우선순위 순으로
읽는다. 위쪽이 항상 이긴다.

| 우선순위 | 조건 | 의도 |
|---|---|---|
| 1 | 잡힘 / 끌림 | `.none` — 포인터가 소유 |
| 2 | 회피 중 | `.none` — 회피가 소유 |
| 3 | 이 소스에 대한 자리가 아직 없음 | `.travel(reason: .newActivity)` |
| 4 | 자리가 캐럿을 덮음 | `.travel(reason: .coveringCaret)` |
| 5 | 자리 emptiness < `abandonEmptiness` **그리고** 체류 시간 경과 | `.travel(reason: .coveringWork)` |
| 6 | 캡처 없이 정한 자리 + 캡처 도착 | `.travel(reason: .plannedBlind)` |
| 7 | 보는 창이 바뀜 | `.travel(reason: .followedFocus)` |
| 8 | 활동 중 + 자리 유지 가능 + user idle 경과 | `.sleepInPlace` |
| 9 | 활동 중 + 자리 유지 가능 | `.hold` |
| 10 | 활동 없음 + 배회 시각 도래 | `.stroll(to:)` — 후보 6개를 emptiness로 거름 |
| 10' | 활동 없음 + 지금 자리가 덮임 | `.escape(to:)` — 후보는 같고, 커서에게 양보하지 않는다 |
| 11 | 그 외 | `.hold` |

3번의 질문은 "펫이 그 창을 보고 있는가"다. 보고 있지 않으면 무조건 걸어간다 — 다른
디스플레이에 있는 경우가 대표적이고, 이건 점수로는 안 나온다. 실측하면 2번 모니터의
구석 자리가 1번 모니터의 깨끗한 자리를 **6.9점** 차이로 이기는데, 마진(15) 아래라
점수만 봤으면 엉뚱한 모니터에 눌러앉는다. 그래서 `watchesRegion`을 직접 묻는다.

이미 그 창을 보고 있다면, 새 자리가 마진만큼 확실히 나을 때만 옮긴다. caret이 있으면
끌림이 최대 40점이라 자동으로 넘고, 없으면 남는 건 하단 선호 12점뿐이라 자동으로 진다.
**"작업 위치를 못 찾으면 제자리 유지"가 조건문이 아니라 점수의 결과로 나온다.**

이게 필요한 이유는 Electron 앱이다. 실측(Paseo, `sh.paseo.desktop`)에서 AX가 주는
caret은 `(0, 33, 0, 0)` — 크기 0이라 `usableCaret`이 버린다. focused element는 화면
89% 지점의 입력창이다. 즉 창 안 어디가 작업 영역인지 알려주는 신호가 하나도 없고, 창은
전체화면이라 "창 하단"이 곧 "화면 하단 구석"이 된다. 신호가 없을 때 구석으로 걸어가는
것보다 서 있던 빈 자리에 있는 편이 낫다.

같은 소스의 두 번째 이벤트부터는 자리가 이미 있으므로 3번이 아예 켜지지 않는다 —
tool call마다 재계획하던 것이 여기서 끝난다.

한편 "보고 있다"의 범위(`holdRegionMargin`)는 **플래너가 창 옆에 앉히는 거리보다 넓어야
한다.** 창 밖 후보는 가장자리에서 `halfWidth + 14`(기본 펫 62pt)에 놓이는데 범위가
48pt 고정이면, 방금 고른 자리를 다음 판정에서 "이 창을 안 본다"고 판단해 또 옮긴다.
전체화면 창에서는 그 후보가 화면 밖으로 잘려 안 드러났다. 지금은 펫 크기에서 유도한다.

5번에 대해 이동한 뒤에는 최소 체류 시간(`seatDwell` 2.5초) 동안 5번이 다시 켜지지 않는다.
4번(캐럿)은 체류 시간을 기다리지 않는다 — 사용자가 방금 클릭한 자리를 2초 더 덮고 있는
것이 이 문서가 막으려는 바로 그 동작이다.

5번의 이탈 기준은 착석 기준과 **같은 `holdEmptiness`(0.55)다.** 더 낮은 이탈 기준을
한 번 넣어 봤고 실측으로 물렸다 — 4장의 probe로 실제 데스크톱(1728×1117)을 재보면
점수 분포가 이렇다.

| emptiness | 화면 비중 | 정체 |
|---|---|---|
| < 0.35 | 87 cell | 빽빽한 글자 |
| 0.35 ~ 0.55 | 28 cell | **성긴 글자 — 여전히 사용자의 작업물** |
| > 0.55 | 65 cell | 여백·벽지 |

이탈 기준을 0.35로 두면 가운데 구간이 통째로 "글자 위인데 앉아 있어도 되는 자리"가 된다.
화면의 15%다. 1.4의 실측(글자 위 자리 = 0.51)도 정확히 이 구간에 있다.

그래서 2.3의 진동은 기준을 낮춰서가 아니라 **원인 쪽에서** 막는다. 펫이 계속 움직였던
이유는 애매한 자리에서 **또 다른 애매한 자리로** 옮겼기 때문이다. 새 자리도 같은 줄에서
점수가 오르내리니 다음 판정에서 또 옮긴다. 그래서 5번은 새 자리가 **그 자체로 유지 가능할
때**(emptiness ≥ 0.55) 이동하고, 그렇지 않으면 `replacementMargin`(15점)만큼 확실히
나을 때만 이동한다. 같은 probe에서 깨끗한 자리는 0.973로 나온다 — 한 번 옮기면 다시
기준 아래로 내려올 일이 없으므로 이동이 한 번으로 끝난다.

실측으로 확인한 결과: 글자 위(0.525)에 앉힌 펫이 `coveringWork`로 0.973 자리로 옮기고,
정지 화면 20초 동안 추가 이동 0회.

### 3.2.1 배회 중에도 자기 자리를 본다

10번이 "배회 시각 도래"만으로 발동하던 동안, 펫은 **산책과 산책 사이 내내 눈을 감고
있었다.** 목적지는 emptiness로 골라 놓고, 앉은 뒤에 그 위로 글자가 차오르는 것은 아무도
안 봤다. 기본 `wanderPause` 12초면 8.4~17.4초, 사용자가 40초로 올리면 28~58초다.
배회를 조용하게 만들수록 글자를 오래 덮는 구조였다.

그래서 10번은 자리가 덮여도 발동한다. 5번과 같은 기준을 그대로 쓴다 — `holdEmptiness`
아래로 내려가고, `seatDwell`이 지났고, **갈 곳이 그 자체로 깨끗할 때만.** 규칙을 새로
만들지 않는 것이 2.2의 교훈이다.

걷는 중이거나 자는 중이면 발동하지 않는다(`isWalking` · `isResting`). 각각 이미 다른
주인이 있는 상태라, 여기서 경로를 새로 깔면 그 주인과 싸운다.

캡처 주기는 6초다. 에이전트 자리(3초)의 절반인데, 스크롤에 가려지는 것은 급한 상황이
아니고 캐럿을 덮는 경우는 4번이 즉시 처리하기 때문이다. 실측으로 캡처 1회가 **62ms**
(median 61.5, n=20)라 6초 주기는 시간의 1.03%다. 3초로 올리면 2.06%가 된다.

실측 확인: 배회 타이머를 끈 채로 화면에서 가장 빽빽한 지점(emptiness 0.000)에 펫을
앉히면 `seatDwell` 직후 0.706 자리로 한 번 이동하고 멈춘다.

```mermaid
stateDiagram-v2
    [*] --> Roaming
    Roaming --> Traveling: 활동 시작
    Traveling --> Seated: 도착
    Seated --> Seated: hold (기본)
    Seated --> Traveling: 자리가 나빠짐 / 눈감고 정함 / 창 바뀜
    Seated --> Napping: user idle + 자리 양호
    Napping --> Traveling: 자리가 나빠짐
    Napping --> Seated: 입력 복귀
    Seated --> Roaming: 활동 종료
    Napping --> Roaming: 활동 종료 후 기상
    Traveling --> Roaming: 활동 종료
```

### 3.2.2 커서는 이 걸음을 못 막는다

10'번은 `.stroll`이 아니라 `.escape`다. 둘은 목적지를 고르는 방식이 같고 **누구에게
양보하는지가 다르다.**

커서를 쳐다보려고 멈추는 것은 순간이고, 사용자의 글자를 깔고 있는 것은 상태다. 순간이
상태의 해결책을 취소하면 안 된다. 실제로는 지연보다 나빴다 — 응시가 경로를 취소하기
때문에(`movement.cancelRoute`), 펫은 커서가 붙잡은 그 자리에 서 버렸고 그 자리는 방금
떠나던 문단 한가운데였다.

세 군데가 같이 고장나 있었다.

1. **판정이 사라졌다.** `decide`는 포인터가 펫을 소유하는 동안 모든 답을 버리는데,
   10'번이 답을 만들면서 `parkedSince`를 비웠다. 커서가 옆에 머무는 한 대기는 성숙할
   때마다 되감겼고, 펫은 커서가 치워질 때까지 글자 위에 있었다. 이제 `parkedSince`는
   실제로 걷기 시작할 때(`isWalking`) 비워진다.
2. **경로가 취소됐다.** 이제 `.escape` 경로가 살아 있는 동안 바깥 대역은 tick을
   가져가지 못한다. 런타임의 `escapeOutranksPointer`가 판단한다.
3. **걸음을 시작하지도 못했다.** 커서가 **이미** 옆에 서 있으면 펫은 응시 상태고,
   `wanderEntryStates`가 그 상태에서의 `.beginWander`를 거절했다. 즉 이동 중에 끊기는
   경우를 고쳐도, 정지한 채로 덮이는 경우는 그대로 남았다 — 그리고 이쪽이 더 흔하다.
   펫이 글자 위에 있다는 건 거기가 사용자가 작업하는 곳이라는 뜻이고, 그러면 커서도
   거기 있다.

이 세 번째가 소유권 모델을 갈랐다. `PetSituation`의 포인터 신호는 이제 둘이다.

| 신호 | 뜻 | 배치가 지는가 |
|---|---|---|
| `isPointerOwned` | 잡힘·끌림·회피, 또는 손이 닿는 거리(≤100px, `catchable`) | 항상 진다 |
| `isPointerWatching` | 바깥 대역(100~170px) 응시 | `.escape`에만 진다 |

`wanderEntryStates`에 `.lookAtPointer`를 넣은 것도 이 갈래 때문이다. 응시 중에 배치가
내주는 경로는 `.escape` 하나뿐이므로(다른 답은 전부 `.none`이 된다), 그 상태에서 걸음을
허용해도 한가한 산책이 새어 나오지 않는다.

**바깥 대역만 양보한다.** 회피와 잡기는 그대로 펫을 소유한다 — 하나는 펫을 옮기고
하나는 집어 올리므로, 둘 다 펫을 글자 위에 버려둘 수 없다.

### 3.3 이 구조가 막는 것

- **결함 1·3 유형** — hint와 결정 출처(`seat.sawCapture`)가 director 안에 있으므로,
  경로마다 세우고 빠뜨리는 일이 성립하지 않는다. 결정 경로가 하나뿐이다.
- **결함 4 유형** — 배회도 같은 결정 표(10번)를 지나므로 규칙이 두 벌 생기지 않는다.
  목적지를 고를 때뿐 아니라 이미 앉은 자리에도 같은 기준이 적용된다(3.2.1).
- **상수를 감으로 만지는 것** — 3.2의 표는 4장의 probe로 잰 값이다. 이탈 기준을 한 번
  낮췄다가 같은 probe로 되돌렸다. 이 문단이 그 기록이다.
- **결함 5 유형** — "판정"과 "이동"이 분리된다. 표는 항상 평가되고, 1·2번 우선순위가
  이동만 막는다.
- **2.1의 비용** — `PlacementDirector`가 순수 값 타입이라 결정 표 전체를 테스트로 고정할
  수 있다. 실행해야만 발견되던 타이밍 버그가 이제 `./scripts/test.sh`에서 잡힌다. 표의
  각 행에 케이스가 하나씩 있고, 결함 3·4·5는 재현 케이스로 남아 있다.

### 3.3.1 director가 자기 상태를 잃지 않는 방법

값 타입 상태기가 새로 만들 수 있는 고장은 "아무도 도착을 알려주지 않아서 영원히 걷는
중" 하나다. 그래서 도착은 통보가 아니라 **관측**이다 — `decide`가 매 tick 위치와 목적지
거리를 보고 `arrivalTolerance` 안이면 그 자리를 seat으로 삼는다. catch나 evade가 경로를
지워도 어댑터가 같은 목적지로 경로를 다시 깔기 때문에 이동이 이어지고, 그래도 끝나지
않으면 거리와 보행 속도로 계산한 timeout이 제자리 착석으로 되돌린다. 셋 다 director
안에 있어서 호출자가 잊을 수 있는 절차가 아니다.

### 3.4 건드리지 않는 것

pet 로딩·카탈로그, 애니메이션, 오버레이, 메뉴, 튜닝 창, 훅 인스톨러, 소스 어댑터.
`MovementController` · `BehaviorController` · `PointerInteractionModel`도 그대로 둔다 —
이미 순수하고 이번 결함들과 무관하다. catch/drag/evade 경로의 동작도 바꾸지 않는다.
`PlacementDirector`는 그 경로들이 활성일 때 `.none`을 돌려주고 비켜선다.

## 4. 진단 방법

배치가 이상할 때 상수부터 만지지 않는다. 실제 데스크톱 좌표와 실제 스크린샷으로 배포된
플래너를 그대로 돌려보는 것이 먼저다. `RoamlingCore`는 OS 비의존이므로 SwiftPM 산출물에
직접 링크해 재생할 수 있다.

```sh
swiftc -O probe.swift -I .build/debug/Modules \
    .build/debug/RoamlingCore.build/*.o -o probe
```

probe에서 `NSScreen`으로 디스플레이를, `CGWindowListCopyWindowInfo`로 전면 창 bounds를
읽고(둘 다 권한 불필요), `screencapture`로 받은 PNG를 `MacCaptureProvider`와 같은 방식으로
축소해 `LuminanceField`를 만든 뒤 `BasicInterestPositionPlanner.destination`을 호출한다.
캡처를 nil로 한 번, 채워서 한 번 돌리면 두 경우의 자리를 직접 비교할 수 있다.

이 방법으로 결함 3의 원인(66pt vs 332pt)을 확정했다. 그전 두 번은 합성 이미지로 상수를
추정했고 둘 다 빗나갔다.
