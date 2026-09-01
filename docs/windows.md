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
빌드된다. 예외는 `ImageIO`를 쓰는 `PetLogicTests.swift` 하나이고, 이는 아래 W2가 없앤다.

## 2. 블로커는 세 개다

### B1. 앱의 두뇌가 macOS 모듈에 있다

`Sources/RoamlingMac/RoamlingRuntime.swift`는 1,732줄인데 **플랫폼 타입을 쓰는 줄은 7개**다:

- `NSObject` 상속
- `PetOverlayViewDelegate` 4개 메서드의 `NSPoint`
- `NSApplication.didChangeScreenParametersNotification` 관찰
- `corePoint(fromAppKitScreenPoint:)`

나머지 전부가 플랫폼 비의존 오케스트레이션이다. 이대로 두면 Windows는 1,700줄을 복제하거나
포크된다. **Windows를 하지 않더라도 고칠 값어치가 있는 항목**이고, 이 포트에서 가장 큰
단일 작업이다.

### B2. `RoamlingPet`의 공개 API가 `CGImage`다

`PetAsset.atlas`가 `CGImage`이고(`Sources/RoamlingPet/PetAsset.swift`), 아틀라스 합성과
프레임 크롭이 `CGContext`/`CGImageSource` 위에 있다. 모듈 경계상 공용이어야 하는 계층이
macOS 전용이다.

### B3. WebP 디코더가 Windows에 없다

내장 Mochi 아틀라스가 `.webp`이고, 더 중요하게 **Petdex/Codex 펫 패키지의
`spritesheet.webp`는 항상 webp다.** 내장 에셋만 PNG로 바꿔도 `~/.codex/pets` 로딩이 안 된다.
macOS에서는 ImageIO가 공짜로 해주던 일이라 결정이 필요한 줄 몰랐던 항목이다.

### 그 밖의 작은 것들

- `LoopbackHookReceiver`의 `NWListener` — Apple `Network` 프레임워크
- 두 hook installer의 하드코딩된 `/usr/bin/curl` (Windows 10 1803+ 는 System32에 `curl.exe`)
- `PetCatalog`의 `Library/Application Support/Roamling/Pets` → `%APPDATA%\Roamling\Pets`
- `scripts/test.sh`가 zsh, `scripts/build-app.sh`가 codesign 전제
- `Localizable.strings`를 `Bundle.module`로 읽는 경로 — corelibs-foundation에서 미확인

## 3. 결정

### Swift on Windows로 간다

**Options:** Swift on Windows 단일 저장소, Rust/C# 쉘 + Swift core FFI, Core 재작성.

**Chosen:** Swift on Windows. `docs/architecture.md`의 "Swift core now, extraction later"는
미확인 Windows 요구 때문에 FFI를 미뤘는데, 요구가 확인된 지금도 결론은 같다. Core가 이미
순수 Foundation이고 **테스트 자산이 언어 독립이 아니라 Swift로 돼 있다.** FFI 경계를 그으면
3,156줄 Core는 넘어가지만 B1의 1,732줄과 테스트는 경계 밖에 남는다 — C ABI가 사주는 것이
없다. Rust 쉘은 Swift-on-Windows가 실제로 막혔다는 증거가 나온 뒤의 카드로 남긴다.

### 리팩터가 먼저, Windows 코드는 나중

**Options:** Windows adapter를 먼저 세우고 맞춰 리팩터, 리팩터를 먼저 끝내고 adapter 작성.

**Chosen:** 리팩터 선행. W1·W2는 Windows 코드가 0줄이고 전부 macOS에서 검증된다. 이걸 먼저
끝내면 Windows 작업이 "adapter 채우기"로 줄어든다. 순서를 뒤집으면 검증되지 않은 두 플랫폼
위에서 동시에 리팩터하게 된다.

## 4. 게이트

`docs/mvp.md`와 같은 규칙이다 — 한 게이트의 exit 조건을 닫기 전에 다음으로 넘어가지 않는다.

### W0 — 스파이크 (버리는 코드, **Windows 머신에서 한다**)

하루 안에 답을 봐야 하는 네 가지:

1. Windows 툴체인에서 `RoamlingCore`가 빌드되는가 (`Package.swift`의 `.linkedFramework`에
   `.when(platforms:)` 조건이 필요할 것이다)
2. `RoamlingLogicTests`가 Windows에서 통과하는가
3. **`Bundle.module`이 리소스를 찾고 `.lproj`/`Localizable.strings`가 읽히는가**
4. `WinSDK` import로 layered window에 per-pixel alpha가 찍히는가

3번과 4번이 진짜 미지수다. `Sources/RoamlingMac/LocalizedText.swift`가 20줄이라 `.strings`가
안 되면 JSON 로더로 갈아끼우면 되지만, **모르는 상태로 W4를 계획하지 않는다.**

### W1 — Runtime 추출 (macOS, 동작 변화 0)

`RoamlingRuntime`을 공용 모듈로 옮긴다. `NSPoint` → `WorldPoint`, 화면 변경 알림 →
`PlatformServices.swift`의 새 `DisplayChangeObserving` 프로토콜. 회귀는 기존 테스트로 잡는다.

### W2 — 이미지 파이프라인 탈-CoreGraphics (macOS)

`PetAsset.atlas`를 RGBA8 버퍼 + 크기를 든 값 타입으로 바꾸고, 아틀라스 합성과 프레임 크롭을
순수 Swift 블리터로 내린다. 디코더는 **libwebp + PNG 디코더를 SwiftPM C 타겟으로 번들**한다
(BSD/zlib 라이선스는 GPL-3.0-only와 호환).

양 플랫폼이 같은 디코더와 같은 블리터를 쓰면 픽셀이 동일해지고, `scripts/pet_qa.py`가 지키는
baseline·중심·detached component 불변식을 로직 하네스 안에서 양쪽 다 검증할 수 있게 된다.

**착수 전에 픽셀아트 보간을 고정할 것** — `PetOverlayPanel`의 `NSImageInterpolation` 설정을
블리터가 그대로 재현해야 한다. W2 전후로 렌더된 프레임을 바이트 비교하는 게이트를 건다.
아트 불변식이 엄격한 프로젝트라 픽셀 차이가 곧 결함이다.

### W3 — Sources 이식

`NWListener` → 이식 가능한 loopback HTTP listener. curl 경로 OS 분기. `PetCatalog` 검색 경로에
`%APPDATA%\Roamling\Pets` 추가.

### W4 — Windows 최소 루프

tray + layered window + Display/Pointer/Idle provider.
**exit 조건: MVP 0 수준(배회 · 포인터 회피 · 잡기 · 드래그)이 Windows에서 돌고 사용자가
실사용으로 확인한다.**

### W5 — 나머지 provider

Window / Focus / Capture. 5절 참조.

### W6 — 패키징

`build-app.sh` 대응물, Authenticode 서명(없으면 SmartScreen 경고), 자동시작 등록.

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

1. **Swift on Windows로 GUI 상주앱을 만든 전례가 드물다.** Core·Runtime·Sources는 확신이
   있지만 W4가 미지수다. W0가 이걸 겨냥한다.
2. **`.lproj` 로컬라이제이션이 corelibs-foundation에서 약할 수 있다.** 확인 필요, 대안은 저렴.
3. **W2가 렌더링 회귀를 부를 수 있다.** 바이트 비교 게이트로 막는다.
4. **COM interop** — UIA가 필요해지는 지점.

## 8. 순서와 머신 제약

리팩터(W1·W2)는 `swift build`로 검증되지 않는 변경을 만들지 않기 위해 **macOS 머신에서
한다.** 반대로 **W0 스파이크는 Windows 머신의 작업이다** — 툴체인·번들·layered window는
Windows에서만 확인된다.

MVP 4가 현재 게이트이고 `RoamlingRuntime`과 capture를 지금 건드리는 중이다. W1을 게이트
중간에 넣으면 충돌한다.

```text
MVP 4 exit rule 충족
        |
        +---- (Windows 머신) W0 스파이크 -- 지금 해도 된다. 독립적이고 정보 이득이 크다
        |
        v
   (macOS 머신) W1 -> W2
        |
        v
   W3 -> W4 -> W5 -> W6
```

모듈이 실제로 움직이면 `CLAUDE.md`의 모듈 경계 절과 `docs/architecture.md`의 Future
migration 절을 같이 고친다.
