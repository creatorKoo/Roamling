# Windows port: 무엇이 이미 넘어가 있고, 무엇이 막혀 있는가

`docs/architecture.md`의 "Future migration"이 provider 매핑 표를 남겨뒀다. 이 문서는 그
표를 실행 계획으로 승격한 것이다 — **지금 코드가 macOS에 얼마나 묶여 있는지 실측한 결과**,
그로부터 나오는 두 개의 결정, 그리고 게이트 순서.

측정 시점은 2026-09-01, `main` 기준이다.

## 1. 실측 — 이식 비용은 모듈마다 다르다

| 모듈 | 크기 | 플랫폼 의존 | 이식 비용 |
|---|---|---|---|
| `RoamlingCore` | 3,156줄 / 20파일 | `import Foundation` 뿐 | **0** |
| `RoamlingSources` | 950줄 / 8파일 | `Network`, `/usr/bin/curl` | 작다 |
| `RoamlingPet` | 2,094줄 / 10파일 | `CoreGraphics`/`ImageIO` (4파일) | **크다** |
| `RoamlingMac` | 3,272줄 / 12파일 | AppKit 전면 | 다시 쓴다 |

`RoamlingCore`에 플랫폼 타입이 하나도 없다는 것이 이 포트의 전제다. 경계를 지켜온
값이 여기서 돌아온다.

### 좌표계는 이미 맞다

Core world plane이 **top-left, y-down**이다(`Sources/RoamlingCore/CoordinateSpace.swift`).
이건 Windows 가상 화면 좌표계와 같은 규약이라 `DesktopCoordinateSpace`가 Windows에서는
사실상 항등 변환이 된다. AppKit의 좌하단 원점은 Mac adapter 밖으로 새지 않았다.

`WorldPoint`가 negative x/y를 허용하는 것도 그대로 필요하다 — Windows 가상 화면도 primary
monitor 왼쪽/위에 있는 디스플레이에 음수 좌표를 준다.

### 테스트는 거의 그대로 돈다

`RoamlingLogicTests`가 XCTest가 아니라 dependency-free executable이라 Windows에서 그대로
빌드된다. W0 시점의 예외 둘 중 `SourceLogicTests.swift`는 W3가 풀었다. 남은 하나는
`ImageIO`로 시트를 디코드하는 `PetLogicTests.swift`이고, **W2가 아니라 W2b가** 푼다 —
디코딩이 플랫폼 서비스가 되면서 하네스도 자기 디코더를 갖게 됐기 때문이다.

## 2. 블로커는 세 개다

### B1. 앱의 두뇌가 macOS 모듈에 있다 — **W1에서 해소 (2026-09-02)**

아래는 W1 착수 전의 진단이다. 지금 `RoamlingRuntime`은 `RoamlingEngine`에 있고
`PlatformServices` 하나만 받는다.


`Sources/RoamlingMac/RoamlingRuntime.swift`는 1,718줄이고 AppKit **타입**이 나오는 곳은 넷뿐이다:

- `NSObject` 상속
- `PetOverlayViewDelegate` 3개 메서드의 `NSPoint`
- `NSApplication.didChangeScreenParametersNotification` 관찰
- `corePoint(fromAppKitScreenPoint:)`

**그러나 진짜 이음새는 타입이 아니라 소유권이다.** 런타임은 `init`에서 Mac provider 8개를
직접 생성하고(`MacDisplayProvider`, `MacBasicSafeZoneProvider`, `MacUserIdleProvider`,
`MacCaptureProvider`, `MacPointerProvider`, `MacWindowProvider`, `MacFocusProvider`,
`MacOverlayProvider`), 그중 6개를 **`PlatformServices.swift`의 프로토콜에 없는 멤버로**
호출한다:

| provider | 프로토콜 밖에서 쓰는 것 |
|---|---|
| Display | `snapshotSet()` — 프로토콜의 `currentDisplays()`는 한 번도 안 불린다 |
| SafeZone | 동기 `currentSafeZones(in:)` — 프로토콜의 async 버전은 안 불린다 |
| Window | `currentActivityLocationHint()` |
| Focus / Capture | `isAuthorized`, `requestAuthorization()` |
| Overlay | `scale`, `objectSize`, `setScale`, `setHitRegionScale`, `setFrameImage(CGImage?)`, `containsPet(atWorldPoint:)`, `view`, settable `coordinateSpace` |

순수하게 프로토콜로만 쓰이는 건 `MacUserIdleProvider`와 `MacPointerProvider` 둘뿐이다.
그래서 Windows adapter를 아무리 잘 써도 런타임이 그걸 못 받는다 — 자기 것을 직접 만들기
때문이다. **프로토콜을 실제 호출 모양으로 고치고 provider를 주입받는 것**이 W1의 본체이고,
`NSPoint` 치환은 그 뒤에 남는 잔업이다.

나머지 전부가 플랫폼 비의존 오케스트레이션이다. 이대로 두면 Windows는 1,700줄을 복제하거나
포크된다. **Windows를 하지 않더라도 고칠 값어치가 있는 항목**이고, 이 포트에서 가장 큰
단일 작업이다.

### B2. `RoamlingPet`의 공개 API가 `CGImage`다 — **W2에서 해소 (2026-09-02)**

아래는 W2 착수 전의 진단이다. 지금 `PetAsset.atlas`는 `PetImage`(RGBA8 premultiplied,
위 행부터)이고 `RoamlingPet`은 Foundation만 import한다.

`PetAsset.atlas`가 `CGImage`이고(`Sources/RoamlingPet/PetAsset.swift`), 아틀라스 합성과
프레임 크롭이 `CGContext`/`CGImageSource` 위에 있다. 모듈 경계상 공용이어야 하는 계층이
macOS 전용이다.

### B3. WebP 디코더가 Windows에 없다

내장 Mochi 아틀라스가 `.webp`이고, 더 중요하게 **Petdex/Codex 펫 패키지의
`spritesheet.webp`는 항상 webp다.** 내장 에셋만 PNG로 바꿔도 `~/.codex/pets` 로딩이 안 된다.
macOS에서는 ImageIO가 공짜로 해주던 일이라 결정이 필요한 줄 몰랐던 항목이다.

**Windows가 대신 해주지 않는다는 것을 2026-09-02에 확인했다.** WIC는 PNG·JPEG·GIF·BMP·TIFF를
내장하지만 **WebP는 Microsoft Store의 "WebP Image Extensions"를 깔아야** 붙는다. 사용자
머신에 그게 있다고 가정할 수 없으므로 **진짜 디코더를 직접 실어야 한다.** W2가 디코딩을
`PetImageSourcing` 뒤로 밀어 둔 것은 이 문제를 없앤 것이 아니라 **결정을 언어 결정과 같은
자리로 옮긴 것**이다 — A로 가면 libwebp를 SwiftPM C 타겟으로 벤더링하고(약 4만 줄),
D로 가면 Rust `image` 크레이트가 WebP와 PNG를 함께 준다.

### 그 밖의 작은 것들

- ~~`LoopbackHookReceiver`의 `NWListener`~~ — **W3에서 해소**
- ~~두 hook installer의 하드코딩된 `/usr/bin/curl`~~ — **W3에서 해소**
- ~~`PetCatalog`의 `Library/Application Support/Roamling/Pets`~~ — **W3에서 해소**
- `scripts/test.sh`가 zsh, `scripts/build-app.sh`가 codesign 전제
- ~~`Localizable.strings`를 `Bundle.module`로 읽는 경로~~ — **9절에서 해소됐다.
  Windows에서 그대로 동작하므로 할 일이 아니다.**

## 3. 결정

### 왜 지금 다시 여는가

`docs/architecture.md`의 "Swift core now, extraction later"가 원래 결정이다.

> **Chosen:** Swift pure module. boundary와 tests는 얻되 미확인 Windows 요구를 위해
> FFI를 선행하지 않는다.

Swift가 최선이라서가 아니라 **의도적인 유예**였다. Windows가 아직 가정일 때 FFI 세금을
미리 내지 않고 경계와 테스트만 확보해 둔 것이고, 그 규율이 지켜져서 3,156줄이 되도록
Core에 플랫폼 import가 들어가지 않았다. 지금 선택지를 논할 수 있는 것이 그 결과다.
**"Windows가 실제가 되면 다시 연다"가 계획이었고 지금이 그 시점이다.**

### 선택지는 넷이다

**Options:**

| | 코드베이스 | Swift가 Windows에서 빌드돼야? | Win32 난이도 | 새로 쓸 코드 | 영구 비용 |
|---|---|---|---|---|---|
| **A. Swift 단일** | 1벌 | 예 | 높음 | ~1,500줄 | 없다 |
| **A′. Swift core + C# 쉘** | 1.5벌 | 예 | 낮음 | ~2,000줄 | 두 언어 seam |
| **B. C# 전체** | **2벌** | 아니오 | 낮음 | ~13,000줄 | **로직 영구 중복** |
| **C. Rust 전면 재작성** | 1벌 | 아니오 | 중간 | ~16,000줄 | macOS까지 재검증 |

**~~Chosen: A.~~ 2026-09-02에 D로 바뀌었다. 아래 "결정: D" 절을 읽는다.** 아래는 A를
고른 당시의 근거이고, 무엇이 틀렸는지가 다음 결정의 정보이므로 지운다.

이유는 "Swift가 Windows에서 좋아서"가 아니다 — **포팅 대상의 88%가 UI가
아니기 때문이다.** 1절 표대로 진짜 macOS 전용 코드는 1,540줄뿐이고 나머지 11,481줄
(로직 7,932 + 테스트 3,549)은 이미 쓰여 있고 이미 게이트를 통과했다. 포트의 어려운 부분은
작지만 하필 Swift가 제일 약한 영역이고, 큰 부분은 Swift가 제일 강한 영역이다.

재구현 안(B·C)의 숨은 비용은 코드가 아니라 **검증**이다. MVP 0~0.7의 값들 — walk 40pt/s,
pause 12초, catch radius 74pt, notice 170pt — 은 유닛테스트가 아니라 사용자가 3-display
앞에 앉아 닫은 값이다. 재구현하면 그 판정을 전부 다시 받아야 하고, "귀엽다"는 회귀
테스트로 잡히지 않는다.

### 결정: D — Rust core + Swift macOS 셸 (2026-09-02)

**사용자가 Windows 지원을 확정했고, 그것이 A를 무너뜨린다.** A는 Swift-on-Windows에 건
베팅인데 그 대가가 셋이다 — 17파일 56 MB 배포(10절), libwebp 약 4만 줄 벤더링(W2b),
그리고 툴체인 추진력(Browser Company 인수 이후, 12절). D는 셋을 동시에 없앤다.

12절과 W0m.3이 D의 세 축을 이미 닫았다: 포팅 정확성(402 world, 불일치 0), FFI 비용
(tick당 0.03%, 아틀라스 크로싱 15.3 ms를 실행당 두 번), 두 언어 빌드·서명(번들 안의 서명된
실행 파일이 rpath로 Rust dylib을 로드해 실제로 동작).

**C로 끝까지 가지 않는 이유.** D에 도달한 뒤 macOS 셸까지 Rust로 옮기면 더 얻는 것은
**"macOS 빌드의 언어가 하나"뿐**이다. 단일 파일은 못 얻는다 — macOS는 언어와 무관하게
`.app` 번들이어야 한다(`LSUIElement`, TCC가 권한을 붙이는 `CFBundleIdentifier`,
`NSScreenCaptureUsageDescription`, 서명·notarize가 전부 번들 전제). Windows 단일 exe와
WebP는 D가 이미 준다. 대가는 `NSPanel` 오버레이·Spaces·fullscreen·TCC·ScreenCaptureKit·
`AXUIElement` 캐럿을 objc2로 다시 만들고 **전부 재검증**하는 것이고, 12절 W0m.1이 그중
캐럿과 실제 캡처 프레임은 재지 않았다.

**그래서 C는 계획하지 않되 닫지도 않는다.** 다시 여는 조건은 둘 중 하나다 — Swift 툴체인이
macOS에서 실제로 문제를 일으키기 시작하거나, `RoamlingMac`이 충분히 얇아져 재작성 비용이
재검증 비용을 밑돌 때(W3b가 608→189줄로 줄인 방향이 계속될 경우).

### 조각내서 갈아탄다 — 빅뱅으로 옮기지 않는다

**항상 실제로 쓰이는 로직은 1벌이고, 어느 시점에 멈춰도 앱이 돈다.** 한 단위를 Rust로
옮기고 → macOS가 Rust판을 부르기 시작하고 → **Swift 원본을 대조군으로 남겨** 같은 입력에
같은 출력이 나오는지 확인하고 → 일치하면 Swift판을 지운다.

대조가 필수인 이유는 W0m.2에 있다. `BasicSafeZonePlanner`는 **Swift `max(by:)`가 동점에서
마지막 원소를 돌려준다**는 것까지 맞춰야 14,070개 질의에서 불일치 0이 나왔다. tie-breaking을
그대로 옮기지 않으면 조용히 다른 모서리에 앉는다. **포팅이 기계적이라는 말이 안전하다는
뜻은 아니다.**

경계는 두 모양이다. 가끔 불리는 것은 **A. 계산만 넘긴다**(값 변환, 측정 3.9 µs). 매 tick
불리는 것은 반드시 **B. 상태를 Rust가 들고 핸들로 부른다**(측정 4.7 µs) — A로 하면 60 Hz에
변환 비용이 곱해진다.

| 단위 | 내용 | 줄 | 모양 |
|---|---|---:|---|
| 1 | Geometry + CoordinateSpace ✅ 2026-09-02 | 175 | Rust 내부용 |
| 2 | BasicSafeZone + DesktopWorld + DisplayTopology ✅ 2026-09-03 | 499 | A |
| 3a | VisualEmptiness + CandidateScoring ✅ 2026-09-03 | 207 | A |
| 3b | InterestPlacement ✅ 2026-09-03 | 229 | A |
| 4 | AttentionModel + ReactionPolicy + Activity ✅ 2026-09-03 | 289 | **B** |
| 5a | Movement + Pointer + Behavior + Timing ✅ 2026-09-03 | 576 | **B** |
| 5b | **PlacementDirector** ✅ 2026-09-03 | 551 | **B** |
| 6a | RuntimeTuning ✅ 2026-09-03 | 244 | A |
| 6b | 활동 오케스트레이션 ✅ 2026-09-03 | 244 | **B** |
| 6c | tick 본체 (rest · roaming · evade · 배치 적용) ✅ 2026-09-03 | ~800 | **B**, tick 2회 |
| 7 | 애니메이션 해석·재생 (resolver · player · PetdexState) ✅ 2026-09-03 | 480 | A + 플레이어는 B |

단위 4도 B가 됐다 — `AttentionModel`은 어느 소스를 보고 있었는지와 언제 떠났는지를,
`ReactionPolicy`는 마지막 반응 시각을 tick 사이에 들고 있다. Swift가 핸들만 쥐고 이벤트만
넘긴다. 선택된 이벤트는 **id만 돌려받는다** — Swift가 자기 표에서 되찾으므로 이벤트 전체를
마샬링해 돌려보낼 이유가 없다.

**단위 5는 5a와 5b로 갈랐다.** 5a는 tick 루프가 매 프레임 미는 세 state machine
(`MovementController` · `PointerInteractionModel` · `BehaviorController`+`BehaviorTiming`)이고,
5b가 그 위에 앉는 `PlacementDirector`다. "매 tick 불리는 것"과 "그것을 지휘하는 것"의
경계로 자르는 편이 검증이 선명하다. (이 표에 있던 969줄은 실측이 아니라 어림이었다 —
여섯 파일을 세어 보면 1,371줄이다.)

**단위 6은 6a·6b·6c로 갈랐고, 여기서 방법이 한 번 바뀐다.** 지금까지는 "Swift 원본을 Core에
대조군으로 남기고 런타임이 Rust를 부른다"였는데, **런타임 자신은 그 방법으로 옮길 수 없다** —
스위치를 쥔 상위 호출자가 없다. 그래서 6b는 런타임의 private 필드 7개와 메서드 10개를 먼저
`SwiftActivityDirector`로 **들어올려** 타입으로 만들고, 그것을 대조군으로 삼았다. 런타임의
사본은 같은 커밋에서 지웠으므로 살아 있는 구현은 여전히 1벌이다.

**6b는 답을 effect 목록으로 돌려준다.** 활동 디렉터는 펫을 움직일 수 없다 — movement ·
behavior · placement 핸들은 런타임 것이다. 그래서 `[CancelRest, SettleInPlace, CancelRoute,
SetNextWanderAt, ApplyReaction, RequestLuminance]` 중 필요한 것을 **순서대로** 돌려주고
런타임이 수행한다. **순서가 곧 답이다** — setback은 자리를 확정하고, 경로를 끊고, *그 다음*
반응한다. 버릴 코드가 아니다: 6c에서 런타임이 건너가면 이 effect들이 직접 호출로 바뀐다.

플랫폼 호출은 규칙과 분리했다. "이 이벤트에 창 위치를 물어볼 가치가 있는가"는 Rust가 답하고
(`wants_window_hint`), 실제 질의는 Swift가 한다 — 그 질의가 동기 왕복이라서.

**`RuntimeTuning`은 5b에서 빼서 6a가 됐다.** 다른 단위는 Swift 원본을 Core에 대조군으로
남기고 **런타임이** Rust를 부르는 모양인데, tuning은 타입 자체가 API다 — clamp가 `init`에
있고 `RuntimeTuning.standard`·Codable 디코드가 Core 안에서 그 `init`을 직접 부른다.
`RoamlingCore`는 `RoamlingEngine`을 못 부르므로(의존 방향은 항상 바깥 → Core), Rust가
clamp를 맡으려면 타입을 Engine으로 **옮겨야** 하고 그러면 대조군이 사라진다. 살아 있는
구현이 항상 1벌이라는 규칙과 대조 테스트 중 하나를 포기해야 하는 자리라서, 타입의 실제
주인인 런타임이 건너갈 때 같이 옮긴다. 6a에서 타입을 `RoamlingEngine`으로 올리고 규칙을 `tuning.rs`로 옮겼다.
Codable과 11개 저장 필드는 그대로라 읽는 쪽은 아무것도 안 바뀐다. clamp가 순서에 의존하므로
(`catchArmDistance`는 **이미 clamp된** awareness에 걸린다) fixture 13,701 케이스 중 대부분이
범위 밖 값이다 — clamp가 실제로 걸릴 때만 순서가 보인다.

**6c에서 방법이 또 한 번 바뀌었다 — 대조군을 쓰지 않는다.** 런타임 위에는 스위치를 쥔
호출자가 없고, 6b처럼 들어올려 대조군을 만들면 600줄짜리 전사(轉寫)를 하나 더 만들어 그
전사가 맞다는 것만 증명하게 된다. 그래서 **실물을 녹화했다**: fake provider 위에서 진짜
런타임을 40초 돌린 것 — 배회 · 커서 접근 · 낚아채기 · 드래그 · agent 턴 한 바퀴 · 낮잠 ·
디스플레이 추가 — 을 tick 단위로 적어 `Tests/RoamlingLogicTests/RuntimeTrace.txt`에 커밋하고,
포팅 후 **바이트 단위로 같은 답**을 요구한다. behavior state 19종 중 18종이 그 안에 나온다.

녹화가 가능하려면 세 가지를 먼저 고쳐야 했다. (1) **무작위성을 인자로 받는다** — 시스템
생성기를 5곳에서 직접 불러서 매 실행이 다른 세션이었다. (2) **`start(drivingTicks:)`** —
tick 타이머와 agent 이벤트 배달이 같은 run loop에 있어서, 배달을 위해 loop를 돌리면 그
사이에 tick이 몇 번 도는지 예측할 수 없었다. (3) `drainActivityEvents`가 실제 시간을
진행시키지 않게. 셋 다 Windows 셸에도 필요한 것들이다 — 프레임 루프를 이미 가진 플랫폼은
런타임의 타이머를 쓰지 않는다.

**tick이 2회 호출로 갈라졌다.** 중간에 플랫폼에 물어봐야 하는 것이 있기 때문이다:
`begin_tick`이 "이번 tick에 accessibility 왕복 비용을 낼 가치가 있는가"를 답하고(창을 보고
있을 때만, 0.5초에 한 번), 셸이 물어본 뒤 `finish_tick`에 넘긴다. 캡처 요청도 같은 이유로
출력으로 나간다 — 권한 · Task · 스로틀은 결정이 아니라 플랫폼의 몫이다.

**런타임은 1,664줄에서 667줄로 줄었다.** 남은 것은 결정이 아닌 것들뿐이다 — 타이머,
UserDefaults, 진단 파일, agent 구독, 스프라이트 시트.

**남았던 6c 정의는 아래와 같았다.** rest lifecycle · roaming · evade · 배치 적용(`apply` ·
`travelToSeat` · `holdSeat`). 콜백으로 방향을 뒤집을 필요는 없었다 — 플랫폼에 물어볼 것이
두 개뿐이라 tick을 2회로 가르는 것으로 충분했다.

**단위 7에서 그리기까지 갔다.** capability → 트랙 → 프레임 인덱스를 잇는 resolver와
player, 그리고 Petdex 9행의 어휘가 `animation.rs`에 있다. tick이 답한 capability를
Windows 셸이 아틀라스 칸으로 바꾸는 데 필요한 것은 이제 전부 Rust에 있다.

여기서도 **정렬 안 된 컬렉션이 결정을 하고 있었다 — 네 번째다.** 패키지가 `idle` 행을
선언하지 않으면 resolver가 `tracks.values.first`로 대역을 골랐는데, Swift는 딕셔너리 순회
순서를 프로세스마다 바꾼다. 두 행짜리 패키지가 **실행할 때마다 다른 행을 그렸다.** 이번엔
포팅이 아니라 fixture가 첫 줄에서 잡았다.

**W4에 남은 것은 매니페스트 로딩과 아틀라스다** — `PetLoader` 246줄, `PetManifest` 162줄,
`PetCatalog` 108줄, `MascotPetFactory` 621줄. 마지막 것은 내장 마스코트의 트랙을 코드로
짓는 것이라 MVP 0의 Windows가 실제로 쓸 물건이다. 디코더는 W2b이고 D에서는 `image` crate
한 줄이다.

**5a가 A 대 B 비용 주장을 실제로 검증한 자리다.** tick당 크로싱 8회로 재보니 Rust 경로가
4.706 µs/tick, Swift 원본이 0.099 µs/tick(둘 다 release) — 차액 4.6 µs는 60 Hz 프레임 예산의
**0.03%**이고, 12절이 미리 잰 4.7 µs와 같은 값이다. 벤치는 `output/w-unit5/bench.swift`.

**5b는 grid를 tick마다 보내지 않는다.** `PetSituation`은 world 전체를 들고 있고 그 안의
luminance grid는 64열 × 40행 = 2,560개 double이다. tick마다 보내면 20 µs/tick인데,
디스플레이 목록과 grid를 **바뀔 때만** 밀면(캡처는 3초에 한 번) 2.75 µs/tick이다 — 7배
차이고, 이는 **바꾸기 전 Swift director의 2.86 µs와 같다.** Swift director는 review 때마다
scene을 마샬링해 Rust planner를 불렀으므로, 방향을 뒤집어 상태를 Rust에 두니 크로싱이
오히려 줄었다. 벤치는 `output/w-unit5/bench-director.swift`.

5a에서 **경계 자체가 결함을 하나 냈고, 대조 테스트가 잡았다.** `RustPointerModel`이
`init` 안에서 `configuration`에 대입했는데 **Swift는 `init` 안의 대입에 `didSet`을 실행하지
않는다** — 핸들이 첫 튜닝 변경 전까지 기본값(awareness 170 / catch 74)으로 남아서 펫이
커서를 잘못된 속도로 봤다. 알고리즘은 fixture가 비트 단위로 맞다고 증명한 뒤였다. 크로싱은
fixture가 증명해 주지 않는다.

`PlacementDirector` 551줄은 원래 단위 3에 있었는데 5로 옮겼다 — tick 사이에 상태(자리, 여정,
마지막 리뷰)를 들고 있어서 호출마다 변환하는 A가 아니라 상태를 Rust가 들고 핸들로 부르는
B다. 3b는 그 director가 **묻는 대상**(`InterestPlacing`)만 Rust로 넘겼다.

`RoamlingCore`의 잎이 얇아서 이 순서가 성립한다 — `MovementController`는 `WorldPoint`와
`WorldVector`만 알고 `DesktopWorld`를 모른다.

**단위 2에서 macOS가 처음 갈아탔고, 배선이 예상보다 단순했다.** dylib 대신 **정적 링크**를
쓰면 W0m.3이 측정한 rpath·`install_name` 교정·재서명이 전부 필요 없다 — 실행 바이너리에
Rust 심볼 109개가 들어가고 동적 의존은 0, 번들은 8.4 → 9.0 MB. `scripts/build-rust-core.sh`가
uniffi 바인딩을 생성하고 정적 아카이브를 놓으며, `test.sh`와 `build-app.sh`가 먼저 부른다.
C 모듈은 `systemLibrary` 타깃이어야 한다 — `-I` 플래그는 그 타깃 안에서만 유효해서
`RoamlingEngine`까지 전파되지 않는다.

전환 확인은 **두 구현을 나란히 돌려 비교하는 테스트**가 한다(200개 배치, 400+ zone과 rest
destination 전부 일치). Swift 원본은 그 대조군으로 남아 있고, 지울 때 이 테스트도 같이 간다.

**differential fixture는 경계값을 일부러 심어야 한다.** 5a의 fixture를 난수만으로 만들었더니
`age >= BehaviorTiming.x`를 `>`로 바꾸는 변이 7개가 전부 살아남았다 — 부동소수 난수가 정확히
공표된 길이에 떨어질 리 없기 때문이다. `enteredAt + length`를 그냥 쓰는 것으로도 부족하다
(`(e+l)-e != l`). 그래서 생성기가 `t - base == offset`이 성립하는 double을 nextUp/nextDown으로
찾아 쓰고, 7개 transient 상태 각각을 길이 -1ulp / 정확히 / +1ulp 세 지점에서 tick한다. 그
스윕을 넣은 뒤 변이 7개가 모두 죽는다.

**5b에서 같은 문제가 더 크게 나왔다.** director의 답 6종 중 난수 시뮬레이션이 낸 것은
4종뿐이고, `escape`(사용자 문단 위에서 비켜서기)는 8,699줄에 한 번, `coveringCaret`·
`coveringWork`(앉은 자리가 나중에 틀려짐)는 0번이었다. 원인은 세 가지였고 전부 실측으로
찾았다 — (1) emptiness는 밝기가 아니라 **평탄도**를 재는데(이웃 차이 0.02면 이미 "꽉 참")
난수 필드는 어디나 busy라 앉을 자리가 아예 없었다. 배경은 평평하게 두고 busy 사각형을
몇 개 얹는 식으로 바꿨다. (2) `escape`는 2.5초의 parked dwell을 요구하는데 run이 34 tick
× 1/30초라 도달 자체가 불가능했다. (3) 도착 후의 자리는 **정의상** caret과 content를 피해
고른 것이라, 화면이 그 위에서 바뀌지 않는 한 두 규칙은 발동하지 않는다.

그래서 fixture 꼬리를 **스크립트**로 만들었다: 앉힌 다음 caret을 펫 위로 옮기고, 창을
채우고, 8초 타임아웃을 넘기고, arrival tolerance에 정확히 서 본다. 이 절을 넣은 뒤
intent 6종·travel 사유 5종이 모두 나오고, 타임아웃·arrival `<=`·caret 규칙·abandon clamp
변이가 전부 죽는다. **타임아웃은 사용자가 제보한 제자리 걷기 버그를 구조하던 바로 그
분기다** — 난수만으로는 한 번도 실행되지 않았다.

### 양 플랫폼이 붙는 방식이 다르다

```text
macOS  :  Swift 셸  --FFI(uniffi)-->  Rust core
Windows:  Rust 셸   --직접 호출-->     Rust core   (같은 crate)
```

**Windows에는 FFI가 없다.** Rust가 Rust를 부르므로 경계도 직렬화도 없고, 오늘 측정한
4.7 µs/tick은 **macOS에만 붙는 비용**이다.

그 대신 **Windows는 조각 단위로 시작할 수 없다.** Rust 셸이 부를 상대는 오케스트레이터인데
그것이 아직 Swift면 부를 것이 없고, Rust는 Swift를 부르지 못한다. W4에 필요한 양은
**Core 2,463 + Engine 1,453 + Pet 1,587 ≈ 5,500줄**이다(`RoamlingSources` 1,010줄은 MVP 0에
필요 없으므로 나중에 — 그것을 가능하게 하려고 agent 주입 이음새를 먼저 넣었다).

**조각내기가 사주는 것은 "Windows가 빨리"가 아니라 "안전하게, 그리고 언제든 멈출 수 있게"다.**
대조 테스트가 계속 어긋나거나 두 언어 빌드가 예상보다 아프면 **W4를 Swift로 하면 되고**,
그 경우에도 W1~W3b는 하나도 버려지지 않는다.

### 착수 전에 정한 것

- **테스트는 로직을 따라간다.** 지금 3,663줄이 Swift 하네스다. Core가 Rust로 가면
  `cargo test`로 같이 가야 differential test를 쓰고 버릴 수 있다.
- **FFI는 tick당 한 번, 스냅샷 in → 지시 out.** 프로퍼티마다 부르는 모양으로 새면 측정한
  숫자가 무너진다. W1의 `PlatformServices`가 이미 그 모양이므로 지키기만 하면 된다.
- **`RoamlingPet`은 로직과 함께 간다.** W2b의 디코더가 Rust `image`이므로 자연스럽다.
- **빌드·서명**: `build-app.sh`에 cargo 단계, `install_name`을 `@rpath`로 교정(W0m.3에서 실측).

### A′는 A의 대안이 아니라 대피로다

경계 후보 타입이 전부 `Codable`이라(`DisplaySnapshot`, `WindowSnapshot`,
`PointerSnapshot`, `FocusSnapshot`, `DesktopWorldSnapshot`, `WorldPoint`, `WorldRect`)
구조체 마샬링을 손으로 쓸 필요가 없다. 경계는 "스냅샷 in → 표시할 프레임 out"에 둔다.
프레임 rect 표를 시작할 때 한 번 넘기면 C# 쪽은 아틀라스 규격을 몰라도 되고
`CLAUDE.md`의 행별 프레임 수 계약이 두 언어로 갈라지지 않는다.

다만 **A′도 Swift가 Windows에서 빌드돼야 한다** — Core를 Windows DLL로 만들어야 하므로
W0의 1·2번은 그대로 남고 회피되는 것은 4번뿐이다. 대가는 한 앱에 두 언어·두 빌드
시스템·두 디버거, `RoamlingPet` 2,094줄의 분할, 그리고 Swift 런타임 DLL과 .NET 런타임을
모두 싣는 가장 무거운 배포다.

### Rust는 포팅 결정이 아니라 재작성 결정이다

**C를 "A가 실패하면 가는 대피로"로 두지 않는다.** 대피로는 원래 계획보다 작아야 하는데
C는 신규 ~16,000줄에 더해 **이미 동작하는 macOS 앱까지 갈아엎는다.** Swift-on-Windows가
막혔다는 사실이 C를 싸게 만들어주지도 않는다 — C의 비용은 처음부터 그 값이다. A의 작은
대피로는 A′다.

장기적으로 C가 최선의 최종 형태인 것은 맞다. 1벌이고, 바이너리 하나로 배포되며(Swift는
Windows에서 런타임 DLL 동봉), `windows-rs`가 Microsoft 공식이고, `image` 크레이트가
WebP를 그냥 디코드해 B3가 사라진다.

**다만 배터리 이득은 기대만큼 크지 않다.** `docs/architecture.md`의 성능 모델대로 이 앱의
비용은 active travel의 60Hz 재그리기와 capture 주기(1회 62ms, 3~6초 간격)가 지배하며 둘 다
언어와 무관하다. Swift도 ARC가 붙은 네이티브 코드지 VM이 아니다. 실제 이득은 배포 단순함과
메모리 소폭, 그리고 1벌이라는 구조다.

**판단 시점은 "A가 실패했을 때"가 아니라 "Swift 코드베이스가 제 역할을 다했을 때"다.**
MVP 4는 2026-09-02에 닫혔고 사다리는 사실상 여기서 멈춘다 — MVP 5(저작 UI)는 이름만 있는
항목이라 필요해질 때 정의하고, MVP 6은 존재하지 않는다. 그래도 재작성 결정은 지금 하지
않는다: 아래 "리팩터가 먼저"대로 W1·W2가 어떤 언어로 가든 이식 명세가 되므로, 그 둘을
끝낸 뒤 실제 경계를 보고 결정하는 편이 싸다.

### W0가 판정표다

**2026-09-01에 W0를 실행했고 분기는 A로 닫혔다. 실측치는 9절에 있다.** 아래는 그때
세워둔 판정 기준이며, 되돌아와 재검토할 때를 위해 남긴다.

```text
W0 1·2번(툴체인/Core 빌드/테스트) 통과 + 4번(layered window) 통과
        -> A. Swift 단일. 가장 단순하다.

W0 1·2번 통과 + 4번이 Swift에서 지옥
        -> A'. Core는 살리고 UI만 C#으로.

W0 1·2번부터 막힘
        -> B. Swift on Windows 자체를 포기한다.

어느 경우든 C는 여기서 고르지 않는다.
```

### 리팩터가 먼저, Windows 코드는 나중

**Options:** Windows adapter를 먼저 세우고 맞춰 리팩터, 리팩터를 먼저 끝내고 adapter 작성.

**Chosen:** 리팩터 선행. W1·W2는 Windows 코드가 0줄이고 전부 macOS에서 검증된다. 이걸 먼저
끝내면 Windows 작업이 "adapter 채우기"로 줄어든다. 순서를 뒤집으면 검증되지 않은 두 플랫폼
위에서 동시에 리팩터하게 된다.

**W1·W2는 A·A′·B 어느 쪽으로 가도 버려지지 않는다.** Runtime을 macOS 모듈에서 빼내고
`PetAsset`을 `CGImage`가 아닌 데이터 포맷으로 만드는 일은, 어떤 언어로 포팅하든
"무엇을 재구현해야 하는가"를 정의해준다. C로 가더라도 그 경계가 이식 명세가 된다.
즉 이 리팩터는 언어 선택에 건 베팅이 아니다.

## 4. 게이트

`docs/mvp.md`와 같은 규칙이다 — 한 게이트의 exit 조건을 닫기 전에 다음으로 넘어가지 않는다.

### W0 — 스파이크 ✅ 완료 2026-09-01 (버리는 코드, **Windows 머신에서 했다**)

**결과는 9절에 있다. 아래는 실행 전에 세운 질문이고, 예측이 빗나간 곳은 그대로 둔다 —
무엇을 잘못 예상했는지가 다음 게이트의 정보다.**

1. Windows 툴체인에서 `RoamlingCore`가 빌드되는가 (`Package.swift`의 `.linkedFramework`에
   `.when(platforms:)` 조건이 필요할 것이다) — **빌드된다. 조건은 필요 없었다.**
2. `RoamlingLogicTests`가 Windows에서 통과하는가 — **Core 72개 통과**
3. **`Bundle.module`이 리소스를 찾고 `.lproj`/`Localizable.strings`가 읽히는가** — **읽힌다**
4. `WinSDK` import로 layered window에 per-pixel alpha가 찍히는가 — **찍힌다**

3번과 4번이 진짜 미지수라고 봤는데, 실제로는 **둘 다 통과했고 예상하지 못한 곳(툴체인
환경변수, 그리고 남은 공백인 다중 디스플레이 미검증)에서 비용이 나왔다.**

#### 4번은 Rust로도 만든다 — 대조군

오버레이 창이 이 포트에서 가장 위험한 단일 항목이다: per-pixel alpha + click-through +
always-on-top + 멀티모니터 + per-monitor DPI v2. 이걸 Swift(`WinSDK`)와
Rust(`windows-rs`) **양쪽으로 각각 200줄쯤** 만든다.

Swift 하나만 해서 실패하면 **Swift 탓인지 Windows 탓인지 구분되지 않는다.** 판정은 이렇다:

- 둘 다 된다 → 제품 요구가 Windows에서 성립한다. Swift가 얼마나 더 아픈지 실측치를 얻는다
- Rust만 된다 → 문제는 Windows가 아니라 Swift다. A′ 또는 B로 간다
- 둘 다 안 된다 → 언어 문제가 아니라 **요구사항을 다시 봐야 한다.** 가장 중요한 정보다

**엄격히 오버레이 창만 만든다.** 로직을 Rust로 옮겨 "느낌을 보는" 순간 그것이 C의
시작이고, C는 W0에서 고르는 선택지가 아니다.

### W1 — Runtime 추출 (macOS, 동작 변화 0) ✅ 완료 2026-09-02

구현이 끝난 같은 날 사용자가 서명 빌드를 실사용해서 달라진 점이 없다고 확인했고, 그것으로
exit가 닫혔다. 테스트 126개가 통과한다. 실제로 생긴 것:

- `Sources/RoamlingEngine/` — `RoamlingRuntime`, `PlatformServices`,
  `PetOverlayProviding`/`PetOverlayInputHandling`, `BasicSafeZoneProvider`
- `Sources/RoamlingMac/MacPlatform.swift` — `makeServices()` 한 함수. Windows 쪽 대응물이
  이것 하나가 된다
- `CoordinateSpaceSource`(Core) — provider가 읽고 런타임만 쓰는 공유 좌표계.
  `handleDisplayChange`가 오버레이에 좌표계를 손으로 밀어 넣던 줄이 사라졌다
- `scripts/test.sh`의 import 게이트
- `Tests/RoamlingLogicTests/RuntimeLogicTests.swift` — 가짜 provider로 런타임을 만들고
  클럭을 손으로 감아 rest 경로 전체를 통과시키는 테스트 2개. 둘 다 mutation으로 확인했다

아래는 착수 시점에 정한 게이트 정의다.

`RoamlingRuntime`을 AppKit 없이 컴파일되는 새 모듈 **`RoamlingEngine`**으로 옮긴다.
클래스 이름은 그대로 둔다 — 모듈과 타입이 같은 이름이면 Swift에서 서로를 가린다.

**목표는 Windows 작업을 "`PlatformServices` 채우기"로 줄이는 것이다.** 부수적으로,
로직 하네스가 가짜 provider로 런타임을 만들어 `tick()`을 돌릴 수 있게 된다 — 지금은
앱을 실행하지 않고는 오케스트레이션 한 줄도 검증할 수 없다.

**In scope**

- B1의 표대로 `PlatformServices.swift`의 프로토콜을 **런타임이 실제로 호출하는 모양**으로
  고친다. `MacDisplaySnapshotSet` → Core의 `DisplaySnapshotSet`, SafeZone은 async를 버리고
  동기, Focus/Capture에 `isAuthorized`/`requestAuthorization()`.
- 새 타겟 `RoamlingEngine`: 런타임, DI 구조체 `PlatformServices`,
  `PetOverlayProviding`/`PetOverlayInputHandling`, `BasicSafeZoneProvider`
  (이미 Foundation만 쓰므로 같이 옮긴다).
- `NSObject`·`@objc` selector 타이머·`NSApplication` 알림·`NSPoint` delegate 제거.
  타이머는 클로저 `Timer`, 화면 변경은 `DisplayChangeObserving`.
- 하네스 seam: `clock`·`UserDefaults`·`PetCatalog` 주입, public `tick()`, 상태 읽기 프로퍼티.
- `scripts/test.sh`에 AppKit import 게이트.

**Out of scope** — 동작·타이밍·기본값·진단 문자열 변경 전부. `CGImage`는 W2까지,
`Network`는 W3까지 그대로 둔다.

**Acceptance**

- `Sources/RoamlingEngine`에 `import AppKit|Cocoa|SwiftUI|ScreenCaptureKit|ApplicationServices`
  가 없다. **컴파일러는 이걸 못 잡는다** — macOS SDK에 AppKit이 있어 링크 설정 없이도
  빌드된다. `scripts/test.sh`가 grep으로 실패시킨다.
- `RoamlingMac`에 남는 것: Mac provider들 + 오버레이 패널 + `MacPlatform.makeServices()` +
  메뉴/튜닝 UI + app delegate + 로컬라이즈 문자열.
- 기존 테스트 전부 + 런타임을 실제로 tick 시키는 테스트 최소 1개가 통과한다.
- app delegate가 쓰는 public API는 그대로다 (`init`만 `init(services:)`).
- identity 서명 빌드에서 Accessibility·Screen Recording 권한이 유지된다.

**Exit** — 서명 빌드를 실사용해서 **달라진 점을 사용자가 못 느낀다**: 배회 · 포인터 회피 ·
잡기 · 드래그 · agent 착석 · 취침 · 디스플레이 연결 변경 · 권한 프롬프트 · scale 변경.

### W2 — 이미지 파이프라인 탈-CoreGraphics (macOS) ✅ 완료 2026-09-02

**원래 계획은 디코더까지 한 게이트에 묶는 것이었는데 둘로 갈랐다.** 문서의 B2(공개 API
탈-`CGImage`)와 B3(WebP 디코더)는 성격이 다르다 — B2는 어느 언어로 가도 이식 명세가 되고,
B3는 4만 줄짜리 C를 저장소에 들이는 결정이라 **언어 결정과 같은 결정**이다. 그래서 W2는
B2까지, 디코더는 W2b로 미뤘다.

실제로 생긴 것:

- `PetImage` — RGBA8 premultiplied, 위 행부터, 행 패딩 없음. `CGContext`가 이미 만들던
  레이아웃이라 경계를 건너는 바이트가 예전에 화면에 닿던 바이트와 같다
- `PetFrame` — 시트 위의 사각형. 프레임을 복사하지 않는다(아래 참조)
- `PetImageCanvas` — 합성용 블리터. nearest-neighbour + 수평 미러
- `PetImageSourcing` — 디코딩과 placeholder 드로잉. macOS 구현은 `MacPetImageSource`
- `RoamlingPet`은 Foundation만 import하고 `RoamlingEngine`은 프레임워크를 하나도 링크하지
  않는다. `scripts/test.sh`의 import 게이트가 이미지 프레임워크까지 막는다
- `PreW2FrameHashes` — 바뀌기 전 파이프라인에서 뜬 336개 프레임 해시

**두 가지는 고쳐 쓰지 않고 지켰다.**

1. 오버레이는 이미 보여주는 프레임을 다시 받으면 재그리지 않고, 그 판단을 **항등성**으로
   한다. 프레임을 복사본으로 만들면 매 tick이 새 프레임으로 보여 60Hz로 재그린다. 그래서
   프레임은 사각형으로 남기고, Mac 쪽이 시트당 `CGImage` 하나를 캐시해 셀마다 crop한다 —
   `CGImage.cropping`이 공짜로 주던 것과 같은 구조다. 셀을 복사하면 펫당 16 MB가 붙는다.
2. `PlaceholderPetFactory`의 시트는 안티에일리어싱된 베지어 아트라 어떤 이식 가능한
   블리터도 재현하지 못한다. **매니페스트와 트랙은 데이터라 남기고 드로잉만**
   `RoamlingMac`으로 한 줄도 바꾸지 않고 옮겼다.

**게이트: 336프레임 전부 바이트 동일.** 내장 mochi(96) · 내장 fat-mochi(56) ·
shipped `mochi-v3` 패키지(96) · placeholder(88). 크롭에 1픽셀 오프셋을 넣으면 테스트가
즉시 실패하는 것을 mutation으로 확인했다. 실행 중인 앱의 RSS는 75.8 MB → 80.9 MB(+5.1 MB).

**Exit**: 서명 빌드 실사용에서 펫이 전과 같아 보인다 — 2026-09-02 사용자가 확인했다.

### W2b — 이식 가능한 디코더 (B3)

**Windows가 실제로 빌드될 때, 또는 언어 결정 뒤에 연다.** 지금은 `PetImageSourcing`의
구현이 macOS 하나뿐이고 테스트 하네스도 자기 ImageIO 디코더를 쓴다 — 1절이 말한
"`PetLogicTests`의 ImageIO 의존"은 W2가 아니라 여기서 없어진다.

경로는 언어 결정을 따른다. **A**면 libwebp 디코더 서브셋 + PNG/zlib(miniz)을 SwiftPM C
타겟으로 벤더링한다(BSD/zlib 라이선스는 GPL-3.0-only와 호환, 약 4만 줄). **D**면 Rust
`image` 크레이트가 WebP·PNG를 함께 주므로 벤더링이 사라진다 — 12절이 만든
`PetImageSourcing` 이음새 뒤에 Rust 디코더를 꽂는 것이 그 확인 방법이고, 아틀라스가
1.5 MB라 FFI 직렬화의 최악 경우를 그대로 때린다.

### W3 — Sources 이식 ✅ 완료 2026-09-02

W0가 Windows 빌드를 멈춘 **두 줄 중 나머지 하나**(`import Network`)가 사라졌다. 다른 하나
(`import CoreGraphics`)는 W2가 없앴다. **이제 포터블 다섯 모듈에 Apple 전용 import가 없다.**

- `LoopbackSocket` — BSD 소켓으로 쓴 loopback 리스너. `#if canImport(Darwin|Glibc|WinSDK)`
  세 갈래뿐이고 그 위의 HTTP 파싱·토큰 검사·크기 상한은 한 줄도 안 바뀌었다.
  `127.0.0.1` 바인드는 기본값이 아니라 요구사항이다 — 이 소켓으로 토큰이 오간다.
- `accept`는 블로킹이고, `stop()`은 **자기 자신에게 한 번 접속해서** 그것을 깨운다.
  폴링하면 한 번 일어날 종료를 잡으려고 CPU를 영원히 깨우게 된다.
- `HookCommand` — 두 installer가 쓰던 `/usr/bin/curl` 하드코딩을 한곳으로 모으고
  경로·따옴표·출력 무음화를 OS별로 갈랐다. Windows는 10 build 1803부터 System32에
  진짜 `curl.exe`가 있어 경로만 다르다.
- `PetCatalog.userPetFolder` — Roamling 자기 폴더만 `%APPDATA%\Roamling\Pets`로 갈린다.
  agent 폴더(`.codex/pets` · `.petdex/pets`)는 agent들이 홈에 두므로 그대로다.
  메뉴의 "펫 폴더 열기"도 같은 값을 읽어서 아무도 안 읽는 디렉터리를 열 수 없다.

**옮기면서 결함 하나를 만들고 잡았다.** 읽지 않은 요청 바이트가 남은 소켓을 닫으면 커널이
RST를 보내 방금 쓴 응답을 버린다 — 크기 초과로 거절당한 hook이 400 대신 **아무 답도 못 받는**
것처럼 보였다. `shutdown(WR)` 뒤 상한을 둔 drain으로 고쳤고, 거절 뒤에도 다음 hook을 계속
받는지까지 테스트가 고정한다. `NWConnection.cancel()`이 공짜로 해주던 일이다.

**검증**: 로직 테스트 134개, 그리고 서명 빌드의 실제 receiver 두 개에 curl —
204 · 401 · 20회 연속 전부 204 · 2 MB 거절 후에도 계속 동작 · 두 포트 모두 `127.0.0.1`에만
바인드.

### W3b — 셸 표면을 데이터로 (macOS, 동작 변화 0) ✅ 완료 2026-09-02

**W4가 두 번째 셸을 만드는 순간 갈라질 것들을 먼저 모은다.** W1이 런타임에 한 것과 같은
일을 UI 표면에 한다 — 구조는 포터블 모듈에 데이터로 두고, 각 플랫폼은 렌더만 한다.
2026-09-02에 `RoamlingMac` 13개 파일을 전수 검토해 넷을 찾았다.

**1. 메뉴 트리** — `RoamlingAppDelegate.swift` 608줄, `NSMenuItem` 34개, 서브메뉴 6개
(펫 · 크기 · Claude · Codex · 손쉬운 사용 · 시각 배치). macOS는 메뉴바 `NSStatusItem`,
Windows는 시계 옆 트레이(`Shell_NotifyIcon`)라 **렌더러는 당연히 다르지만 트리는 같아야
한다.** 지금 구조로 가면 항목 하나 추가할 때마다 양쪽을 고쳐야 하고, 한쪽만 고치면 조용히
어긋난다. 메뉴가 호출하는 런타임 public API 32개는 이미 공유되므로 남은 것은 트리뿐이다.

**2. 튜닝 패널의 범위 — 이미 어긋나 있다.** `RuntimeTuningWindowController`의 슬라이더 11개가
범위와 스텝을 직접 들고 있는데, 같은 범위가 `RuntimeTuning.init`의 clamp에도 있다. 한 진실이
두 벌이고 **`catchArmDistance`가 실제로 갈라졌다**:

```text
Core : catchArmDistance.clamped(to: 40...self.pointerAwarenessDistance)   -> 최대 360
UI   : range: 40...140                                                     -> 최대 140
```

pointer awareness를 360까지 올려도 catch arm은 140을 못 넘는다. **이건 Windows 문제가 아니라
지금 있는 결함이고**, 셋째 사본이 생기기 전에 닫는 것이 맞다. 범위·스텝·단위는 Core가
정본이어야 한다.

**3. 사용자에게 보이는 문자열 91개** — `Sources/RoamlingMac/Resources/{en,ko}.lproj`에 있다.
9절이 Windows에서 `.lproj`가 무수정으로 읽힌다고 확인했지만 **그 번들은 AppKit 모듈 것**이라
Windows 셸이 못 읽는다. `LocalizedText.swift`는 이미 Foundation 20줄이므로 옮길 것은
리소스 위치뿐이다.

**4. 알림 8개** — `NSAlert` 8곳. *무엇을 말할지*는 정책이고 *어떻게 띄울지*가 플랫폼이다.
설치 결과·권한 안내 문구가 셸에 박혀 있으면 3번과 같은 이유로 갈라진다.

**올바르게 플랫폼에 남는 것** (옮기지 않는다): provider 7종, 오버레이 패널,
`MacPlaceholderArt`, `MacPetImageSource`, `MacPlatform.makeServices()`, 그리고 앱 수명주기
(`NSApp.setActivationPolicy(.accessory)`, terminate).

**실제로 생긴 것** (2026-09-02):

- `RoamlingShell` — `ShellMenu.items(for:)`(런타임 상태의 순수 함수), `MenuAction`(닫힌 집합),
  `ShellController.perform`, `ShellPrompt`, 그리고 문자열 91개. **위젯은 하나도 없다.**
- `RoamlingAppDelegate`는 608줄 → **189줄**. 남은 것은 렌더와 모달뿐이다.
- `RuntimeTuningKey` + `RuntimeTuning.limits(for:)`(Core) — 슬라이더 범위의 정본.
  `catchArmDistance`가 실제로 넓어졌다: pointer awareness 360에서 이제 300을 받는다.
- 테스트 5개가 붙었다. 메뉴가 값이 된 덕에 **처음으로 검증된다** — 펫·크기가 정확히 하나만
  체크되는지, 토글이 자기가 뒤집는 상태를 보고하는지, 설정·권한을 건드리는 동작이 전부
  먼저 묻고 되돌릴 수 있는 동작은 묻지 않는지, 번역 안 된 키가 제목으로 새지 않는지.
  전체 133개 통과.
- import 게이트가 `Sources/RoamlingShell`까지 덮는다.

**남은 것**: `RuntimeTuningWindowController`(SwiftUI 257줄)의 *레이아웃*은 아직 macOS 전용이다.
필드 목록·범위·단위는 Core와 Shell이 들고 있으므로 Windows는 렌더러만 쓰면 되지만, 섹션 제목과
순서는 아직 SwiftUI 뷰 안에 있다. W4에서 트레이를 만들 때 같이 뺀다.

**Exit**: W1·W2와 같다 — 메뉴·튜닝 패널·알림이 실사용에서 전과 같고, `catchArmDistance`만
Core의 범위대로 넓어진다(이건 의도된 수정이므로 따로 확인받는다).

**언어 결정과의 관계**: D로 가면 이 데이터 트리가 Rust에 살고 양 플랫폼이 렌더만 한다.
A로 가면 Swift 포터블 모듈에 산다. **어느 쪽이든 필요한 일이라 결정을 기다리지 않는다** —
W1·W2와 같은 성질이다.

### W4 — Windows 최소 루프

tray + layered window + Display/Pointer/Idle provider.
**exit 조건: MVP 0 수준(배회 · 포인터 회피 · 잡기 · 드래그)이 Windows에서 돌고 사용자가
실사용으로 확인한다.**

### W5 — 나머지 provider

Window / Focus / Capture. 5절 참조.

### W6 — 패키징

10절의 실측(17파일 · 56.0 MB · zip 21.3 MB · 단일 파일 불가)이 이 게이트의 입력이다.

**Inno Setup + per-user 설치.** `scripts/build-installer.ps1`이 `scripts/build-app.sh`의
대응물이 된다:

```text
1. swift build -c release -Xlinker /SUBSYSTEM:WINDOWS -Xlinker /ENTRY:mainCRTStartup
2. exe + *.resources + 런타임 DLL 16개를 스테이징
3. iscc roamling.iss  ->  Roamling-Setup.exe (약 21 MB)
```

2번의 DLL 수집은 `dumpbin /dependents`를 재귀 순회해 실제로 쓰이는 것만 모은다. W0에서
그 방법으로 56 MB 폴더를 만들어 **PATH에서 Swift를 지운 채 실행되는 것까지 확인했다.**

**per-user 설치가 핵심 선택이다.** `%LOCALAPPDATA%\Programs\Roamling`에 깔면 **UAC 프롬프트가
아예 뜨지 않고**, 자동시작도 `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`이라 권한이
필요 없다. 데스크톱 펫을 설치하는데 관리자 승인을 묻는 것은 그 자체로 *Never annoying* 위반이다.

포터블 zip도 같이 낸다(공짜로 나온다). winget 매니페스트는 GitHub 릴리스의 exe를 가리키기만
하면 되므로 나중에 붙인다.

**MSI/WiX는 쓰지 않는다** — 기업 배포 수요 없이 복잡도만 는다. **MSIX/Store도 아니다** —
서명·심사·샌드박스 마찰이 이 단계에 맞지 않는다.

#### 반드시 걸릴 함정 — `*.resources`

`CLAUDE.md`가 macOS에 대해 경고하는 그것이 Windows에서 그대로 재발한다. `build-app.sh`가
`$BIN_DIR/*.bundle`을 복사하듯, Windows 설치 스크립트는 **`*.resources` 디렉터리**를
복사해야 한다 — W0의 l10n 탐침에서 `L10nProbe_L10nProbe.resources`로 확인된 형태다.
**빠지면 `Bundle.module`이 런타임에 trap한다.** 91개 UI 문자열과 Mochi 아틀라스가 거기 있다.

#### 서명

서명 없는 설치 파일은 SmartScreen 경고가 뜬다. 초기에는 서명 없이 내고 README에 설명하되,
**Azure Trusted Signing**을 우선 검토한다 — 전통적 OV/EV 인증서보다 싸고 Microsoft가 CA라
평판이 빨리 쌓인다. **가격과 자격 요건은 바뀌므로 착수 시점에 직접 확인할 것.**

macOS의 `scripts/signing.env` 패턴을 그대로 가져온다 — identity를 git-ignore된 파일에 두고,
없으면 서명 없이 빌드해서 기여자 빌드가 깨지지 않게 한다.

### W7 — 자동 업데이트 (양 플랫폼 공통)

**첫 배포부터 넣는다.** 나중에 붙이면 이미 설치된 사용자에게 도달할 방법이 없다.

**Sparkle(macOS) + WinSparkle(Windows).** 둘은 **같은 appcast XML 피드 형식**을 쓴다. 피드
하나와 릴리스 절차 하나에 얇은 플랫폼 어댑터 둘 — 이 저장소의 모듈 경계와 같은 모양이다.
Sparkle은 Swift에서 그대로 쓰이고 WinSparkle은 C DLL이라 Windows 쪽에서 얇은 shim이 필요하다.

**대안은 Velopack이다.** Windows·macOS를 한 도구로 덮고 델타 업데이트를 준다. Swift 바인딩
유무는 착수 전에 확인해야 한다. Sparkle 계열의 강점은 macOS에서 사실상 표준이라는 것이고,
Velopack의 강점은 릴리스 파이프라인이 하나라는 것이다. **둘 다 착수 시점에 현재 상태를
확인하고 정한다** — WinSparkle의 유지보수 상태와 Sparkle 2.x와의 appcast 호환도 같이 본다.

#### 이 게이트의 진짜 비용은 업데이터가 아니라 서명이다

- **macOS**: Sparkle의 EdDSA 서명은 공짜지만, 앱 자체가 Developer ID 서명 + notarize가
  안 되면 Gatekeeper가 막는다. `CLAUDE.md`가 이미 경고하듯 ad-hoc 서명은 빌드마다 다른 앱으로
  보여 Accessibility 권한까지 잃는다. **Developer ID 인증서와 notary 서비스는 Apple Developer
  Program 유료 가입(연 $99)이 있어야 나온다** — 현재 로컬 서명은 무료 개발용 identity다.
  이 게이트는 지출 결정을 포함하므로 착수 전에 사용자에게 확인한다.
- **Windows**: 서명이 없으면 **업데이트할 때마다** SmartScreen 경고를 보게 된다.

**서명 없이 자동 업데이트를 먼저 붙이면 업데이트가 없느니만 못하다.** 사용자가 매번 경고를
클릭하게 된다. 순서는 서명 → 업데이터다.

#### 제품 원칙과 충돌하지 않게

업데이트 알림은 *Never annoying*이 금지하는 종류의 방해다. **백그라운드에서 조용히 받고 다음
실행에 적용한다.** 모달을 띄우지 않고, 재시작을 요구하지 않는다. Sparkle이 이 모드를 지원한다.

GPL-3.0이므로 배포하는 각 버전에 대응하는 소스를 계속 제공해야 한다 — 릴리스마다 태그를
남기면 충족된다.

## 5. 매핑 표에 더할 것

`docs/architecture.md`의 표는 맞다. 다만 몇 군데는 더 싼 길이 있다.

| provider | 표의 경로 | 실제로 먼저 시도할 것 |
|---|---|---|
| Capture | Windows Graphics Capture | **BitBlt + StretchBlt → 작은 DIB** |
| Focus | UI Automation | **`GetGUIThreadInfo`** |
| Window | HWND/Win32 | `DWMWA_EXTENDED_FRAME_BOUNDS` |
| SafeZone | work area candidates | `MONITORINFO.rcWork` |

- **Capture**: Windows Graphics Capture는 WinRT라 Swift에서 아프다. 64컬럼 다운샘플을 수 초
  주기로 뜨는 용도(현재 예산 capture 1회 62ms)에는 BitBlt가 충분하다. 그리고 "펫이 자기
  자리를 바빠 보이게 만들면 안 된다"는 `MacCaptureProvider`의 `excludingApplications` 로직이
  Windows에서는 **`SetWindowDisplayAffinity(WDA_EXCLUDEFROMCAPTURE)` 한 줄**로 끝난다.
  다만 **BitBlt는 하드웨어 오버레이(MPO)로 그려지는 영상과 DRM 보호 창을 검게 읽는다.**
  우리는 어두운 곳을 "비어 있다"로 점수화하므로 펫이 재생 중인 영상 위에 앉을 수 있다 —
  MVP 4가 막으려던 바로 그 실패다. W5 착수 시 유튜브 전체화면과 넷플릭스로 먼저 확인하고,
  검게 나오면 그 창은 캡처가 아니라 window rect로 판정한다.
- **Focus**: UI Automation COM 전에 `GetGUIThreadInfo`를 시도한다. COM 없이 캐럿 rect가
  나오는 앱이 많아 MVP 3 수준을 싸게 얻는다. COM interop이 이 포트에서 가장 아플 구간이므로
  피할 수 있으면 피한다.
- **Window**: `GetWindowRect`는 보이지 않는 테두리를 포함한다. geometry를 맞추려면 DWM의
  extended frame bounds를 써야 한다.
- **DPI**: per-monitor DPI awareness v2가 필수다. Core가 논리 포인트를 쓰고
  `DisplaySnapshot.scale`이 이미 있으므로 `GetDpiForMonitor`로 채운다.

`HWND`와 UIAutomation COM 타입은 adapter를 넘지 않는다 — 기존 규칙 그대로다.

## 6. 권한 모델이 Windows에서 달라진다

위 전부가 **Windows에서는 권한 프롬프트 없이 된다.** Screen Recording 승인도 Accessibility
승인도 없다. `docs/mvp.md` MVP 4의 권한 모델(opt-in 게이팅, 거부 시 MVP 3 경로 복귀)이
Windows에서는 성립하지 않는다 — 캡처가 항상 가능하다.

프라이버시 원칙(디스크 미기록, 로그 미기록, 내용 미해석)은 그대로 지킨다. 그러나 **"OS 권한
승인이 곧 사용자 동의"였던 자리를 Windows에서는 명시적 설정으로 대신 만들어야 한다.**
기본값을 켜 두면 "Never annoying"이 아니라 몰래 보는 쪽이 된다. Windows의 capture는
**opt-in 설정 뒤에 둔다.**

## 7. 리스크

W0가 둘을 없애고 하나를 새로 만들었다.

1. ~~Swift on Windows로 GUI 상주앱을 만든 전례가 드물다~~ — **해소.** layered window의
   7가지 요구가 Swift에서 전부 동작했고, Rust 대조군과 결과가 같았다(9절).
2. ~~`.lproj` 로컬라이제이션이 corelibs-foundation에서 약할 수 있다~~ — **해소.** 무수정 동작.
3. **W2가 렌더링 회귀를 부를 수 있다.** 바이트 비교 게이트로 막는다. *(남아 있음)*
4. **COM interop** — UIA가 필요해지는 지점. 5절대로 `GetGUIThreadInfo`를 먼저 시도해
   피할 수 있는지 본다. *(남아 있음)*
5. ~~다중 디스플레이가 미검증이다~~ — **같은 날 해소.** 두 번째 모니터를 붙여 재실행했고
   1.5배·3.0배 혼합 DPI에서 통과했다(9절). **음수 좌표 배치만 남았다** — 보조 화면을
   primary 왼쪽/위로 옮기면 1분이면 확인된다. W4의 cross-display 경로 전에 볼 것.
6. **툴체인 환경이 macOS보다 무겁다.** vcvars64 + `SDKROOT`이 없으면 `swift build`가
   깨진 툴체인처럼 실패한다. CI와 기여자 문서에 그대로 비용이 된다. *(신규)*

## 8. 순서와 머신 제약

리팩터(W1·W2)는 `swift build`로 검증되지 않는 변경을 만들지 않기 위해 **macOS 머신에서
한다.** 반대로 **W0 스파이크는 Windows 머신의 작업이었고 2026-09-01에 끝났다** —
툴체인·번들·layered window는 Windows에서만 확인된다.

MVP 4가 2026-09-02에 닫히면서 그 충돌이 없어졌다 — `RoamlingRuntime`과 capture를 동시에
건드리는 게이트가 더는 없다. W1은 2026-09-02에 닫혔고 **W2가 현재 게이트다.**

```text
(Windows 머신) W0 스파이크  ✅ 2026-09-01 완료 -> 분기 A
        |
MVP 4 exit rule 충족  ✅ 2026-09-02
        |
        v
   (macOS 머신) W1 ✅ 2026-09-02 -> W2 ✅ 2026-09-02 -> W2b
        |
        v
   W3 ✅ -> W3b ✅ -> W4 <- 다음 -> W5 -> W6 -> W7
        ^
        +-- W2b(디코더)는 여기까지 미룬다. 언어 결정과 같은 결정이다.
             ^                ^
             |                +-- 자동 업데이트. 양 플랫폼 공통이고 macOS도 함께 받는다.
             |                    서명이 선행 조건이다 (W6).
             |
             +-- 진입 전에 보조 화면을 primary 왼쪽/위로 옮겨 음수 좌표만 확인
                 (다중 디스플레이 본체는 2026-09-01에 통과, 9절)
```

**W7만 Windows 전용이 아니다.** 자동 업데이트는 macOS에도 없는 기능이라 이 게이트에서 양쪽이
같이 생긴다. 그래서 피드 형식과 릴리스 절차를 한 번만 정하는 것이 중요하다.

모듈이 실제로 움직이면 `CLAUDE.md`의 모듈 경계 절과 `docs/architecture.md`의 Future
migration 절을 같이 고친다.

## 9. W0 실행 결과 (2026-09-01)

### 환경

Windows 11 build 26200, **단일** 2560×1600 디스플레이, DPI 144(1.5배).
Swift 6.3.3 `x86_64-unknown-windows-msvc`, Rust 1.98.0, MSVC 14.44 + Windows SDK 10.0.26100.
탐침은 `output/w0/`에 있다(git 미추적, 버리는 코드).

### 결과

| 항목 | 결과 |
|---|---|
| W0.0 실제 `Package.swift`로 `RoamlingCore` 빌드 | **PASS — 무수정** |
| W0.1 Core 3,156줄 / 20파일 빌드 | **PASS** |
| W0.2 `CoreLogicTests` 72개 실행 | **PASS — 72/72** |
| W0.3 `Bundle.module` + `.lproj` 7개 체크 | **PASS — 7/7** |
| W0.4a Swift layered window 7종 | **PASS** (컴파일 수정 3곳) |
| W0.4b Rust layered window 7종 | **PASS** |
| 캡처 제외 대조 실험 | **PASS** |

### 예상과 달랐던 것 셋

**1. 로컬라이제이션이 그냥 됐다.** 이 문서가 최대 미지수로 꼽았던 항목이다. corelibs-foundation이
`.strings`를 파싱하고, `ko.lproj`가 해석되고(`menu.pet` → `펫`), 없는 키는 키를 돌려주고,
`String(format:)` 인자도 살아남는다. **`LocalizedText.swift`는 한 줄도 안 고쳐도 된다.**
JSON 로더 대안은 필요 없다.

**2. 블로커는 매니페스트가 아니라 소스의 import 두 줄이다.** `.linkedFramework("AppKit")`
같은 설정이 문제를 일으킬 거라고 4절에 적어뒀는데 **틀렸다** — SwiftPM은 그 타겟을 빌드하지
않는 한 무시하고, `platforms: [.macOS(.v13)]`도 Windows에서 그냥 무시된다. 실제로 멈추는 곳은
정확히 두 줄이다:

```text
Sources/RoamlingSources/Shared/LoopbackHookReceiver.swift:5   import Network        -> W3
Sources/RoamlingPet/MascotPetFactory.swift:4                  import CoreGraphics   -> W2
```

`.when(platforms:)` 조건은 지금 당장은 필요 없다. W2·W3의 범위가 그만큼 좁아진다.

**3. Swift Win32 코드가 Rust보다 짧았다.** 같은 7가지를 하는 데 주석 제외 Swift 166줄,
Rust 261줄(대조 실험 함수를 빼면 약 226줄). 줄 수가 곧 ergonomics는 아니지만, "Swift로
Win32는 지옥"이라는 가설과는 반대 방향의 증거다.

### 캡처 경로가 실측으로 확인됐다

5절이 근거 없이 주장하던 두 가지를 대조 실험으로 검증했다.

```text
WDA_NONE             -> BitBlt가 스프라이트 픽셀 17,129개를 읽음
WDA_EXCLUDEFROMCAPTURE -> 0개
```

BitBlt는 화면을 실제로 읽고(Windows Graphics Capture 없이 충분), `SetWindowDisplayAffinity`는
펫을 캡처에서 진짜로 제외한다. MVP 4의 "펫이 자기 자리를 바빠 보이게 만들면 안 된다"가
Windows에서는 한 줄로 해결된다.

### 툴체인 마찰 — 기록해 둘 것

**Swift on Windows는 자체 링커가 없다. MSVC의 `link.exe`를 부른다.** 평범한 셸에서
`swift build`를 하면 `toolchain is invalid: could not find CLI tool 'link'`로 죽고,
`SDKROOT`이 없으면 `unable to load standard library for target 'x86_64-unknown-windows-msvc'`로
죽는다. 둘 다 툴체인이 깨진 것처럼 보이지만 환경변수 문제다.

macOS에서 `swift build`는 그냥 된다. 이 차이가 CI와 기여자 문서에 그대로 비용이 된다.
`output/w0/vcvars.ps1`이 그 환경을 만든다.

### Swift 마찰의 실제 크기

탐침에 10곳을 `FRICTION`으로 표시했지만, Rust와 비교했을 때 **진짜 Swift 고유 비용은 셋**이다:

- wide string 리터럴이 없다 — Rust는 `w!()`, Swift는 `[UInt16]` 버퍼를 수명까지 관리
- 캐스팅 매크로를 importer가 버린다 — `DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2`를
  `bitPattern: -4`로 손수 재구성
- nullable import — `CreateCompatibleDC`가 Optional로 들어온다

나머지(`@convention(c)` 캡처 불가, `BLENDFUNCTION` 좁히기 변환, 상수 캐스팅)는 **Rust도 똑같이
낸 C interop 세금**이다. 세 항목 모두 작은 shim 하나로 흡수된다 — 한 번 쓰면 나머지 코드는
평범하게 읽힌다. **A′로 대피할 근거가 나오지 않았다.**

### 다중 디스플레이 — 같은 날 두 번째 모니터를 붙여 닫았다

처음 실행한 머신은 모니터가 하나여서 6번이 미검증으로 남았는데, 사용자가 두 번째 모니터를
연결해 다시 돌렸다. **그리고 우연히 더 좋은 시험이 됐다 — DPI가 섞여 있다.**

```text
[0] bounds=(0,0)..(2560,1600)     work=(0,0)..(2560,1528)     dpi=144  scale=1.50
[1] bounds=(2560,0)..(6400,2160)  work=(2560,0)..(6400,2016)  dpi=288  scale=3.00
```

1.5배와 3.0배가 한 데스크톱에 있다. MVP 0을 닫았던 비대칭 3-display 환경과 같은 성격이라
균일한 배치보다 훨씬 나은 조건이다. 탐침은 `stop 0 → (0,0)`, `stop 1 → (2560,0)`으로
**두 모니터를 정확히 번갈아 착지했고**, per-monitor DPI v2 아래에서 두 화면 모두에서 논리
좌표가 맞았다.

**캡처 제외도 멀티모니터에서 확인됐다.** 로그가 `stop 7: (2560,0)`으로 펫이 두 번째
모니터에 있다고 말하는 순간에 가상 화면 전체를 캡처했는데 그 영역에 아무것도 없었다.
`WDA_EXCLUDEFROMCAPTURE`가 화면을 가리지 않고 창 단위로 동작한다.

**아직 안 남은 하위 항목 하나** — 두 번째 모니터가 primary의 **오른쪽**에 있어서
`negative-origin display present: false`다. 보조 화면을 primary의 왼쪽이나 위로 옮기면
음수 좌표가 생기고, `WorldPoint`가 음수 x/y를 허용하는 이유가 그것이다. 디스플레이 설정에서
배치만 바꾸면 1분이면 확인된다. **W4의 cross-display 경로를 만들기 전에 한 번 보는 것이 좋다.**

### 사람이 본 결과 — 그리고 프로브가 잡아낸 결함 둘

| 확인 항목 | 결과 |
|---|---|
| 알파 경계가 부드러운가 (사각 테두리 없음) | **PASS** |
| 포커스를 안 뺏는가 (`WS_EX_NOACTIVATE`) | **PASS** — 타이핑이 안 끊긴다 |
| 클릭 통과 토글 (`WS_EX_TRANSPARENT`) | **PASS** — 양쪽 상태 모두 동작 |
| 항상 위 (`WS_EX_TOPMOST`) | **PASS** |
| 작업표시줄 버튼 없음 (`WS_EX_TOOLWINDOW`) | **PASS** — 10절 링커 플래그로 재빌드 후 확인 |
| 커서가 정상 화살표 (`hCursor` 수정 후) | **PASS** |
| 두 모니터를 오간다 (혼합 DPI) | **PASS** — 아래 참조 |

**로그로는 안 나오고 사람이 봐야 잡히는 결함이 둘 나왔다.** 프로브를 눈으로 확인한 값어치가
여기 있다.

1. **콘솔 창이 작업표시줄 버튼을 가져간다.** 프로브가 콘솔 앱이라 오버레이가
   `WS_EX_TOOLWINDOW`로 버튼을 숨기는지 확인 자체가 불가능했다. 10절의
   `/SUBSYSTEM:WINDOWS` + `/ENTRY:mainCRTStartup`이 이걸 없앤다. **실제 앱도 그렇게 빌드해야
   한다.**
2. **`WNDCLASSW.hCursor`가 nil이면 커서가 "처리중"으로 바뀐다.** click-through가 꺼진 동안
   오버레이 위에서 대기 커서가 뜬다 — 펫이 멈춘 것처럼 보이는 결함이다. Windows 한계가
   아니라 우리 실수이고, `LoadCursorW(nil, IDC_ARROW)`로 끝난다. 다만 `IDC_ARROW`가
   `MAKEINTRESOURCE(32512)`라 Swift importer가 또 버려서 `bitPattern: 32512`로 재구성해야
   했다 — FRICTION 3과 같은 부류가 즉시 재발한 사례다(11절 참조).

### 판정

**A — Swift 단일 코드베이스.** 3절 분기표의 "1·2번 통과 + 4번 통과" 경로다.
W1·W2로 진행하며, 그 둘은 macOS 머신에서 한다.

## 10. 배포 실측 (2026-09-01)

W0에서 같이 쟀다. **Swift on Windows는 단일 파일이 될 수 없다.**

`--static-swift-stdlib`을 클린 빌드로 시험했는데 **Windows에서는 조용히 무시된다** — Linux
전용 플래그다. 플래그를 줘도 결과 exe가 `swiftCore.dll` · `Foundation.dll` · `swiftCRT.dll` ·
`swiftWinSDK.dll`을 그대로 요구한다.

실제 최소 배포 폴더를 만들어 **PATH에서 Swift를 완전히 제거한 뒤 실행시켜** 검증한 값이다.

| | Swift | Rust |
|---|---|---|
| 파일 수 | **17개** | **1개** |
| 폴더 크기 | **56.0 MB** | 0.15 MB |
| zip | **21.3 MB** | — |
| 실행 파일 자체 | 0.09 MB | 0.15 MB |

무게의 정체는 하나다 — **`_FoundationICU.dll`이 35.6 MB로 전체의 64%다.** Foundation을 쓰는
한 딸려오고, Roamling은 `Codable`·JSON·`FileManager`를 전면적으로 쓰므로 피할 수 없다.

**사용자에게는 여전히 파일 하나를 준다.** 폴더를 Inno Setup이나 WiX로 감싸 설치 관리자
`.exe` 하나로 만들면 시작 메뉴·자동시작·제거까지 붙는다. 그것이 W6의 내용이다. 최소로
가면 21.3 MB zip이다. 상주 앱으로 이상한 크기는 아니다 — Electron 앱은 보통 100 MB를 넘는다.

### GUI 앱으로 빌드하려면 링커 플래그가 둘 필요하다

```sh
swift build -c release -Xlinker /SUBSYSTEM:WINDOWS -Xlinker /ENTRY:mainCRTStartup
```

`/SUBSYSTEM:WINDOWS`만 주면 `undefined symbol: WinMain`으로 죽는다. Swift는 `main`을
만들기 때문에 CRT 진입점을 명시해야 한다. 이것을 빼면 콘솔 창이 따라다니고, **콘솔이
작업표시줄 버튼을 가져가서 오버레이가 `WS_EX_TOOLWINDOW`로 버튼을 숨기는지조차 확인할 수
없다.**

## 11. Rust 전면 재작성 재검토 브리프

사용자가 장기적으로 C(Rust 전면 재작성)를 원한다. 이 절은 **그 논의를 위한 자립적 브리프**다.
3절의 결정(A)은 아직 유효하고, 이 절은 그것을 뒤집자는 주장이 아니라 **판단에 필요한 실측치를
한곳에 모은 것**이다.

### W0가 바꾼 것 — 이제 추측이 아니다

**Rust 쪽으로 기우는 실측 근거**

- **배포**: 1파일 0.15 MB vs 17파일 56 MB(zip 21.3 MB). 10절 참조. Swift는 단일 파일이 불가능.
- **툴체인**: Swift는 vcvars64와 `SDKROOT` 없이는 깨진 툴체인처럼 실패한다. Rust는 `cargo build`
  하나로 끝났다. CI와 기여자 문서에 영구 비용.
- **B3 소멸**: `image` 크레이트가 WebP를 디코드한다. libwebp를 번들할 이유가 없어진다.
- **마찰의 패턴이 확인됐다**: Swift의 대표 비용은 **importer가 캐스팅 매크로를 버리는 것**이다.
  `DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2`(FRICTION 3)에서 처음 나왔고, 커서 결함을
  고치다 `IDC_ARROW`(FRICTION 11)에서 똑같이 재발했다. 앞으로 Win32 표면을 넓힐수록 계속 나온다.
- **COM**: W5의 UI Automation이 COM이다. `windows-rs`는 COM을 제대로 덮고 Swift는 수동 vtable이다.
  5절대로 `GetGUIThreadInfo`로 피해지면 이 항목은 사라지고, 안 피해지면 커진다. **아직 미측정.**

**Swift 쪽에 남는 실측 근거**

- **11,481줄이 이미 있고 이미 게이트를 통과했다.** 그리고 그중 Core 3,156줄 + 테스트 1,615줄이
  **Windows에서 무수정으로 빌드·통과한다는 것이 9절에서 증명됐다.**
- **Swift의 Win32 코드가 Rust보다 짧았다** — 같은 7가지에 주석 제외 166줄 vs 261줄. "Swift로
  Win32는 지옥"은 실측으로 반증됐다. 마찰은 실재하지만 좁고 기계적이라 shim 하나로 흡수된다.
- **Rust도 같은 C interop 세금을 냈다** — `@convention(c)` 캡처 불가, `BLENDFUNCTION` 좁히기,
  상수 캐스팅은 양쪽 모두에서 나왔다.

### Rust가 사주지 않는 것 — 직관을 교정할 것

- **배터리는 거의 그대로다.** 비용은 active travel의 60Hz 재그리기와 capture 주기(1회 62ms,
  3~6초 간격)가 지배하고 둘 다 언어와 무관하다. Swift도 ARC 붙은 네이티브 코드지 VM이 아니다.
- **Win32가 쉬워지지 않는다.** `windows-rs`도 결국 raw Win32다. 위 줄 수가 그 증거다.
- **오버레이 요구가 검증된 것은 언어 덕이 아니다.** 7가지가 되는 것은 Windows가 지원하기
  때문이고, Swift·Rust 양쪽에서 같은 결과가 나왔다.

### 아직 아무도 재보지 않은 것 — 이게 C의 진짜 위험이다

**W0는 Windows만 쟀다. C의 위험은 macOS 쪽에 있다.**

C는 잘 돌아가는 macOS 앱을 `objc2`로 다시 만드는 것을 포함한다. AppKit의 `NSPanel` 오버레이,
Spaces/fullscreen 의미론, ScreenCaptureKit, Accessibility 권한 — **그중 무엇도 Rust에서
측정되지 않았다.** Windows 쪽 자신감을 macOS 쪽으로 옮겨 적으면 안 된다.

**C를 진지하게 고려한다면 대칭적인 스파이크가 먼저다** — W0.4와 같은 오버레이 프로브를
macOS에서 `objc2`로 만들어 보는 것. 그게 없으면 C는 측정되지 않은 절반 위에 서 있다.

### 맥 에이전트와 정할 것 넷 — 답 (2026-09-02)

1. **21 MB → 0.15 MB가 11,481줄 재작성 값어치가 있는가.** → **아니오.** 사용자는 설치 관리자
   하나를 받을 뿐이고 디스크 21 MB는 오늘날 체감되지 않는다. 이 항목 단독으로는 재작성 근거가
   되지 않는다.
2. **MVP 사다리가 언제 멈추는가.** → **사실상 지금 멈췄다.** MVP 4가 2026-09-02에 닫혔고,
   MVP 5(저작 UI)는 이름만 있는 항목이며 MVP 6은 없다. 그런데도 **지금 C로 가지 않는다** —
   멈춤이 재작성의 필요조건이었지 충분조건은 아니다. W1·W2를 먼저 해서 실제 경계를 드러낸
   뒤에 결정하는 편이 싸고, 그 둘은 어느 쪽으로 가도 버려지지 않는다.
3. **W5의 UIA가 COM을 요구하는가.** → **아직 미측정. W5 전에 재는 것으로 확정.**
   `GetGUIThreadInfo` 탐침을 VS Code · Windows Terminal · Chrome에서 돌려 캐럿 rect가
   나오는지 본다. **Electron 계열이 위험군**이다 — 네이티브 캐럿을 노출하지 않으면 거기서만
   UIA가 필요해진다. 탐침은 W0.4와 같은 크기의 버리는 코드다.
4. **macOS 쪽 `objc2` 스파이크를 먼저 돌릴 것인가.** → **돌렸다. 2026-09-02, 결과는
   12절.** 처음에는 "W2 뒤로 미룬다"고 답했는데 사용자가 지금 재기로 정했다. C의 미측정
   절반이 더는 미측정이 아니다 — 오버레이 10개 검사가 전부 통과했고, 같은 날 잰 제3길
   (Rust core + 기존 Swift 셸)의 FFI 실측이 이 절의 선택지 표에 다섯 번째 줄을 만들었다.

### 이 브리프의 권고

**지금 C로 전환하지 않는다. 그러나 문을 닫지도 않는다.**

W1·W2를 먼저 한다. 3절에 적었듯 그 둘은 A·A′·B·C **어느 쪽으로 가도 버려지지 않는다** —
Runtime을 macOS 모듈에서 빼내고 `PetAsset`을 데이터 포맷으로 만드는 일은 어떤 언어로
포팅하든 이식 명세가 된다. **C로 갈 경우에도 그 경계가 재작성의 설계도가 된다.**

그러니 W1·W2는 결정을 기다릴 필요가 없고, 그 사이에 위 네 질문의 답이 모인다.

## 12. macOS 스파이크 실행 결과 (2026-09-02)

11절 4번이 "C의 미측정 절반"이라고 부른 자리를 실제로 쟀다. **버리는 코드이고
`output/spikes/`(git 미추적)에 있다.** W0와 같은 규칙이다 — 코드가 아니라 판정을 남긴다.

환경: macOS 26.5(Darwin 25.5), Apple Swift 6.3.3, **Rust 1.98.0**(W0가 Windows에서 쓴 것과
같은 버전), `objc2` 0.6.4 / `objc2-app-kit` 0.3.2, `uniffi` 0.32. 실측 머신은 3-display
(1920×1080@2x, 1728×1117@2x, 1920×1080@1x).

### W0m.1 — objc2 오버레이 프로브 (C의 macOS 절반)

`PetOverlayPanel.swift`가 하는 일곱 가지에 권한 게이트 둘을 더해 Rust/objc2로 다시 만들었다.
W0.4가 Windows에서 세운 것과 같은 구조의 대조 실험이다.

| 검사 | 결과 |
|---|---|
| W0m.1 borderless · non-activating `NSPanel`, floating, clear bg, never key | **PASS** |
| W0m.2 per-pixel alpha `CGImage`, `NSImageInterpolation.none` | **PASS** |
| W0m.3 click-through 토글 (`ignoresMouseEvents`) | **PASS** |
| W0m.4 `canJoinAllSpaces｜fullScreenAuxiliary｜stationary｜ignoresCycle` | **PASS** |
| W0m.5 `sharingType = .none` (캡처 제외) | **PASS** |
| W0m.6 `NSScreen` frames + `backingScaleFactor` (3-display 혼합 DPI) | **PASS** |
| W0m.7 `NSView` 서브클래스, 타원 hit region | **PASS** |
| W0m.7b 합성 클릭 라우팅 (`CGEventPost`) | **PASS** |
| W0m.8 `AXIsProcessTrusted` 호출 가능 | **PASS (약한 검사)** |
| W0m.9 `CGPreflightScreenCaptureAccess` 호출 가능 | **PASS (약한 검사)** |

**per-pixel alpha는 두 층에서 확인했다.** 뷰의 백킹 비트맵을 직접 읽어 불투명 3,625 ·
반투명 1,400 · 투명 4,959 픽셀(테스트 이미지의 원+반투명 링 구조와 정확히 일치), 그리고
화면 합성 후 스크린샷에서 반경 34pt 원이 예상 좌표에 그대로 나왔다.

**캡처 제외 대조 실험** — W0가 Windows에서 한 `WDA_NONE` → `WDA_EXCLUDEFROMCAPTURE`
실험의 macOS 판이다:

```text
sharingType = .readOnly  -> screencapture가 스프라이트 픽셀 14,500개를 읽음
sharingType = .none      -> 0개
```

MVP 4의 "펫이 자기 자리를 바빠 보이게 만들면 안 된다"가 objc2에서도 한 줄이다.

**마찰은 아홉 개, 전부 기계적이었다.** 첫 빌드에서 컴파일 에러 9개가 나왔고 네 번의
수정으로 닫혔다. 성질별로:

- `CGDataProvider::with_data`가 C 시그니처 그대로(info·ptr·len·release 콜백 4인자)라
  `CFData`를 경유해야 했다. `CGImage::width(Some(&img))`도 메서드가 아니라 자유 함수 모양이다.
- **`CGRect`에 점 포함 헬퍼가 없다.** `NSPointInRect`도 바인딩돼 있지 않아 직접 썼다.
- `define_class!`로 내보내는 ObjC 메서드는 `bool`이 아니라 `runtime::Bool`을 돌려줘야 한다.
  실제로는 `containsPet:`을 ObjC 셀렉터에서 빼고 평범한 Rust 메서드로 내리는 게 맞았다 —
  AppKit이 부르지 않는 것을 셀렉터로 내보낼 이유가 없다.
- `alloc()`에 `AnyThread`, `retain()`에 `Message` 트레이트를 각각 import해야 한다.
- 이름이 Swift 프로퍼티가 아니라 ObjC 셀렉터를 따른다 (`canBecomeKeyWindow`).

**objc2 탓이 아닌 마찰이 하나 있었고 이게 제일 오래 걸렸다.** `NSRunLoop::runUntilDate`만
돌리면 창이 합성되지도, 클릭이 배달되지도 않는다. AppKit 이벤트는
`nextEventMatchingMask` + `sendEvent:`가 돌아야 흐르고, 그건 `NSApplication::run`이 하는
일이다. **Swift로 썼어도 똑같이 겪는다** — W0의 "Rust도 같은 C interop 세금을 냈다"와
같은 종류의 발견이다.

**줄 수는 사실상 동률이다.** 오버레이 구현부만 Rust 216줄, `PetOverlayPanel.swift` 202줄
(주석·빈 줄 제외). 다만 정확히 같은 것을 세지는 않았다 — Rust 쪽엔 테스트 이미지 생성기가
들어 있고 Swift 쪽엔 world 좌표를 다루는 `MacOverlayProvider`가 들어 있다. **"objc2가
AppKit보다 몇 배 장황하다"는 직관은 이 워크로드에서 성립하지 않는다.** release 바이너리는
541 KB.

**이 프로브가 재지 않은 것 — 여기가 여전히 공백이다.**

- W0m.8·W0m.9는 **심볼이 붙는다는 것만** 증명한다. 실제 캐럿 rect를 `AXUIElement`로 읽는
  것도, ScreenCaptureKit로 한 프레임을 실제로 뜨는 것도 하지 않았다. MVP 3·4의 본체가
  거기 있으므로 **C를 진지하게 저울에 올릴 때 이 둘은 따로 재야 한다.**
- Spaces 전환·fullscreen 진입 시의 실제 거동, 서명된 번들에서의 TCC 프롬프트, 메뉴바
  아이템, 튜닝 창 — 전부 미측정.
- 프로브는 서명되지 않은 CLI라 TCC 신원을 부모 프로세스에서 물려받는다. 그래서 `AXIsProcessTrusted`가
  `true`를 돌려줬고, 이는 앱의 권한 상태와 무관하다.

### W0m.2 — 제3길 프로브: Rust core + 기존 Swift 셸 (uniffi)

`BasicSafeZonePlanner`와 그것이 쓰는 geometry·world 타입을 Rust로 포팅하고 uniffi로
Swift에 노출한 뒤, **진짜 `RoamlingCore`와 같은 입력을 넣어 결과를 대조했다.**

```text
402개 world (실측 3-display + 단일 + 무작위 400) x
  safe zone 전체 비교 + 펫/포인터를 화면 전체로 쓸어가며 destination 35회
= safe-zone 불일치 0, destination 불일치 0 (14,070 질의)
```

**Swift의 `max(by:)`가 동점에서 마지막 원소를 돌려준다**는 것까지 맞춰야 0이 나왔다.
`min_by`/fold의 tie-breaking을 그대로 옮기지 않으면 조용히 다른 모서리에 앉는다 —
포팅이 "기계적"이라는 말이 "안전하다"는 뜻은 아니라는 증거다.

**성능 — 직관과 반대 방향이었다.** 100,000회 호출, 양쪽 다 release 빌드:

| | µs/call | |
|---|---:|---|
| Swift `RoamlingCore` (release, native) | **1.913** | 지금 |
| Rust native (FFI 없음) | **1.302** | 1.47배 빠름 |
| Swift → Rust, world를 매 호출 전달 | 8.590 | 순진한 설계 |
| Swift → Rust, world를 Rust가 보유 | **4.656** | 현실적 설계 |
| uniffi 크로싱만 (페이로드 없음) | **0.027** | 사실상 공짜 |

**크로싱 자체는 27 ns로 공짜다. 비싼 것은 직렬화다** — 3개 display와 문자열이 든 world를
매 호출 넘기면 3.9 µs가 붙는다. world를 Rust 쪽 객체에 얹고 tick마다 점 세 개만 넘기면
4.656 µs이고, 남은 3.4 µs는 `String` 두 개(`display_id`, `reason`)를 든 반환값의 마샬링이다.

판정은 이렇다. **제3길은 성능 이득이 아니다** — 이 워크로드에서 Rust의 원시 이득은 1.47배뿐이고
FFI를 거치면 Swift로 그냥 두는 것보다 2.4배 느리다. 그러나 **성능 문제도 아니다**: tick당
한 번이면 4.656 µs는 60 Hz 프레임 예산 16,666 µs의 **0.028%**다. 그러므로 제3길의 근거는
속도가 아니라 구조여야 한다 — 로직 1벌, Swift-on-Windows 의존 제거, 실사용으로 조율된
macOS 셸 보존.

**설계 지침 세 줄** (제3길로 갈 경우):

- 상태는 Rust가 들고, tick마다 넘기는 것은 값 몇 개로 줄인다.
- **tick마다 돌려주는 값에 `String`을 넣지 않는다.** `reason` 같은 진단 문자열은 열거형
  인덱스로 넘기고 표시할 때 Swift에서 해석한다.
- 경계는 A′ 절이 이미 설계한 "스냅샷 in → 표시할 프레임 out" 그대로다. **W1이 만든
  `PlatformServices`가 정확히 그 자리다** — Swift가 tick을 몰고, 플랫폼 상태를 모아 넘기고,
  무엇을 그릴지 돌려받는다. 콜백 방향이 없으므로 uniffi callback interface가 필요 없다.

**비용.** Rust 포팅본 300줄(벤치 헬퍼 포함) — Swift 원본은 플래너 143줄 + 그것이 기대는
geometry·world 282줄이다. uniffi가 만드는 Swift 바인딩 989줄은 **생성물이라 유지보수 대상이
아니다.** 손으로 쓴 브리지는 10줄. cdylib 551 KB, 바인딩 dylib 253 KB, cold `cargo build
--release` 99초.

**타입 이름이 충돌한다.** uniffi가 만드는 `WorldPoint`·`WorldRect`·`DisplaySnapshot`이
`RoamlingCore`의 같은 이름과 부딪혀 프로브에서는 모듈로 한정해야 했다. 진짜 제3길에서는
Rust 쪽이 **유일한** 정의가 되므로 이 충돌은 사라진다 — 다만 그 말은 곧 **전환이 부분적일 수
없다는 뜻**이다. 두 벌을 나란히 두면 매 참조를 한정해야 한다.

### W0m.3 — 디코더 스파이크: Rust가 B3를 없애는가 (2026-09-02)

W2가 디코딩을 `PetImageSourcing` 뒤로 밀어 둔 덕에 **런타임 1,700줄을 옮기지 않고** D의
핵심 질문 셋을 잴 수 있었다. 바꾼 Swift는 `RustPetImageSource` 한 구조체뿐이고 나머지
앱은 자기가 무엇으로 디코드되는지 모른다.

**1. 바이트 동일한가 → 그렇다.** `image` 0.25.10으로 디코드한 결과가 ImageIO와
**248프레임 전부 일치**한다(내장 mochi 96 · fat-mochi 56 · 패키지 mochi-v3 96;
placeholder 88은 플랫폼 드로잉이라 제외). W2의 게이트를 그대로 돌린 것이다.

한 가지는 맞춰야 했다 — **`image`는 straight alpha, ImageIO는 premultiplied**를 준다.
Rust 쪽에서 `(c * a + 127) / 255`로 반올림 곱을 해야 일치한다. 이걸 Swift에서 하면 경계를
건너는 것이 `PetImage`가 약속한 것과 달라지므로 Rust 안에서 한다.

**그리고 C가 없다.** `image-webp` · `png` · `zlib-rs` 전부 순수 Rust다. 4절 W2b가 A 경로에
적어둔 "libwebp + miniz 약 4만 줄 벤더링"이 **D에서는 `Cargo.toml` 한 줄**이 된다.

**2. 11.5 MB를 건네는 비용 → 감당된다.** 아틀라스는 1.15 MB로 실려 11.5 MB로 풀린다.

| | ms/decode |
|---|---:|
| ImageIO (지금) | **19.87** |
| Rust native (FFI 없음) | 24.09 |
| Rust via uniffi | **39.37** |

FFI 세금은 **11.5 MB당 15.3 ms**(약 750 MB/s)다. 어제 잰 "크로싱은 27 ns로 공짜, 비싼 것은
직렬화"와 같은 결론이고 이번엔 최악의 페이로드에서 확인했다. **디코드는 실행당 두 번
(표준 시트 + 확장 시트) 일어나고 그 뒤로는 없다.** 시작이 40 ms 늘어난다는 뜻이라 무의미하다.
Rust 자체가 ImageIO보다 21% 느린 것도 같은 이유로 무의미하다.

**3. 두 언어 빌드와 서명 → 통과한다.** 이게 12절이 남긴 질문 (2)번이었고 제일 모르던
부분이다. dylib을 `Contents/Frameworks/`에 넣고, identity로 서명하고,
`codesign --verify --deep --strict`가 통과하고, **번들 안의 서명된 실행 파일이 rpath로
Rust dylib을 찾아 실제로 돌았다.**

함정은 하나였고 반드시 밟는다 — **Rust cdylib의 기본 `install_name`이 빌드 머신의 절대
경로다**(`/Users/.../target/release/deps/lib....dylib`). 그대로 배포하면 다른 머신에서
로드에 실패한다. `install_name_tool -id @rpath/...` 또는
`-Clink-arg=-install_name,@rpath/...`로 고치고, 실행 파일에
`-rpath @executable_path/../Frameworks`를 준다. 그 밖의 의존은 `libiconv`와 `libSystem`
둘뿐이라 추가로 실을 것이 없다.

비용: dylib 0.88 MB + uniffi Swift 바인딩 dylib 0.17 MB = **번들 +1.08 MB**(8.4 MB → 11 MB,
디버그 심볼 포함). cold `cargo build --release` 99초.

**판정: D의 디코더 논거는 실측으로 섰다.** WebP가 Windows에서 공짜가 아니라는 사실(B3)이
A에서는 4만 줄 벤더링이고 D에서는 의존성 한 줄이다. 그리고 그것을 확인하는 데 제품 코드를
한 줄도 옮기지 않았다 — W1의 `PlatformServices`와 W2의 `PetImageSourcing`이 만든 이음새
덕이다.

**아직 재지 않은 것**: `AXUIElement` 캐럿과 ScreenCaptureKit 한 프레임을 objc2로 실제로
뜨는 것(12절 W0m.1의 공백 그대로), 그리고 `RoamlingEngine` 1,700줄을 옮길 때 W0m.2에서 본
tie-breaking 함정을 테스트가 전부 잡아 주는지. **D는 macOS 셸을 그대로 두므로 첫 번째는
D에 필요 없다** — C에만 남는 공백이다.

### 선택지 표에 다섯 번째 줄

| | 로직 | macOS 셸 | Windows 셸 | Swift on Windows 필요? | macOS 재검증 | Windows 배포 | WebP(B3) |
|---|---|---|---|---|---|---|---|
| **A** (현재) | Swift | Swift/AppKit 그대로 | Swift/`WinSDK` 신규 | 예 | 없음 | 17파일 56 MB | libwebp 벤더링 |
| **A′** | Swift DLL | Swift/AppKit 그대로 | C# | 예 | 없음 | + .NET 런타임 | libwebp 벤더링 |
| **B** | C# 2벌 | Swift/AppKit 그대로 | C# | 아니오 | 없음 | .NET | .NET 기본 제공 |
| **C** | Rust 신규 | **Rust/objc2 신규** | Rust/`windows-rs` | 아니오 | **전부** | **1파일** | `image` 한 줄 |
| **D. 제3길** | Rust 신규 | **Swift/AppKit 그대로** | Rust/`windows-rs` | **아니오** | **없음** | **1파일** | `image` 한 줄 |

**단일 파일은 Windows에서만 의미가 있다.** macOS는 언어와 무관하게 `.app` 번들이어야 한다 —
`LSUIElement`(Dock 없는 상주앱), TCC가 권한을 붙이는 `CFBundleIdentifier`,
`NSScreenCaptureUsageDescription`, 서명·notarize가 전부 번들 전제다. **그리고 그 Windows
이득은 C만의 것이 아니다** — D의 Windows 셸도 Rust라 Swift가 없다. 11절 1번이
"21 MB → 0.15 MB가 11,481줄 재작성 값어치가 있나 → 아니오"라고 답했을 때, 그 이득을 얻으려고
macOS 셸까지 버릴 필요는 없다는 점을 놓쳤다.

D는 C에서 **측정되지 않은 절반을 뺀 것**이다. 로직은 결정적이고 테스트 126개가 덮고 있어
포팅이 검증 가능하지만, 셸은 사용자가 3-display 앞에 앉아 닫은 값들이 사는 곳이라 재검증이
비싸다. D는 비싼 쪽을 건드리지 않는다. **그리고 Swift on Windows 의존이 사라진다** — 툴체인이
공식이긴 해도 가장 큰 기여자였던 The Browser Company가 Atlassian에 인수되고 Windows용 Arc가
멈춘 지금, 이건 값이 있는 성질이다.

D의 대가는 macOS 빌드에 두 언어·두 빌드 시스템이 들어오는 것, 그리고 위의 "전환이 부분적일
수 없다"는 제약이다.

### 바인딩 생태계 — 공식이 어디까지인가 (2026-09-02 조사)

| | 성격 | stars | 최근 push |
|---|---|---:|---|
| `microsoft/windows-rs` | **Microsoft 공식**, winmd에서 생성 | 12,719 | 2026-09-02 |
| `madsmtm/objc2` | 비공식. **Xcode SDK 헤더에서 생성**, 단일 메인테이너 | 1,023 | 2026-08-27 |
| `thebrowsercompany/swift-winrt` | 비공식(Browser Company). 최신 릴리스 2026-03 | 851 | 2026-04-10 |
| `servo/core-foundation-rs` (`cocoa`) | 손으로 쓴 바인딩. 생태계가 objc2로 이탈 중 | 1,281 | 2026-05-08 |
| `ryanmcgrath/cacao` | 고수준 래퍼. 18개월 정체 | 2,077 | 2025-02-03 |
| `mozilla/uniffi-rs` | Mozilla. 활발 | 4,925 | 2026-08-31 |
| `chinedufn/swift-bridge` | 개인. uniffi 대안 | 1,129 | 2026-01-06 |

**Apple 공식 Rust 바인딩은 없다. objc2가 사실상 유일한 실전 후보다.** 공식은 아니지만
공식에 가장 가까운 이유는 셋이다 — Xcode SDK 헤더에서 생성하므로 커버리지가 사람 손에
달려 있지 않고(`objc2-app-kit`·`objc2-screen-capture-kit`·`objc2-application-services`가
전부 있다), winit이 이미 옮겼고 tauri 계열과 wgpu가 이전 중이며, 위 표의 나머지는 전부 더
작거나 더 오래됐다. 약점은 **단일 메인테이너**라는 것 하나다.

**Windows 쪽은 걱정할 축이 아니다.** swift-winrt가 비공식이고 같은 조직의 `swift-winui`·
`swift-windowsappsdk`가 2025-10에 archive됐지만, **우리 경로에 WinRT가 없다** — W0는 툴체인
내장 `WinSDK`(Win32)로 통과했고 5절이 캡처를 BitBlt, focus를 `GetGUIThreadInfo`로 잡아 둔
것이 결과적으로 WinRT 의존을 피했다. 남는 신호는 Swift-on-Windows 툴체인 자체의 추진력이고,
그건 막힌 것이 아니라 지켜볼 것이다. D는 그 신호에 걸린 베팅을 아예 없앤다.

### 이 절이 바꾸는 것과 바꾸지 않는 것

**바꾸지 않는 것: 지금 할 일은 여전히 W2다.** D로 가더라도 `PetAsset`을 `CGImage`에서
떼어내는 일은 그대로 필요하고(오히려 Rust `image` 크레이트가 B3를 없앤다), W1이 만든
`PlatformServices` 경계가 D의 경계와 같은 자리라는 것이 이 절의 발견이다.

**바꾸는 것: C의 미측정 절반이 줄었고, D라는 선택지가 실측 위에 올라왔다.** 결정은 W2가
끝난 뒤에 한다 — 그때 남는 질문은 셋이다. (1) `AXUIElement` 캐럿과 ScreenCaptureKit을
objc2로 실제로 뜰 수 있는가 (2) 두 언어 빌드를 `scripts/build-app.sh`와 서명 흐름에 얹는
비용 (3) `RoamlingEngine` 1,700줄을 옮길 때 W0m.2에서 본 tie-breaking 함정을 테스트가
전부 잡아 주는가.
