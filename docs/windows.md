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

**Chosen: A.** 이유는 "Swift가 Windows에서 좋아서"가 아니다 — **포팅 대상의 88%가 UI가
아니기 때문이다.** 1절 표대로 진짜 macOS 전용 코드는 1,540줄뿐이고 나머지 11,481줄
(로직 7,932 + 테스트 3,549)은 이미 쓰여 있고 이미 게이트를 통과했다. 포트의 어려운 부분은
작지만 하필 Swift가 제일 약한 영역이고, 큰 부분은 Swift가 제일 강한 영역이다.

재구현 안(B·C)의 숨은 비용은 코드가 아니라 **검증**이다. MVP 0~0.7의 값들 — walk 40pt/s,
pause 12초, catch radius 74pt, notice 170pt — 은 유닛테스트가 아니라 사용자가 3-display
앞에 앉아 닫은 값이다. 재구현하면 그 판정을 전부 다시 받아야 하고, "귀엽다"는 회귀
테스트로 잡히지 않는다.

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
지금은 MVP 4 진행 중이고 5·6이 남았다 — 아직 발견 중인 동작 스펙을 재작성하면 움직이는
과녁을 두 번 구현하고 게이트를 양쪽에서 다시 받게 된다. 사다리가 멈춘 뒤에 그 자체의
근거로 결정한다.

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
  보여 Accessibility 권한까지 잃는다.
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

MVP 4가 현재 게이트이고 `RoamlingRuntime`과 capture를 지금 건드리는 중이다. W1을 게이트
중간에 넣으면 충돌한다.

```text
(Windows 머신) W0 스파이크  ✅ 2026-09-01 완료 -> 분기 A
        |
MVP 4 exit rule 충족
        |
        v
   (macOS 머신) W1 -> W2
        |
        v
   W3 -> W4 -> W5 -> W6 -> W7
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

### 맥 에이전트와 정할 것 넷

1. **21 MB → 0.15 MB가 11,481줄 재작성 값어치가 있는가.** 사용자 체감으로는 설치 관리자
   하나를 받는 것이라 차이가 드러나지 않는다.
2. **MVP 사다리가 언제 멈추는가.** 지금은 4가 열려 있고 5·6이 남았다. 움직이는 과녁을
   재작성하면 발견 중인 동작 스펙을 두 번 구현하고 게이트를 양쪽에서 다시 받는다.
   **사다리가 멈춘 뒤라면 C는 합리적이다. 지금은 아니다** — 이것이 3절 결론이 여전히 서는 이유.
3. **W5의 UIA가 COM을 요구하는가.** `GetGUIThreadInfo`로 피해지면 Rust 이점이 하나 줄어든다.
   W5 착수 전에 싸게 답할 수 있는 질문이다.
4. **macOS 쪽 `objc2` 스파이크를 먼저 돌릴 것인가.** 위 문단대로 C의 미측정 절반이다.

### 이 브리프의 권고

**지금 C로 전환하지 않는다. 그러나 문을 닫지도 않는다.**

W1·W2를 먼저 한다. 3절에 적었듯 그 둘은 A·A′·B·C **어느 쪽으로 가도 버려지지 않는다** —
Runtime을 macOS 모듈에서 빼내고 `PetAsset`을 데이터 포맷으로 만드는 일은 어떤 언어로
포팅하든 이식 명세가 된다. **C로 갈 경우에도 그 경계가 재작성의 설계도가 된다.**

그러니 W1·W2는 결정을 기다릴 필요가 없고, 그 사이에 위 네 질문의 답이 모인다.
