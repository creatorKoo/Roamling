# Roamling agent guide

macOS + Windows desktop companion runtime. Swift 6 / AppKit / SwiftPM + Rust, GPL-3.0-only.
제품 원칙은 하나다 — **Cute first. Useful second. Never annoying.** 반응 빈도나
움직임을 늘리는 변경은 이 원칙을 먼저 통과해야 한다.

이 파일은 `AGENTS.md`로도 심볼릭 링크돼 있어서 Claude Code와 Codex가 같은 규칙을 읽는다.
규칙이 갈라지지 않도록 수정은 항상 `CLAUDE.md`에서 한다.

**여기는 규칙과 함정이고, 절차와 근거는 `docs/`에 있다.** 어떤 질문에 어떤 문서인지는
[`docs/README.md`](docs/README.md)가 지도다. 닫힌 게이트의 기록은 `docs/history/`에 있으니
결정을 뒤집으려는 것이 아니면 거기부터 읽지 않는다.

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
  못 잡는다 — `scripts/test.sh`는 링크만 보지 문장이 참인지는 안 본다. 일하는 앱을 상태형으로 옮긴
  날, 흐름의 정본인 `docs/behavior-flow.md` §5b가 통째로 거짓이 돼 있었고 우연히 읽다가 잡았다.
- 계기: G 항목(일하는 앱)을 고치는 동안 source에 우회 코드가 다섯 개 붙었고, 계획이 코드와 어긋나
  구현이 네 번 멈췄고, 문서에 먼저 쓴 문장이 여러 번 틀렸다. 테스트는 초록이었지만 동작을 한 번에
  설명할 수 없는 상태가 됐다.

## Build, test, run

```sh
swift build                    # 약 5초
./scripts/test.sh              # 하네스 + cargo test + import·링크 게이트. 실패 시 non-zero
swift run Roamling
./scripts/build-app.sh release # build/Roamling.app — 서명 identity 필수, docs/release.md
```

```powershell
.\scripts\test.ps1        # core + pet + 셸
.\scripts\run.ps1         # 끄고 -> 빌드 -> 다시 켜기. 실행 중인 사본이 exe를 잡고 있다
```

**Rust 툴체인이 필수다** (rustup, `~/.cargo/bin`이 PATH에). `scripts/test.sh`와
`build-app.sh`가 `scripts/build-rust-core.sh`를 먼저 부르므로 cargo 없이는 빌드가 안 된다.
그 스크립트가 uniffi 바인딩을 `Sources/RoamlingCoreRs`와 `Sources/CRoamlingCoreFFI`에
생성한다 — **둘 다 빌드 산출물이라 git 미추적이고 손으로 고치지 않는다.**

XCTest 대신 dependency-free executable harness를 쓴다. Command Line Tools의 compiler/SDK
mismatch가 나면 `ROAMLING_SWIFT_SDK=/path/to/MacOSX.sdk`로 우회한다. 새 Swift 파일에는
기존 파일과 같은 2줄 SPDX 헤더를 넣는다.

**릴리스는 `v*` 태그를 밀면 만들어지고, 태그와 세 곳의 버전이 같아야 한다** — 다르면
워크플로가 실패시킨다. 목록과 이유는 `docs/windows.md`, macOS 쪽 서명·dmg는
`docs/release.md`. **버전 올리기와 태그는 사용자가 말할 때만 한다.**

## 절대 하지 않는 것

- **ad-hoc 빌드.** identity 없이 서명하면 designated requirement가 cdhash로 고정돼
  Accessibility·화면기록 권한이 매 빌드 날아가고, 사용자가 시스템 설정에서 손으로 복구해야 한다.
  AX 관련 작업은 반드시 identity로 서명한 빌드에서 확인한다.
- **픽스처·트레이스를 통과시키려고 다시 만드는 것.** 아래 "포팅 규칙" 참조.
- **`git add -A`로 커밋.** `output/`은 미추적이고 수백 MB의 중간 산출물이다. 대상 경로를 명시한다.
- **커밋·push를 요청 없이.** 사용자가 말할 때만 한다.
- **키보드 훅.** 타임스탬프만 본다 해도 키로거로 보인다. macOS는
  `CGEventSource.secondsSinceLastEventType(_:eventType:)` 하나만 쓴다.
- **사용자의 실사용 확인 전에 다음 게이트로 넘어가는 것.** exit rule이다. 한 단계의 체감
  품질을 닫고 실제 피드백을 받은 뒤에 움직인다.
- **리팩터 게이트 중에 동작·타이밍·기본값을 고치는 것.** W2의 exit에는 렌더 프레임 336개의
  바이트 비교가 포함됐다(`Tests/RoamlingLogicTests/PreW2FrameHashes.swift`).
- **`~/.agents/skills/hatch-pet`을 Claude 쪽에 설치하는 것.** Codex 전용 스킬이다. 수동
  경로가 필요하면 `docs/art/mochi-animation-prompts-ko.md`의 코드블록을 그대로 쓴다.

## 모듈 경계

```text
RoamlingCore/     OS 비의존. geometry, world, behavior, attention, reaction.
                  Rust로 넘어간 것들의 대조군으로 남아 있다
rust/roamling-core/  결정 로직의 정본. Core 전체 + tick 본체 + 애니메이션 해석 +
                  시트 디코딩. 상태형 source 층(source_state.rs)도 여기
RoamlingCoreRs/   생성된 uniffi 바인딩. Engine이 RustCore.swift로 감싸 쓴다
RoamlingPet/      Petdex manifest, atlas runtime, 내장 마스코트. 이미지는 PetImage(RGBA8)
RoamlingSources/  ClaudeCode / Codex activity adapter + BSD 소켓 loopback transport
RoamlingEngine/   RoamlingRuntime — tick loop, placement, activity orchestration.
                  RuntimeTuning도 여기 (규칙은 Rust에 있고 Core는 seam을 못 부른다)
RoamlingShell/    메뉴 트리·알림 문구·Localizable.strings. 위젯은 없다
RoamlingMac/      AppKit display, pointer, overlay, 메뉴 렌더러, app delegate
RoamlingApp/      entry point
rust/roamling-pet/    내장 마스코트 · 펫 패키지
rust/roamling-agent/  훅 payload 정규화 · 인증 loopback 수신기. 제품별 payload는 안 나간다
rust/roamling-update/ 버전 비교 · appcast 파싱 · Ed25519 검증. 양 플랫폼 공유
rust/roamling-win/    Windows 셸. 코어를 rlib으로 직접 링크한다 — FFI가 없다.
                      Win32 타입은 platform.rs를 넘지 않는다
```

**의존 방향은 항상 바깥 → Core다.** Core에 AppKit이나 agent-specific 타입을 넣지 않는다.

**Core·Pet·Sources·Engine·Shell 다섯과 테스트 하네스는 window system도 Apple 이미지
프레임워크도 import하지 않는다.** macOS SDK에 다 있어서 컴파일러는 이걸 못 잡는다 —
`scripts/test.sh`가 grep으로 막고 걸리면 non-zero로 끝난다. 런타임이 플랫폼에 닿는 통로는
`PlatformServices` 하나이고, macOS 쪽 조립은 `MacPlatform.makeServices()` 한 함수다.

**`roamling-win`은 workspace `default-members`에 없다** — 맨 `cargo test`가 macOS에서
`windows` 크레이트를 빌드하려다 깨진다. Windows에서는 `cargo build -p roamling-win`.

**언어 결정은 2026-09-02에 D(Rust core + Swift macOS 셸)로 닫혔다.** 근거·순서·되돌아올
조건은 `docs/history/windows.md` 3절, Rust 전환 판단의 실측 브리프는 같은 문서 11절이다.
**포팅·언어 선택·재작성 논의를 시작하기 전에 그것을 읽는다.**

**펫이 무엇을 할지 결정하는 코드는 전부 `rust/roamling-core`에 있다.** `RoamlingRuntime`에
남은 것은 타이머 · UserDefaults · 진단 파일 · agent 구독 · 스프라이트 시트, 즉 결정이 아닌
것들뿐이다.

**배터리를 이유로 무언가를 바꾸기 전에 `docs/battery.md`를 읽는다** — capture 1회 62 ms가
나머지 전부의 만 배다. 산술은 손댈 것이 없다.

### 포팅 규칙 — 살아 있는 구현은 항상 1벌

Swift 원본은 대조군으로 Core에 남아 있고, 단위마다 게이트가 둘이다 — `cargo test`의
differential fixture 10개(12 MB, 6만+ 케이스)와 Swift 쪽에서 두 구현을 나란히 돌려 비교하는
테스트. 런타임처럼 위에 호출자가 없어 대조군을 만들 수 없는 경우에는 **실물을 40초 녹화해서**
(`Tests/RoamlingLogicTests/RuntimeTrace.txt`) 바이트 단위로 같은 답을 요구한다.

재생성은 `ROAMLING_WRITE_TRACE=<path> swift run RoamlingLogicTests`이고, **통과시키려고 다시
만들지 않는다.** Windows에서 `cargo test --release`가 깨지면 그것은 새로운 발견이므로
fixture를 다시 만들지 말고 원인을 찾는다 — 1 ULP 부동소수점 처방은 2026-09-03에 이미
시행됐다(`docs/history/windows.md` W4).

## 리소스 번들 — `Bundle.module`을 쓰지 않는다

`scripts/build-app.sh`가 `$BIN_DIR/*.bundle`을 `Contents/Resources`로 복사하지만 **복사만으로는
부족하다.** `RoamlingShell`과 `RoamlingPet`은 `Bundle.module`이 아니라 `petResourceBundle` ·
`shellResourceBundle`을 쓴다. **새 리소스 접근도 그 둘을 쓴다.**

`Bundle.module`은 두 군데만 본다: `Bundle.main.bundleURL` 바로 밑과, **그 바이너리를
컴파일한 기계의 절대 `.build` 경로.** 손으로 조립한 `.app`에는 둘 다 틀리다 — 앱 번들 루트의
리소스는 `codesign`이 봉인을 거부하므로(`unsealed contents present in the bundle root`)
리소스는 `Contents/Resources`에 가는데 **거기가 그 접근자가 유일하게 안 보는 곳이다.**

**만든 기계에서는 두 번째 후보가 늘 존재해서 아무 문제가 없다.** 자기가 빌드하지 않은
바이너리를 돌린 첫 기계가 첫 macOS 릴리스(v0.3.0)를 설치한 사용자였고, 앱은 실행 즉시
trap했다. 그래서 릴리스와 리허설 양쪽이 **`.build`를 치운 채로 패키징된 앱을 실제로 켜 본다**
(`ROAMLING_SMOKE_TEST=1`). 이 결함은 그 조건에서만 보인다.

## 사용자에게 보이는 문자열

menu, alert, tuning panel copy는 전부 `Sources/RoamlingShell/Resources/{en,ko}.lproj/
Localizable.strings`에 있고 `localized(_:)` / `localizedFormat(_:_:)`로 읽는다. **메뉴 트리와
알림 문구도 같은 모듈(`ShellMenu` · `ShellPrompt`)에 있다** — AppKit은 렌더만 한다.

**새 UI 문자열은 두 파일에 같은 key를 넣는다.** en이 base라 ko를 빠뜨리면 조용히 영어로
나온다. 언어 분기 코드는 쓰지 않는다. 제품명(`Roamling`, `Claude Code`, `Codex`)은 번역하지
않는다.

## 마크는 하나다 — 🐾

메뉴바 status item의 title, Windows 트레이 아이콘, macOS 앱 아이콘이 **같은 글리프**를 쓴다.
셋 다 **그려서** 쓴다 — 비트맵을 넣어 두면 요청된 크기와 어긋난다. 메뉴바는
`RoamlingAppDelegate`가 title에 `"🐾"`, Windows 트레이는 `tray.rs`의 `paw_icon()`이 GDI로
그린다. 아이콘 파일만 커밋한다
(`assets/Roamling.icns` · `assets/Roamling.ico`, 후자는 전자에서 **픽셀을 그대로 복사**한다).
마크를 바꾸면 `scripts/build-icon.sh`를 다시 돌린다.

**글리프의 실제 잉크를 재서 맞춘다** — 폰트 크기를 고르면 side bearing을 추측하게 되고,
추측하면 여백만 넓고 발바닥은 작아진다. **크기마다 따로 그린다**: 1024를 줄이면 16px에서
발가락이 뭉갠다.

## 상태 어휘는 Petdex가 정본이다

`PetdexState` 9종의 **뜻·표준 길이·transient/steady 분류는 우리가 정하지 않는다.** upstream
(`petdex/src/lib/pet-states.ts`, `petdex-desktop-native/src/hook_runner.zig`, `main.zig`)에서
포팅한 값이고, 파일 상단 주석에 출처가 적혀 있다. 바꿔야 하면 그 세 파일을 다시 읽는다.

**capability에 track 이름을 직접 붙이지 않는다** — `petdexState`와 `borrows`를 선언하면
resolver가 후보를 생성한다. 특히 `jumping`은 축하가 아니라 **턴 시작** 신호이고 완료 신호는
`waving`이다. 이 둘을 이름만 보고 뒤집어 둔 것이 오래된 결함이었다.

**행별 프레임 수는 고정이다.** Petdex 데스크탑 렌더러(`sprite.zig`)는 매니페스트 타이밍을
읽지 않고 행마다 고정된 앞 N칸만 재생한다 — idle 6 · 걷기 8 · waving 4 · jumping 5 ·
failed 8 · waiting 6 · running 6 · review 6. 더 그리면 뒤는 아무도 못 보고, 덜 그리면 빈
칸이 깜빡인다.

**Roamling 확장은 `roamling.json`과 자기 시트에 산다.** `pet.json`과 `spritesheet.webp`는
9행 계약 그대로 두고 건드리지 않는다.

층 구조는 `docs/state-contract.md`, 어떤 상황에 어떤 그림이 뜨는지는 `docs/behavior-flow.md`,
**지금 시트에 무엇이 그려져 있는지는 `docs/art/mochi-sheet.md`**. 행을 새로 그리기 전에 이
셋을 읽는다.

## Atlas 규격은 두 종류다 — 절대 섞지 말 것

| | 내장 마스코트 | Petdex/Codex pet package |
|---|---|---|
| 파일 | `Sources/RoamlingPet/Resources/BuiltInPets/*` | `~/.codex/pets/<id>/spritesheet.webp` + `pet.json` |
| 배치 | 8열 × 9행 + 확장 8×3 | v1 8×**9**, v2 8×**11** |

**내장 Mochi는 shipped `mochi-v3` 패키지와 같은 바이트다.** 패키지가 바뀌면 두 파일을 같이
복사하고, 매니페스트 타이밍은 `MascotPetFactory`에 옮겨 적혀 있으므로 같이 고친다.

pet 탐색 순서: `$ROAMLING_PET_PATH` → `~/Library/Application Support/Roamling/Pets`
→ `~/.codex/pets` → `~/.petdex/pets`. 커버리지와 대체 사슬은 `docs/pets.md` — 펫이 "동작을
안 한다"고 보일 때 로직을 파기 전에 그 문서를 먼저 읽는다.

## 아트 불변식 (`docs/art/mochi-animation-handoff.md`)

프레임 결함의 원인이 기록돼 있다. **이 규칙을 완화하는 제안을 먼저 하지 않는다.**

- 프레임마다 머리부터 꼬리·네 발까지 **전신을 한 장으로** 새로 그린다.
- 눈/발/얼굴만 생성해 붙이는 patch, cut-and-paste, inpainting fragment,
  이전 프레임 조각 재사용 금지.
- 한 strip 안에서 canvas, scale, ground line, body center가 고정된다.
- 프레임의 visible pixel은 모두 하나의 connected component여야 한다. 떨어진
  수염·털·발 조각이 있으면 수선하지 말고 해당 전신 프레임을 재제작한다.
- 생성 배경은 flat `#00FF00`. chroma 제거·축소·중앙 정렬·mirror·packing은 Roamling
  쪽에서 **완성된 전체 프레임 단위로만** 한다.

**생성한 frame은 합성 전에 QA 게이트를 통과시킨다.** `scripts/pet_qa.py`가 baseline·중심·
detached component를 재고 위반 시 non-zero로 끝난다. **한 프레임의 결함은 육안으로 찾을 수
있는 크기가 아니다** — 기존 시트에서 22프레임이 1px 조각을 달고 있었다. 정식 호출과 면제
플래그의 의미는 `docs/history/mochi-v3-plan.md` 0.5절.

**이미지 생성은 PixelLab MCP가 계획된 경로다.** 붙어 있으면 Claude가 직접 생성하고, 없으면
직접 만들려 하지 말고 사용자에게 경로를 확인한다. 어느 쪽이든 생성 이후(frame 추출, alpha
검증, atlas 합성, QA, packaging)는 Claude의 몫이다. 파이프라인·비용은 `docs/pets.md`.
**`api.pixellab.ai/v1`은 deprecated이므로 v1 문서를 근거로 계획을 세우지 않는다.**

## Python

시스템·Homebrew python3 어디에도 Pillow가 없다. 이미지 처리 script는 반드시
`./scripts/pyimg.sh <script.py|-c '...'>` 로 실행한다 (uv의 임시 환경 경유).

## 작업 중 상태와 output/

`output/`은 git 미추적이고 `.gitignore`에 있다. sprite 작업의 승인 상태는
**`output/v3/approvals.json`** 이 유일한 기록이다 — 어떤 트랙이 approved/pending인지 판단할
때 이 파일을 먼저 읽는다. 반려된 후보를 다시 제안하기 전에 사용자에게 확인한다.
`output/hatch-pet/mochi-row-review/run/qa/approvals.json`은 표준 9행만 다루던 이전 런의
기록이라 확장 트랙이 없다 — 그쪽을 보지 않는다.

## Commit

영어 명령형 제목, 접두사 없음 (`Add …` / `Document …` / `Improve …`).
**커밋과 push는 사용자가 요청할 때만 한다.**

## 맥에서 해야 하는 일

A~F는 전부 끝났다 — 기록은 `docs/history/mac-fixes.md`. 열려 있는 것은 G 하나다.

### G. agent 없이도 "일하는 중"을 안다 — macOS 확인됨 2026-09-12 · Windows 배선 대기 (W8)

첫 비개발자 사용자(초등교사, Windows, 한글·한쇼·한셀)가 펫이 Claude 옆에서 일하는 걸 보고
**"왜 나는 이런 행동 안 해?"** 라고 물은 데서 나왔다 — 훅을 걸 agent가 없는 사람에게는
제품의 Useful 절반이 통째로 없었다. 이제 **지정한 앱이 앞에 있으면** 같은 흐름이 돈다.

```
일하는 앱이 앞에 옴      → 옆으로 와서 그냥 앉음(idle)      [Beside]
그 세션의 첫 타이핑      → 점프, 이어서 running             [Active + SittingStarted]
마지막 키 뒤 10초        → 갸웃                            [Paused]
갸웃 5초                 → 자리를 비우고 돌아다님            [Away]
앱을 3초 이상 떠남       → 3분 이상 쳤으면 먼저 손 흔듦      [Away + SittingEnded]
```

**흐름과 시간표는 `docs/behavior-flow.md` §5b, 설계는 `docs/state-sources.md`, Windows
배선은 `docs/windows.md` W8.** 고치기 전에 그 셋을 읽는다.

여기 남기는 것은 규칙 넷뿐이다.

- **반응은 앱이 아니라 키 입력에 한다.** 첫 판은 앱에 오기만 해도 갸웃했고 사용자가
  이상하다고 했다. 갸웃은 "기다리는 중"이라 타이핑이 멈췄을 때가 맞다.
- **agent가 우선이다 (사용자 결정 2026-09-11).** agent가 자리를 지키는 동안 일하는 앱의
  상태 전이는 펫에게 가지 않는다. 인사는 보관했다 자리가 나면 쓰고, **작별은 버린다** —
  남겨 두면 agent가 몇 초 뒤 끝났을 때 그 축하 직후에 늦게 튀어나온다.
- **source는 사건이 아니라 상태를 선언한다.** 0.5초마다 갱신하고 2초 끊기면 만료한다.
  **펫의 상태를 셸이 되먹이지 않는다** — 예전의 네 입력은 전부 director 안의 사실이었다.
- **기본값은 빈 목록이다.** 사용자가 메뉴에서 앱을 고르기 전까지 아무것도 하지 않는다 —
  여기서 잘못 짐작하는 것은 "never annoying"을 정면으로 어기는 쪽이다.

**가짜 입력이 실물과 다른 모양이면 초록은 아무것도 증명하지 않는다.** 하네스의 가짜 키보드가
"마지막 키가 N초 전"을 상수로 답해서 멈춘 키보드가 늙지 않았고, 그 위에서 테스트가 **우연히**
통과하고 있었다. 지금은 `lastKeyAt`으로 멈춤이 시계와 함께 자란다.
