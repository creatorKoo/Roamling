# Roamling agent guide

macOS desktop companion runtime. Swift 6 / AppKit / SwiftPM, GPL-3.0-only.
제품 원칙은 하나다 — **Cute first. Useful second. Never annoying.** 반응 빈도나
움직임을 늘리는 변경은 이 원칙을 먼저 통과해야 한다.

이 파일은 `AGENTS.md`로도 심볼릭 링크돼 있어서 Claude Code와 Codex가 같은 규칙을 읽는다.
규칙이 갈라지지 않도록 수정은 항상 `CLAUDE.md`에서 한다.

**여기는 규칙과 함정이고, 근거는 `docs/`에 있다. 어떤 질문에 어떤 문서인지는
[`docs/README.md`](docs/README.md)가 지도다.** 닫힌 게이트의 기록은 `docs/history/`로
내려가 있으니, 결정을 뒤집으려는 것이 아니면 거기부터 읽지 않는다.

## 작업 방식 — 문서부터, 부분 고치기 금지 (사용자 결정 2026-09-11)

동작이 여러 층(source 상태 기계 · director · attention · 펫 런타임)에 걸친 기능을 고치거나 늘릴 때는
**먼저 그 흐름을 문서로 정리하고, 사용자가 보고 방향을 정한 뒤에** 코드를 바꾼다. 증상 하나를 막는
부분 수정을 쌓지 않는다.

- 문서의 문장마다 코드 위치(파일:줄 또는 함수)를 단다. 기억이나 계획서의 문장을 사실로 옮기지 않는다.
- 부분 수정이 다른 층의 규칙과 부딪치면 그 자리에서 우회 코드를 더하지 않는다. 멈추고, 충돌을 문서에
  적고, 사용자에게 올린다.
- **사용자가 원한다고 말한 것은 코드보다 먼저 `docs/requests.md`에 적는다.** 계획서에 라벨로만
  참조하지 않는다 — `output/`은 미추적이라 다음 세션이 못 본다. 아내분의 색 팔레트 요청이 그렇게
  사라졌고, 사용자가 다시 묻기 전까지 아무도 몰랐다.
- **기능을 다시 지으면 그 기능을 설명하는 문서를 같은 작업 안에서 고친다.** 코드만 초록이면 아무도
  못 잡는다 — `scripts/test.sh`는 문서를 안 본다. 일하는 앱을 상태형으로 옮긴 날, 흐름의 정본인
  `docs/behavior-flow.md` §5b가 통째로 거짓이 돼 있었고 우연히 읽다가 잡았다.
- 계기: G 항목(일하는 앱)을 고치는 동안 source에 우회 코드가 다섯 개 붙었고, 계획이 코드와 어긋나
  구현이 네 번 멈췄고, 문서에 먼저 쓴 문장이 여러 번 틀렸다. 테스트는 초록이었지만 동작을 한 번에
  설명할 수 없는 상태가 됐다.

## Build, test, run

```sh
swift build                    # 약 5초
./scripts/test.sh              # RoamlingLogicTests + cargo test, 실패 시 non-zero
swift run Roamling
./scripts/build-app.sh release # build/Roamling.app (서명 identity 필수, 아래 참조)
```

Windows에는 Swift가 없다 — 그쪽 빌드는 전부 Rust다.

```powershell
.\scripts\test.ps1        # core + pet + 셸, 실패 시 non-zero
.\scripts\run.ps1         # 끄고 -> 빌드 -> 다시 켜기. release, 트레이만
.\scripts\run.ps1 -Debug  # 같은 것의 debug 빌드. 콘솔에 상태 로그가 찍힌다
```

릴리스는 `v*` 태그를 밀면 `.github/workflows/release.yml`이 만든다. **태그와
`rust/Cargo.toml`의 버전이 다르면 워크플로가 실패한다** — 다르면 설치된 빌드가 영원히
자기를 업데이트하기 때문이다. 자동 업데이트의 서명 키·피드 구조·수동으로 해야 할 일은
`docs/windows.md`의 "자동 업데이트" 절에 있다. **`PUBLIC_KEY_HEX`가 전부 0이면 업데이트는 꺼진 상태이고,
그게 안전한 기본값이다** — 서명을 확인할 수 없는 빌드는 업데이트하지 않는다. 지금은 실제
키가 들어 있고 양 플랫폼의 자동 업데이트가 켜져 있다.

**빌드한 뒤에는 앱을 다시 켠다.** 상주 펫이라 내려둔 채로 두지 않고, 실무적으로도
**실행 중인 사본이 자기 `roamling.exe`를 잡고 있어** `cargo build`가 "액세스가 거부되었습니다"로
실패한다. `run.ps1`이 그 셋을 묶어 둔 이유다.

**Rust 툴체인이 필수다** (rustup, `~/.cargo/bin`이 PATH에). `rust/roamling-core`가 Swift
Core를 한 단위씩 넘겨받는 중이고, **앱이 이미 그걸 링크한다** — `scripts/test.sh`와
`build-app.sh`가 `scripts/build-rust-core.sh`를 먼저 부르므로 cargo 없이는 빌드가 안 된다.

`build-rust-core.sh`는 uniffi 바인딩을 `Sources/RoamlingCoreRs`와 `Sources/CRoamlingCoreFFI`에
생성한다. **둘 다 빌드 산출물이라 git 미추적이고 손으로 고치지 않는다.** 정적 링크라
번들에 dylib이 없고 rpath·install_name·재서명이 필요 없다.

`build-app.sh`는 시작할 때 git-ignore된 `scripts/signing.env`가 있으면 source한다.
거기에 `ROAMLING_CODESIGN_IDENTITY`를 넣어 두면 매번 환경변수를 지정하지 않아도 된다.
설정 방법은 저장소에 있는 `scripts/signing.env.example`에 적혀 있다. identity 이름은
머신의 keychain에 종속되므로 script에 하드코딩하지 않는다 — 기여자의 빌드가 그 이름을
찾지 못해 실패한다.

identity 없이 빌드하면 ad-hoc이 되고 designated requirement가 cdhash로 고정된다. 빌드할
때마다 macOS가 다른 앱으로 보기 때문에 Accessibility 권한이 사라진다. AX 관련 작업은
반드시 identity로 서명한 빌드에서 확인한다.

XCTest 대신 dependency-free executable harness를 쓴다. Command Line Tools의
compiler/SDK mismatch가 나면 두 script 모두 `ROAMLING_SWIFT_SDK=/path/to/MacOSX.sdk`
로 우회한다. 새 Swift 파일에는 기존 파일과 같은 2줄 SPDX 헤더를 넣는다.

## 마크는 하나다 — 🐾

메뉴바 status item의 title, Windows 트레이 아이콘, macOS 앱 아이콘이 **같은 글리프**를 쓴다.
셋 다 **그려서** 쓴다 — 비트맵을 넣어 두면 요청된 크기와 어긋난다.

- macOS 메뉴바: `RoamlingAppDelegate`가 title에 `"🐾"`
- Windows 트레이: `tray.rs`의 `paw_icon()`이 GDI로 그린다
- macOS 앱 아이콘: `scripts/build-icon.sh` → `assets/Roamling.icns` (커밋됨)
- Windows 앱 아이콘: `scripts/build-ico.py` → `assets/Roamling.ico` (커밋됨).
  `.icns`에서 **크기별 이미지를 그대로 복사**한다 — 닮은 것을 두 번 그리는 것이 아니라
  같은 픽셀이다. `build.rs`가 `rc.exe`로 exe에 박는다

앱 아이콘만 파일이어야 해서 커밋한다. 마크를 바꾸면 `build-icon.sh`를 다시 돌린다.
**글리프의 실제 잉크를 재서 맞춘다** — 폰트 크기를 고르면 side bearing을 추측하게 되고,
추측하면 여백만 넓고 발바닥은 작아진다. 그리고 **크기마다 따로 그린다**: 1024를 줄이면
16px에서 발가락이 뭉갠다. 작은 크기일수록 여백을 줄이고 마크를 키운다.

## 사용자에게 보이는 문자열

menu, alert, tuning panel copy는 전부 `Sources/RoamlingShell/Resources/{en,ko}.lproj/
Localizable.strings`에 있고 `localized(_:)` / `localizedFormat(_:_:)`로 읽는다. **메뉴 트리와
알림 문구도 같은 모듈(`ShellMenu` · `ShellPrompt`)에 있다** — AppKit은 렌더만 한다. 새 UI
문자열을 넣을 때는 **두 파일에 같은 key를 넣는다.** en이 base라 ko를 빠뜨리면 조용히
영어로 나온다. 언어 분기 코드는 쓰지 않는다 — 한국어는 ko.lproj, 나머지는 bundle이
알아서 en으로 떨어진다.

제품명(`Roamling`, `Claude Code`, `Codex`)은 번역하지 않는다.

`RoamlingShell`과 `RoamlingPet`이 resource bundle을 가지므로 `scripts/build-app.sh`가 `$BIN_DIR/*.bundle`을
`Contents/Resources`로 전부 복사한다. **복사만으로는 부족하다** — 두 모듈은 `Bundle.module`이
아니라 `petResourceBundle` · `shellResourceBundle`을 쓴다. 새 리소스 접근도 그 둘을 쓴다.

`Bundle.module`은 SwiftPM이 만들어 주는데 두 군데만 본다: **`Bundle.main.bundleURL` 바로 밑**과
**그 바이너리를 컴파일한 기계의 절대 `.build` 경로**. 손으로 조립한 `.app`에는 둘 다 틀리다 —
`bundleURL`은 `.app` 자신이고, 앱 번들 루트의 리소스는 `codesign`이 봉인을 거부한다
(`unsealed contents present in the bundle root`, 직접 확인). 그래서 리소스는
`Contents/Resources`에 가는데 **거기가 그 접근자가 유일하게 안 보는 곳이다.**

**만든 기계에서는 두 번째 후보가 늘 존재해서 아무 문제가 없다.** 자기가 빌드하지 않은
바이너리를 돌린 첫 기계가 첫 macOS 릴리스(v0.3.0)를 설치한 사용자였고, 앱은 실행 즉시
trap했다. 그래서 릴리스와 리허설 양쪽이 **`.build`를 치운 채로 패키징된 앱을 실제로 켜 본다**
(`ROAMLING_SMOKE_TEST=1`). 이 결함은 그 조건에서만 보인다.

## 모듈 경계

```text
RoamlingCore/     OS 비의존. geometry, world, behavior, attention, reaction
rust/roamling-core/  결정 로직의 정본. 단위 1~7 완료 — Core 전체 + tick 본체 +
                  애니메이션 해석. 시트 디코딩(W2b, `image` 크레이트)도 여기 있다 —
                  결정은 아니지만 양 셸이 같은 바이트를 받아야 하는 것이라서다.
                  Swift 쪽 원본은 대조군으로만 남아 있다
RoamlingCoreRs/   생성된 uniffi 바인딩. Engine이 RustCore.swift로 감싸 쓴다
RoamlingPet/      Petdex manifest, atlas runtime, built-in mascot, fallback
                  이미지는 PetImage(RGBA8)다. 디코딩은 공유 Rust 디코더가 하고
                  PetImageSourcing 뒤에는 placeholder 그리기만 남았다
RoamlingSources/  ClaudeCode / Codex activity adapter + BSD 소켓 loopback transport
RoamlingEngine/   RoamlingRuntime — tick loop, placement, activity orchestration
                  RuntimeTuning도 여기 산다 (규칙은 Rust에 있고 Core는 seam을 못 부른다)
RoamlingShell/    메뉴 트리·알림 문구·Localizable.strings. 위젯은 없다
RoamlingMac/      AppKit display, pointer, overlay, 메뉴 렌더러, app delegate
RoamlingApp/      entry point
rust/roamling-pet/   내장 마스코트 · 펫 패키지. (시트 디코딩은 core로 내려갔다 —
                  pet이 core를 의존하므로 셸이 부르려면 그쪽이어야 했다)
                  makeStandardMochi와 PetCatalog/PetManifest/PetLoader의 이식이고,
                  옛 authored 시트와 fallback은 아직 Swift에 있다
rust/roamling-agent/ RoamlingSources의 이식. 훅 payload 정규화 · 인증 loopback
                  수신기 · 훅 설치 제거. 제품별 payload는 이 크레이트를 안 나간다
rust/roamling-update/ 버전 비교 · appcast 파싱 · Ed25519 검증. 양 플랫폼 공유이고,
                  바이트 가져오기와 파일 교체만 셸이 한다
rust/roamling-win/   Windows 셸. 코어를 rlib으로 직접 링크한다 — 이쪽은 FFI가 없다.
                  Win32 타입은 platform.rs를 넘지 않는다
```

**`roamling-win`은 workspace `default-members`에 없다.** 맨 `cargo test`가 macOS에서
`windows` 크레이트를 빌드하려다 깨지기 때문이다. Windows에서는 `cargo build -p roamling-win`.

**Core·Pet·Sources·Engine·Shell 다섯은 window system도 Apple 이미지 프레임워크도
import하지 않는다.** macOS SDK에 다 있어서 컴파일러는 이걸 못 잡는다 — `scripts/test.sh`가 grep으로
막고, 걸리면 non-zero로 끝난다. 런타임이 플랫폼에 닿는 통로는 `PlatformServices` 하나이고, macOS 쪽 조립은
`MacPlatform.makeServices()` 한 함수에 모여 있다.

의존 방향은 항상 바깥 → Core다. Core에 AppKit이나 agent-specific 타입을 넣지 않는다.
자세한 근거는 `docs/architecture.md`, MVP 0~4의 acceptance criteria와 실제로 실린 것은
`docs/history/mvp.md`에 있다. **배터리를 이유로 무언가를 바꾸기 전에 `docs/battery.md`를 읽는다** —
무엇이 실제로 비싼지의 실측과, 이미 되어 있어서 다시 할 필요가 없는 것들이 적혀 있다.
(요약: capture 1회 62 ms가 나머지 전부의 만 배다. 산술은 손댈 것이 없다.) **MVP 사다리는 4에서 멈췄고(2026-09-02 완료), W1 Runtime 추출도 같은 날
닫혔다. W2(이미지 파이프라인 탈-CoreGraphics)도 2026-09-02에, **W2b(이식 가능한 디코더)는
2026-09-07에 닫혔다.** 이제 포터블 다섯 모듈과 **테스트 하네스** 모두 Apple 이미지·윈도우
프레임워크 import가 0이고, `scripts/test.sh`의 grep이 둘 다 본다.** exit rule이 있으므로 사용자의 실사용 확인
전에 다음 게이트로 넘어가지 않는다. 리팩터 게이트 중에는 **동작·타이밍·기본값을 고치지
않는다** — W2의 exit에는 렌더 프레임 336개의 바이트 비교가 포함됐고, 그 픽스처는
`Tests/RoamlingLogicTests/PreW2FrameHashes.swift`다.

**언어 결정은 2026-09-02에 D(Rust core + Swift macOS 셸)로 닫혔고, 포팅은 2026-09-03에
단위 1~7이 끝났다.** 근거·순서·되돌아올 조건은 `docs/history/windows.md` 3절에 있다.

**펫이 무엇을 할지 결정하는 코드는 이제 전부 `rust/roamling-core`에 있다.** geometry ·
world · topology · emptiness · 배치 · attention · 반응 · 튜닝 · 활동 지휘 · tick 본체 ·
애니메이션 해석까지. macOS 앱이 그것을 쓰고 있고 `RoamlingRuntime`은 1,664줄에서 700줄
아래로 줄었다 — 타이머 · UserDefaults · 진단 파일 · agent 구독 · 스프라이트 시트, 즉 결정이
아닌 것들뿐이다.
**Windows 게이트는 W7까지 전부 닫혔다 (2026-09-04).** W4 최소 루프 · W5 provider 셋 ·
W5b agent 연동 · W6 패키징(Inno Setup, per-user) · W7 자동 업데이트(공유 Rust 업데이터 +
Ed25519 서명)까지 실물로 돈다. 첫 릴리스 `v0.2.0`이 나가 있고 자동 업데이트가 켜져 있다.
각 게이트의 실측과 결정은 `docs/history/windows.md`에 있다.

## 맥에서 해야 하는 일

**A~C는 2026-09-04에 끝났다.** Windows 실사용에서 나온 목록이었고, 전부 공유 코어와
공유 문자열의 문제였다.

| | 무엇 | 결과 |
|---|---|---|
| A1 | 착지 동작이 아무에게도 안 보였다 | `pet_runtime`이 `Dropped` 동안 포인터를 안 먹인다. 놓은 직후의 커서는 신호가 아니라 그 조작의 부산물이다 |
| A2 | 걷기 사이 멈춤 12 → 40, 경계 2~78 | 천장이 40이라 같이 넓혔다 |
| A3 | 피하는 거리가 인식 거리를 따라간다 | `awareness * 100/170`·`* 50/170`. 기본 170에서 정확히 100·50이라 기본 동작은 그대로 |
| A4 | 피하기에 이력이 생겼다 | 반경의 1.35배를 벗어나고 0.4초가 지나야 멈춘다. A3만으로는 넓어진 경계에서 똑같이 twitch한다 |
| B1 | 정보 창에 버전 | `Support/Info.plist`에서 읽어 셸에 넘긴다 |
| B2 | 실패 제목이 항상 Claude Code였다 | agent마다 자기 키를 든다 |
| C1 | 슬라이더가 기본값에서 시작한다 | 트랙을 기본값에서 가른다. 범위가 기본값 기준 대칭이 아니었다 |
| C2 | 움직인 슬라이더가 원래 값을 보여준다 | `40초 (기본 12초)` |

**녹화 세션도 같이 고쳤다.** 드래그 중에 가짜 커서가 따라가지 않아 놓는 순간 커서가
347포인트 밖에 있었다 — AppKit에서 불가능한 상황이고, 하필 착지가 방해받지 않는 유일한
거리였다. 그래서 A1의 버그를 트레이스가 잡지 못하고 있었다.

### E. 저장된 설정이 옛 기본값을 얼린다 — 2026-09-04 양 플랫폼 완료

A2가 Windows에 닿지 않아서 드러났다. **저장된 값이 기본값을 이기는데, 패널을 한 번이라도
열면 열한 값이 전부 저장된다** — 그래서 걷기 사이 멈춤을 40으로 올려도 파일에 12가 박힌
기계에는 영영 오지 않았다. "기본값으로"를 누른 것이 그 파일을 만든 원인이라, 가장 성실한
사용자가 가장 확실하게 갇혔다.

양쪽 다 **기본값과 같은 값은 저장하지 않고 지운다.** 안 건드린 항목은 기본값을 계속
따라가고, 하나를 옮겨도 나머지 열을 붙잡지 않고, "기본값으로"는 키를 하나도 남기지 않는다.
Windows는 키별 항목이라 지우면 되고(`remember_tuning`), macOS는 `roamling.runtimeTuning`의
JSON에서 **기본값과 같은 필드를 빼고 쓴다** — 전부 기본값이면 키 자체를 지운다.

**저장 형식은 안 바뀌었다.** 부분 객체는 옛 전체 객체와 같은 JSON이고, 디코더가 이미 없는
키를 기본값으로 채운다(필드 추가를 견디려고 그렇게 돼 있었다). 그래서 옛 파일이 여기서 읽히고
새 파일이 옛 빌드에서도 읽힌다. 마이그레이션은 없다.

**이미 12가 박힌 파일은 12를 유지한다.** 그 12가 옛 기본값이었는지 사용자가 고른 값인지
파일만 봐서는 구분되지 않는다. 슬라이더를 하나라도 움직이면 열한 개가 전부 다시 평가되면서
파일이 스스로 정리되고, "기본값으로"를 누르면 즉시 사라진다.

### D. 자동 업데이트 macOS 절반 — 2026-09-04 완료

`rust/roamling-update`의 결정 로직을 uniffi로 가져다 쓰고, 이 기계에 닿는 셋만 Swift가
한다: URLSession으로 받고, `ditto`로 풀고, 번들을 바꾼다. **macOS는 실행 중인 앱을 바꿀 수
있다** — 프로세스가 경로가 아니라 inode를 들고 있어서, 번들을 치우고 새것을 놓아도 계속
돈다. Windows가 이름을 바꿔 다음 실행에 넘기는 것과 여기서 갈린다.

쓰기 전에 두 번 증명한다: Ed25519 서명이 "우리 바이트"를, `codesign --verify --deep
--strict`가 "macOS가 실행에 동의함"을 말한다.

**서명은 자체 서명 인증서로 간다 (Developer ID 아님).** 대가는 첫 다운로드가 막히는 것 —
사용자가 시스템 설정 → 개인정보 보호 및 보안에서 "그래도 열기"를 한 번 눌러야 한다.
얻는 것이 더 크다: designated requirement가 cdhash가 아니라 **인증서**에 고정되므로
업데이트해도 macOS가 같은 앱으로 보고 **권한이 유지된다.** ad-hoc이면 업데이트마다
접근성·화면기록이 조용히 날아간다.

**`.p12`는 키체인 접근이 내보낸 그대로 쓴다 — 다시 감싸지 않는다.** macos-14에서 재보니
OpenSSL이 쓸 수 있는 형식 중 **`-legacy`만 `security import`가 받는다**(기본값도, AES +
SHA-1 MAC도 거부된다). 그 legacy가 곧 키체인이 쓰는 형식이다. RC2-40이 약하다는 지적은
맞지만 **답은 긴 암호지 다른 컨테이너가 아니다** — 바꿨다가 v0.3.0의 macOS 잡이
`MAC verification failed`로 멈췄고, 그때 `openssl`은 같은 암호로 파일을 잘 열었다.
`security`만 못 읽은 것이다.

`.github/workflows/check-macos.yml`이 **릴리스 없이** 이것을 시험한다
(`gh workflow run check-macos.yml`). 인증서만이 아니라 **macOS 잡 전부**다 — 툴체인 · 인증서 ·
테스트 · 빌드 · 패키징 · dmg 왕복 · 실제 실행까지, 발행 직전에서 멈춘다. 릴리스가 쓰는 것과
같은 composite action(`.github/actions/signing-identity` · `swift-toolchain`)을 쓰므로 둘이
다르게 판정할 수 없다.

**인증서는 서명을 해 봐서 판정한다.** `security find-identity -v`를 보면 안 된다 — 그건
*신뢰*를 거르고, 자체 서명 인증서는 새 러너에서 신뢰되지 않아 멀쩡한 인증서를 두고
`0 valid identities found`라고 한다. codesign은 서명에 신뢰가 필요 없고 검증에만 필요하다.

`.p12`와 암호는 GitHub Secret(`MACOS_CERT_P12` · `MACOS_CERT_PASSWORD`)에 있고
`.github/workflows/release.yml`의 macOS 잡이 임시 키체인에 넣어 서명한다. **`build-app.sh`는
이제 identity 없이는 빌드를 거부한다**(`ROAMLING_ALLOW_ADHOC=1`로만 우회).

배포물은 둘이다 — 사람이 받는 `.dmg`(Applications 심볼릭 링크로 드래그드롭), 업데이터가
받는 `.zip`.

### dmg 창은 커밋된 두 파일이다

`assets/dmg-background.png`와 `assets/dmg/DS_Store`. `scripts/build-dmg-background.sh`가
둘을 만들고(uv 필요), `build-dmg.sh`는 복사만 한다. 아이콘을 커밋하는 것과 같은 이유다 —
릴리스가 그리지도 스크립트하지도 않는다.

**`.DS_Store`를 Finder로 만들지 않는다.** 볼륨을 열어 손으로 정렬시키는 것이 보통인데 그건
데스크탑 세션과 자동화 권한이 필요하고, 릴리스 러너에도 이 환경에도 없다 — `-1712`
(AppleEvent 시간 초과)로 실패하고 껍데기만 남긴다. dmgbuild가 하는 대로 직접 쓴다.

배치를 고칠 때 필요한 것 넷. 전부 실측이고, 모르면 원인을 엉뚱한 데서 찾게 된다:

- **`WindowBounds`는 내용이 아니라 창 전체다.** Finder가 제목표시줄 27포인트를 먼저 떼고,
  경로 막대를 켠 사람에게서 32를 더 뗀다. 400을 달라고 하면 341이 남는다.
- **경로 막대·상태 막대는 Finder 전역 설정이라 dmg가 끌 수 없다.** `bwsp`에 `ShowPathbar:
  False`를 적어도 켜 둔 사람에게는 나온다. 그래서 **그림의 아래쪽은 여백으로 비운다** —
  잘려도 되는 것만 잘리게. 지금 배치는 창 프레임 428, 내용은 위 340 안에 있다.
- **배경은 왼쪽 위 기준으로 1픽셀 = 1포인트, 확대·축소가 없다.** 그림 크기가 곧 배치다.
- **hidpi 2페이지 TIFF는 쓰지 않는다.** `tiffutil -cathidpicheck`이 규격대로 만들어도
  (640×400 @72dpi + 1280×800 @144dpi) Finder가 짝으로 읽지 않는다. 단일 해상도 PNG를 쓴다.

**증상을 눈대중으로 재지 않는다.** 여기서 원인을 세 번 잘못 짚었다. 좌표와 격자를 그린
배경으로 dmg를 만들어 한 번 열어 보면 배율·기준점·실제 내용 높이가 한눈에 나온다.

`assets/dmg/render-dmg-background.swift`와 `assets/dmg/write-ds-store.py`는 **같은 좌표를
따로 들고 있다.** 한쪽만 고치면 화살표가 빈 곳을 가리킨다.

볼륨 이름에 버전을 넣지 않는다. 배경은 별칭으로 참조되고 별칭은 만들어진 경로를 기억하므로,
릴리스마다 바뀌는 이름은 아무도 가진 적 없는 볼륨을 가리키게 된다.

**Intel Mac은 지원하지 않기로 했다 (2026-09-04).** arm64만 빌드한다. 덮으려면 universal
빌드가 필요한데 — Rust 두 타깃 · Swift 두 슬라이스 · `lipo` — Apple이 2023년에 판매를
끝낸 기계를 위한 값이다. Intel Mac은 피드에 항목이 없어 **"최신"이라는 답을 받고**, 실행
못 할 것을 받지는 않는다. 되돌리려면 `build-rust-core.sh`와 `build-app.sh` 양쪽에 타깃을
추가하고 워크플로의 appcast 줄에 `macos-x86_64`를 더한다.

Apple Developer Program 연 $99는 **여전히 안 냈다.** 내면 첫 다운로드 마찰이 사라진다.

포팅 규칙은 그대로다: **살아 있는 구현은 항상 1벌.** Swift 원본은 대조군으로 Core에
남아 있고(`MovementController` · `PlacementDirector` · `AttentionModel` 등), 단위마다
게이트가 둘이다 — `cargo test`의 differential fixture 10개(12 MB, 6만+ 케이스)와 Swift
쪽에서 두 구현을 나란히 돌려 비교하는 테스트. 런타임처럼 위에 호출자가 없어 대조군을
만들 수 없는 경우에는 **실물을 40초 녹화해서**(`Tests/RoamlingLogicTests/RuntimeTrace.txt`)
바이트 단위로 같은 답을 요구한다. 재생성은
`ROAMLING_WRITE_TRACE=<path> swift run RoamlingLogicTests`이고, **통과시키려고 다시 만들지
않는다.**

**그 `cargo test --release`를 2026-09-03에 Windows에서 돌렸고, 10개 중 5개가 깨졌다.**
포팅 결함이 아니라 `hypot`/`atan2`가 플랫폼 libm으로 새는 것이 원인이다 — macOS는 정확
반올림을 하고 MSVC UCRT는 하지 않는다. 어긋남은 전부 1 ULP이고 결정은 바뀌지 않는다.
**처방과 실측치는 `docs/history/windows.md` W4의 "실행 결과"·"처방" 두 절에 있다.**

**그 처방은 2026-09-03에 시행됐다.** `hypot`은 `(dx*dx + dy*dy).sqrt()`로 바뀌었고
(Swift 2곳 · Rust 2곳 + fixture 재생성, 한 커밋), `atan2`는 `look_direction_degrees` 한
필드만 1 ULP 면제됐다. 바뀐 fixture 5개가 Windows에서 깨진 5개와 정확히 일치했고,
**녹화된 세션은 한 바이트도 바뀌지 않았다** — 40초 동안 어떤 비교도 뒤집히지 않았다.
Windows에서 `cargo test --release`가 이제 초록이어야 한다. **아니라면 그것은 새로운
발견이므로 fixture를 다시 만들지 말고 원인을 찾는다.**

**이 경계가 Windows port의 전제다.** `docs/history/windows.md`에 모듈별 실측 이식 비용, 언어
선택 네 가지의 비교, 그리고 2026-09-01에 Windows에서 실행한 W0 스파이크 결과가 있다.
`RoamlingCore`는 실제 `Package.swift`로 Windows에서 무수정 빌드되고 Core 테스트가 통과한다. **포팅·언어 선택·Rust 재작성 논의를 시작하기 전에 그 문서를 읽는다** — 특히
11절이 Rust 전환 판단에 필요한 실측치를 모아 둔 브리프다.

### F. 커서가 앉은 자리로 걷지 않는다 — 2026-09-09 완료

활동 중 좌석이 커서 옆이면 펫이 가다 서다를 반복했다. 응시 대역(인식 거리 170)이 걸음을
멈추고, 커서가 조금 움직이면 `travel_to_seat`가 같은 좌석으로 다시 출발하기 때문이다.
배치 판정에 포인터 항이 없었고 후보 점수의 포인터 감점(최대 27.5)은 좌석을 못 뒤집었다.

이제 **커서에서 `pointer_clearance`(회피가 켜져 있으면 인식 거리, 꺼져 있으면 0) 안의 좌석
후보는 후보가 아니다.** 걷는 중에 목적지가 그 안에 들어오면 review beat에 `SeatUnderPointer`로
다른 좌석을 고르고, 없으면 선 자리가 깨끗할 때 거기서 `settle`, 캐럿·글자 위면 커서 반대쪽
clearance×1.1 지점으로 비켜선다(`step_aside`, 정규 좌석 선택에는 안 섞는다). 그 fallback은
`CoveringCaret`·`CoveringWork`에도 적용된다 — 좌석을 빼면서 생긴 "갈 곳이 없어 캐럿 위에
앉는" 구멍을 막는다. **앉아 있는 펫 옆을 지나는 커서는 대상이 아니다.** clearance 0이면
옛 동작과 같다. `PetSituation`에 필드가 하나 늘어 placement·interest 픽스처를 재생성했고
(생성기는 `output/w-unit5/gen-director.swift` · `output/w-unit2/gen4.swift`, 재현 확인 후),
녹화된 세션은 한 바이트도 안 바뀌었다. 사다리는 `docs/placement.md` §3.2.

**같은 날 두 번째 결함.** 활동 중 펫을 글자 위에 놓으면 `Travel(CoveringCaret|CoveringWork)`로
떠나는데, 그 길에 커서를 두면 응시 대역에서 멈춰 글자 위에 그대로 앉았다. §3.2.2의 "응시는
글자 탈출 걸음을 못 막는다"가 배회의 `Escape`에만 구현돼 있었기 때문이다. 이제 그 정의는
`PlacementIntent::outranks_glance` 하나다 — `Escape`와, `PlacementTravelReason::
keeps_walking_past_glance`가 참인 `Travel`(`CoveringCaret` · `CoveringWork` · `SeatUnderPointer`.
셋째는 커서 때문에 시작된 걸음이 커서를 보느라 멈추면 자기모순이라서다). `decide`의 응시
게이트와 런타임의 `walk_outranks_glance`가 둘 다 그것을 본다. `NewActivity` · `PlannedBlind` ·
`FollowedFocus`는 여전히 응시에 양보한다 — 자리가 나쁜 게 아니라 더 나은 자리로 가는
걸음이라서다. 이 조합은 랜덤 픽스처에 없어서 `gen-director.swift`에 스크립트 시나리오로
박았다 — Windows의 `cargo test`가 이 규칙을 보는 유일한 자리다.

### G. agent 없이도 "일하는 중"을 안다 — 2026-09-12 상태형으로 재구성, 실사용 확인 대기 · Windows 배선 대기 (W8)

첫 비개발자 사용자(초등교사, Windows, 한글·한쇼·한셀)의 피드백에서 나왔다. 펫이 Claude
옆에서 일하는 걸 보고 **"왜 나는 이런 행동 안 해?"** — 훅을 걸 agent가 없는 사람에게는
제품의 Useful 절반이 통째로 없었다. 이제 **지정한 앱이 앞에 있으면** 같은 흐름이 돈다.

```
일하는 앱이 앞에 옴           → 옆으로 와서 그냥 앉아 있음(idle)       [Beside]
그 세션의 첫 타이핑           → 점프, 이어서 running                  [Active + SittingStarted]
세션 안의 다음 타이핑         → 즉시 running                          [Active]
마지막 키 뒤 10초             → 갸웃                                 [Paused]
갸웃 5초                      → 자리를 비우고 원래대로 돌아다님         [Away]
돌아다니다 다시 타이핑         → 걸어와서 running, 점프 없음            [Active]
타이핑 없이 보기만 함          → 옆에 계속 앉아 있음                    [Beside 유지]
앱을 3초 이상 떠남            → 끝. 3분 이상 쳤으면 먼저 손 흔듦        [Away (+ SittingEnded)]
펫 자신이 앞에 옴(메뉴·설정)  → 아무 일 없음. 셸이 nil을 보내고 nil은 "모름"이다
```

**처음 만든 흐름은 앱에 오기만 해도 갸웃했고, 사용자가 이상하다고 했다.** 갸웃(`waiting`
행)은 "기다리는 중"이라는 뜻이라 타이핑이 멈췄을 때가 맞다. 첫 판의 기록은
`output/plans/focus-activity.md`, 지금 판은 `focus-activity-v2.md`다.

**2026-09-11에 사건형으로 붙였다가, 2026-09-12에 상태형으로 다시 지었다.** 동작은 위 표
그대로이고 바뀐 것은 모양이다. 사건형 파이프라인(attention 점수 · dwell · 쿨다운 · 도착 반응
1회)에 "지금 이렇다"를 실어 나르려니 source에 우회 코드가 열 개 붙었고, 계획이 코드와
어긋나 구현이 네 번 멈췄다. 그 진단이 `docs/history/focus-activity-flow.md`(2단계 이전 구조의 기록),
상위 설계가 `docs/state-sources.md`, 지금 도는 흐름과 시간표가 `docs/behavior-flow.md` §5b다.
**이 항목을 고치기 전에 그 셋을 읽는다.**

**source는 사건이 아니라 상태를 선언한다.** `rust/roamling-core/src/focus_activity.rs`의 상태
기계가 전부이고 결정적(`now` 인자)이다. 셸은 0.5초마다 셋(앞에 있는 앱 id, 그 앱이 일하는
앱인지, 마지막 키 입력 뒤 초)을 넘기고 `Vec<StateDeclaration>`을 받아, **창 위치 하나만** 얹어
`declare_state`를 부른다. **펫의 상태를 되먹이지 않는다** — 예전의 네 입력
(`dispatched_event` · `arrival_pending` · `pet_resting` · `agent_on_duty`)은 전부 director 안의
사실이라 셸을 거칠 이유가 없었다.

낱말은 `rust/roamling-core/src/source_state.rs`에 있다. 등급 넷(`Away` · `Beside` · `Active` ·
`Paused`)과 이정표 둘(`SittingStarted` · `SittingEnded`)이고, **(종류 × 등급) → 그림**은 그
파일의 `reaction_for`(전이 때 한 번)와 `sustained_reaction`(유지하는 동안)이 정한다.
점프가 running에 덮이지 않는 것을 타이머가 아니라 이 구분이 보장한다.

**갱신이 2초(`STATE_EXPIRY`) 끊기면 `Away`로 만료한다.** 셸이 말을 멈춘 것을 감지하는 유일한
장치다. 그래서 **0.5초 샘플을 거르면 안 되고**, 앞의 앱을 모를 때(nil) source가 직전 선언을
이정표만 떼고 다시 내는 것도 이 때문이다.

**세션**은 앱 앞에 있는 한 이어지고, 2분(`BREAK`) 미만 자리 비움도 같은 세션이다. 2분 넘게
쉬었거나 다른 일하는 앱으로 가면 새 세션이라 첫 타이핑 점프가 다시 걸린다. **앱을 바꿀 때
누른 Cmd-Tab도 키 입력이다** — 그 앱이 앞에 온 뒤에 눌린 키만 타이핑으로 세고
(`last_counted_key_at`), 갸웃까지의 10초도 그 값으로 잰다.

**`roamling.workApps`는 첫 리스트형 설정이다** — 쉼표 구분 문자열, 비면 키를 지운다(튜닝과
같은 규칙). macOS는 번들 id, Windows는 exe 이름. 메뉴 "일하는 앱"에 **최근 앞에 있었던
앱**이 체크박스로 뜬다(`recent_apps()`, 크롬도 체크 안 된 채로 떠서 켤 수 있다). 기본값은
macOS 빈 목록이다. Windows 기본값은 W8을 구현할 때 그 기계에서 실제 exe 이름을 확인해
정한다(한컴오피스 추정: `Hwp.exe` · `Hshow.exe` · `Hcell.exe`).

**agent와 같이 있을 때는 agent가 우선이다 (사용자 결정 2026-09-11).** agent가 자리를 지키는 동안 —
agent 신호가 30초 안에 왔거나, agent가 자리를 쥐고 5분 침묵 만료 전(`agent_on_duty`) — 일하는 앱의
상태 전이는 펫에게 가지 않는다. 막혀 있어도 **선언은 계속 갱신되므로** agent가 자리를 놓는 순간
다음 샘플에 이어받는다. 보관된 전이는 둘로 갈린다: **인사(`SittingStarted`)는 보관**했다가 자리가
나면 쓰고, **작별(`SittingEnded`)은 버린다** — 남겨 두면 agent가 몇 초 뒤 끝났을 때 그 축하 직후에
늦게 튀어나온다(실제로 그랬다). `Away` 전이는 보관된 인사를 덮는다.

**처음엔 이 규칙이 없었다.** 점수만으로 겨뤄서, 방금 Claude에게 메시지를 보낸 뒤 첫 타이핑의 점프(≈61)가
Claude의 턴 시작(≈62)에 이력 여유 12를 못 넘어 밀렸고, 20초 상한 뒤에야 점프 없이 running이 나갔다.
반대로 갸웃은 "사용자 답을 기다림" 종류(기본 100, 긴급)라 일하는 Claude 자리를 뺏었다 — 타이핑은 지고
멈추면 이기는 상태였다. 점수는 kind 기본 + intensity×10 + 신선도(최대 5) + 창 확신도×3이다
(`attention.rs` `score`). **상태형은 이제 attention을 아예 거치지 않는다.**

**손 흔들기: 이번 세션에서 3분 이상 쳤으면, 앱을 떠날 때 펫이 있는 자리에서 흔든다 (사용자 결정
2026-09-11).** 이미 자리를 비운 상태(갸웃 뒤 배회)여도 흔든다. 보통은 타이핑을 멈추고 한참 뒤에 앱을
닫기 때문에, 한때 "펫이 그 앱 옆에 있을 때만"으로 두었더니 손 흔들기가 거의 나오지 않았다. 누적은
`Active`인 동안에만 쌓이고(`typed_seconds`), 떠나는 순간 0이 된다.

**쉬는 펫에게는 director가 전이를 보관한다.** source는 펫이 쉬는지 알 필요가 없다. 예외는
`SittingEnded` 하나로, 대기 중이면 director가 `CancelRest`를 내서 먼저 깨운다 — 3분 일한 끝의
인사는 늦게라도 보여야 한다.

**`CompanionEventKind::Present`는 이제 아무도 만들지 않는다.** 사건형이던 때 늘렸던 kind인데,
kind가 FFI를 인덱스로 건너서 differential 픽스처 10개가 위치로 이름을 부르므로 지우지 않았다
(`activity.rs`에 사유 주석).

**고치다 찾은 옛 결함.** 걸어서 도착한 틱에 `hold_seat`가 도착 반응을 입힌 뒤 `did_arrive`
블록이 **한 번 더** 전달했고, 두 번째는 줄 것이 없어 `observe`로 떨어져 같은 틱에 점프를
지웠다. **어떤 source든 걸어와서 인사·축하하는 모습이 보인 적이 없었다** — 9월 3일 커밋
`1dbc68c`가 도착을 이벤트로 만들었고 이번이 그것을 한 번만 울리게 한 것이다. 녹화 세션과
differential 픽스처는 그 경로를 **안 지나서** 바이트가 같다 — "안 바뀜"이 아니라 "안 거침".

**하네스의 가짜 키보드를 고쳤다 (2026-09-12).** `FakeUserIdleProvider`는 "마지막 키가 N초 전"을
샘플마다 같은 값으로 답해서, 시계가 가면 마지막 키도 같이 앞으로 밀렸다 — 멈춘 키보드가 늙지
않는다. `the first keystroke hops, ten quiet seconds ask...`는 그 어긋남 위에서 **우연히**
통과하고 있었다(인사 박자 0.84초가 구간을 밀어 `key=9`인 샘플 하나가 마침 "앱이 앞에 온 뒤"로
떨어졌고, 그것이 마지막 키로 다시 찍혀 10초를 시작시켰다). 이제 fake가 `lastKeyAt`을 들고
멈춤이 시계와 함께 자라며, 테스트는 진짜 규칙 — **앱에 친 마지막 키로부터 10초** — 을 고정한다.
단언은 하나도 안 고쳤다. **가짜 입력이 실물과 다른 모양이면 초록은 아무것도 증명하지 않는다.**

남은 것: 실사용 확인, 그다음 영상 → 게임(`docs/state-sources.md` §8의 3~5단계). Windows 셸
배선은 `docs/windows.md` W8 — 프로세스 이름, 훅 없는 키보드 판정, 설정, 트레이 하위 메뉴,
그리고 위 입력 셋.

## 상태 어휘는 Petdex가 정본이다

`PetdexState` 9종의 **뜻·표준 길이·transient/steady 분류는 우리가 정하지 않는다.** upstream
(`petdex/src/lib/pet-states.ts`, `petdex-desktop-native/src/hook_runner.zig`, `main.zig`)에서
포팅한 값이고, 파일 상단 주석에 출처가 적혀 있다. 바꿔야 하면 그 세 파일을 다시 읽는다.

`PetCapability` 16종 중 9종은 Petdex 행 하나를 그대로 뜻하고 나머지 7종은 확장이다.
**capability에 track 이름을 직접 붙이지 않는다** — `petdexState`와 `borrows`를 선언하면
resolver가 후보를 생성한다. 특히 `jumping`은 축하가 아니라 **턴 시작** 신호이고 완료 신호는
`waving`이다. 이 둘을 이름만 보고 뒤집어 둔 것이 오래된 결함이었다.

**행별 프레임 수는 고정이다.** Petdex 데스크탑 렌더러(`sprite.zig`)는 매니페스트 타이밍을
읽지 않고 행마다 고정된 앞 N칸만 재생한다 — idle 6 · 걷기 8 · waving 4 · jumping 5 ·
failed 8 · waiting 6 · running 6 · review 6. 더 그리면 뒤는 아무도 못 보고, 덜 그리면 빈
칸이 깜빡인다.

**Roamling 확장은 `roamling.json`과 자기 시트에 산다.** `pet.json`과 `spritesheet.webp`는
9행 계약 그대로 두고 건드리지 않는다. 확장 프레임 인덱스는 패키지 격자 끝에서 이어지므로
(8×9면 72번이 확장 시트의 첫 칸) 한 트랙이 두 시트를 섞어도 된다 — `landing`이 패키지의
점프 프레임을 그대로 빌린다.

층 구조와 결정 근거는 `docs/state-contract.md`, 어떤 상황에 어떤 그림이 뜨는지는
`docs/behavior-flow.md`, **지금 시트에 무엇이 그려져 있는지는 `docs/art/mochi-sheet.md`**에 있다. 그것을 만들기
전의 진단과 계획은 `docs/history/mochi-v3-plan.md`, v2 시트의 기록은
`docs/history/mochi-v2-animation-spec.md`다.
**행을 새로 그리기 전에 이 문서들을 읽는다.**

## Atlas 규격은 두 종류다 — 절대 섞지 말 것

| | 내장 마스코트 | Petdex/Codex pet package |
|---|---|---|
| 파일 | `Sources/RoamlingPet/Resources/BuiltInPets/*-runtime-atlas.png` | `~/.codex/pets/<id>/spritesheet.webp` + `pet.json` |
| 배치 | 8열 × **7행**, cell 192×208 (1536×1456) | v1 8×**9**, v2 8×**11** |
| 행 순서 | idle, running right, running left, sleeping, caught, stretching, landing | `docs/art` 및 pet manifest 참조 |

내장 7행 레이아웃(FatMochi)은 Roamling 내부 asset이지 새 Petdex 규격이 아니다.

**내장 Mochi는 이제 shipped `mochi-v3` 패키지와 같은 파일이다.**
`mochi-standard-atlas.webp`(8×9)와 `mochi-extension-atlas.webp`(8×3)가
`~/.codex/pets/mochi-v3`의 `spritesheet.webp` · `roamling.webp`와 같은 바이트다.
패키지가 바뀌면 두 파일을 같이 복사하고, 매니페스트 타이밍은 `MascotPetFactory`에
옮겨 적혀 있으므로 같이 고친다 — 테스트가 트랙 길이와 프레임이 그려진 칸에
떨어지는지를 고정한다.

Petdex 9종과 Roamling capability 16종의 간격, 무엇이 항상 대체되는지, Roamling 전용
펫을 만들 때의 행 구성은 `docs/pets.md`에 있다. 펫이 "동작을 안 한다"고 보일 때 로직을
파기 전에 그 문서의 커버리지 절을 먼저 읽는다.

pet 탐색 순서: `$ROAMLING_PET_PATH` → `~/Library/Application Support/Roamling/Pets`
→ `~/.codex/pets` → `~/.petdex/pets`.

## 아트 불변식 (`docs/art/mochi-animation-handoff.md`)

프레임 결함의 원인이 기록돼 있다. 이 규칙을 완화하는 제안을 먼저 하지 않는다.

- 프레임마다 머리부터 꼬리·네 발까지 **전신을 한 장으로** 새로 그린다.
- 눈/발/얼굴만 생성해 붙이는 patch, cut-and-paste, inpainting fragment,
  이전 프레임 조각 재사용 금지.
- 한 strip 안에서 canvas, scale, ground line, body center가 고정된다.
- 프레임의 visible pixel은 모두 하나의 connected component여야 한다. 떨어진
  수염·털·발 조각이 있으면 수선하지 말고 해당 전신 프레임을 재제작한다.
- 생성 배경은 flat `#00FF00`. chroma 제거·축소·중앙 정렬·mirror·packing은 Roamling
  쪽에서 **완성된 전체 프레임 단위로만** 한다. 왼쪽 걷기는 승인된 오른쪽의 full-frame
  mirror다.

## 이미지 생성 경로

생성 경로는 붙어 있는 도구에 따라 다르다. PixelLab MCP가 붙어 있으면 Claude가 직접
생성한다. 없으면 직접 만들려 하지 말고 사용자에게 경로를 확인한다. 어느 쪽이든 생성
이후는 Claude의 몫이다 — frame 추출, alpha/identity 검증, atlas 합성, QA, packaging,
Swift 런타임 작업. PNG는 직접 읽어 육안 QA할 수 있다.

- **PixelLab MCP가 현재 계획된 경로다.** 도구 문서는 `https://api.pixellab.ai/mcp/docs`,
  설정은 `https://api.pixellab.ai/mcp`에 Bearer 헤더뿐이라 OAuth 흐름이 없다. `quadruped`
  + `template='cat'`, `view='side'`가 이 프로젝트의 펫과 맞고, character 객체 하나에서
  모든 애니메이션이 파생돼 정체성이 규율이 아니라 구조로 유지된다. 파이프라인·비용·미확인
  사항은 `docs/pets.md`의 PixelLab 절에 있다.
- **`api.pixellab.ai/v1`은 deprecated다.** 거기서 읽히는 제약(skeleton 3프레임 윈도우,
  `animate-with-text` 64×64 고정, 200×200 상한, "Tier 1부터 320×320")은 MCP에 적용되지
  않는다. 이전 기록이 그 값을 담고 있었으므로 v1 문서를 근거로 계획을 세우지 않는다.
- **Mochi v1은 Codex의 `hatch-pet` 스킬로 만들었다.** 스킬 본체는
  `~/.agents/skills/hatch-pet`에 있고 Codex 내장 `$imagegen`을 쓴다. Claude 쪽에는
  설치하지 않는다. 수동 경로가 필요하면 `docs/art/mochi-animation-prompts-ko.md`의
  코드블록을 ChatGPT 이미지에 그대로 복붙한다.
- **생성한 frame은 합성 전에 QA 게이트를 통과시킨다.**
  `scripts/pet_qa.py`가 baseline·중심·detached component를 재고 위반 시 non-zero로 끝난다.
  한 프레임의 결함은 육안으로 찾을 수 있는 크기가 아니다 — 기존 시트에서 22프레임이 1px
  조각을 달고 있었다. `--baseline`은 **마지막 불투명 행**이라 Mochi는 175다(미리보기가
  긋는 지면선 176과 한 칸 다르다). 면제는 행 단위가 아니라 프레임 단위로 준다 —
  `--allow-airborne 1`은 달리기 행의 착지 프레임 5장까지 검사에서 빼버린다. 꼬리가
  흔들리는 행은 실루엣 중심이 12.5px 밀리므로 `--center-measure 8=head`로 머리를 잰다.
  Mochi v3의 정식 호출은 `docs/history/mochi-v3-plan.md` 0.5절에 있다.

## Python

시스템·Homebrew python3 어디에도 Pillow가 없다. 이미지 처리 script는 반드시
`./scripts/pyimg.sh <script.py|-c '...'>` 로 실행한다 (uv의 임시 환경 경유).

## 작업 중 상태와 output/

`output/`은 git 미추적이고 `.gitignore`에 있다(수백 MB의 중간 산출물). **커밋에
포함하지 않는다.** `git add -A` 대신 대상 경로를 명시한다.

sprite 작업의 승인 상태는 **`output/v3/approvals.json`** 이 유일한 기록이다. 어떤
트랙이 approved/pending인지 판단할 때 이 파일을 먼저 읽는다. 승인 근거는 notes 한
줄뿐이므로, 반려된 후보를 다시 제안하기 전에 사용자에게 확인한다. 반려된 후보 자체는
`output/v3/bcuts/`에 무엇이 틀렸는지와 함께 남아 있다.

`output/hatch-pet/mochi-row-review/run/qa/approvals.json`은 표준 9행만 다루던 이전
리뷰 런의 기록이라 확장 트랙이 없다. 그쪽을 보지 않는다.

## Commit

영어 명령형 제목. 최근 이력은 `Add …` / `Document …` / `Improve …` 형태이고 이전
이력에는 `feat:` `fix:` `tune:` `polish:` prefix가 섞여 있다. 새 커밋은 접두사 없는
명령형 문장을 쓴다. 커밋과 push는 사용자가 요청할 때만 한다.
