# Roamling agent guide

macOS desktop companion runtime. Swift 6 / AppKit / SwiftPM, GPL-3.0-only.
제품 원칙은 하나다 — **Cute first. Useful second. Never annoying.** 반응 빈도나
움직임을 늘리는 변경은 이 원칙을 먼저 통과해야 한다.

이 파일은 `AGENTS.md`로도 심볼릭 링크돼 있어서 Claude Code와 Codex가 같은 규칙을 읽는다.
규칙이 갈라지지 않도록 수정은 항상 `CLAUDE.md`에서 한다.

## Build, test, run

```sh
swift build                    # 약 5초
./scripts/test.sh              # RoamlingLogicTests + cargo test, 실패 시 non-zero
swift run Roamling
./scripts/build-app.sh release # build/Roamling.app (ad-hoc codesign)
```

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

## 사용자에게 보이는 문자열

menu, alert, tuning panel copy는 전부 `Sources/RoamlingShell/Resources/{en,ko}.lproj/
Localizable.strings`에 있고 `localized(_:)` / `localizedFormat(_:_:)`로 읽는다. **메뉴 트리와
알림 문구도 같은 모듈(`ShellMenu` · `ShellPrompt`)에 있다** — AppKit은 렌더만 한다. 새 UI
문자열을 넣을 때는 **두 파일에 같은 key를 넣는다.** en이 base라 ko를 빠뜨리면 조용히
영어로 나온다. 언어 분기 코드는 쓰지 않는다 — 한국어는 ko.lproj, 나머지는 bundle이
알아서 en으로 떨어진다.

제품명(`Roamling`, `Claude Code`, `Codex`)은 번역하지 않는다.

`RoamlingShell`과 `RoamlingPet`이 resource bundle을 가지므로 `scripts/build-app.sh`가 `$BIN_DIR/*.bundle`을
전부 복사한다. 빠지면 `Bundle.module`이 런타임에 trap한다.

## 모듈 경계

```text
RoamlingCore/     OS 비의존. geometry, world, behavior, attention, reaction
rust/roamling-core/  결정 로직의 정본. 단위 1~7 완료 — Core 전체 + tick 본체 +
                  애니메이션 해석. Swift 쪽 원본은 대조군으로만 남아 있다
RoamlingCoreRs/   생성된 uniffi 바인딩. Engine이 RustCore.swift로 감싸 쓴다
RoamlingPet/      Petdex manifest, atlas runtime, built-in mascot, fallback
                  이미지는 PetImage(RGBA8)다. 디코딩은 PetImageSourcing 뒤에 있다
RoamlingSources/  ClaudeCode / Codex activity adapter + BSD 소켓 loopback transport
RoamlingEngine/   RoamlingRuntime — tick loop, placement, activity orchestration
                  RuntimeTuning도 여기 산다 (규칙은 Rust에 있고 Core는 seam을 못 부른다)
RoamlingShell/    메뉴 트리·알림 문구·Localizable.strings. 위젯은 없다
RoamlingMac/      AppKit display, pointer, overlay, 메뉴 렌더러, app delegate
RoamlingApp/      entry point
rust/roamling-pet/   시트 디코딩(image 크레이트)과 내장 마스코트. makeStandardMochi의
                  이식이고, 옛 authored 시트와 fallback은 아직 Swift에 있다
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
`docs/mvp.md`에 있다. **배터리를 이유로 무언가를 바꾸기 전에 `docs/battery.md`를 읽는다** —
무엇이 실제로 비싼지의 실측과, 이미 되어 있어서 다시 할 필요가 없는 것들이 적혀 있다.
(요약: capture 1회 62 ms가 나머지 전부의 만 배다. 산술은 손댈 것이 없다.) **MVP 사다리는 4에서 멈췄고(2026-09-02 완료), W1 Runtime 추출도 같은 날
닫혔다. W2(이미지 파이프라인 탈-CoreGraphics)도 2026-09-02에 닫혔다 — 남은 것은 디코더
(W2b)이고 그건 언어 결정과 같은 자리에 있다.** exit rule이 있으므로 사용자의 실사용 확인
전에 다음 게이트로 넘어가지 않는다. 리팩터 게이트 중에는 **동작·타이밍·기본값을 고치지
않는다** — W2의 exit에는 렌더 프레임 336개의 바이트 비교가 포함됐고, 그 픽스처는
`Tests/RoamlingLogicTests/PreW2FrameHashes.swift`다.

**언어 결정은 2026-09-02에 D(Rust core + Swift macOS 셸)로 닫혔고, 포팅은 2026-09-03에
단위 1~7이 끝났다.** 근거·순서·되돌아올 조건은 `docs/windows.md` 3절에 있다.

**펫이 무엇을 할지 결정하는 코드는 이제 전부 `rust/roamling-core`에 있다.** geometry ·
world · topology · emptiness · 배치 · attention · 반응 · 튜닝 · 활동 지휘 · tick 본체 ·
애니메이션 해석까지. macOS 앱이 그것을 쓰고 있고 `RoamlingRuntime`은 667줄만 남았다 —
타이머 · UserDefaults · 진단 파일 · agent 구독 · 스프라이트 시트, 즉 결정이 아닌 것들뿐이다.
**현재 게이트는 W4(Windows 최소 루프)이고, 착수 순서와 호출 예시는 `docs/windows.md`의
W4 절에 있다.**

포팅 규칙은 그대로다: **살아 있는 구현은 항상 1벌.** Swift 원본은 대조군으로 Core에
남아 있고(`MovementController` · `PlacementDirector` · `AttentionModel` 등), 단위마다
게이트가 둘이다 — `cargo test`의 differential fixture 10개(14 MB, 6만+ 케이스)와 Swift
쪽에서 두 구현을 나란히 돌려 비교하는 테스트. 런타임처럼 위에 호출자가 없어 대조군을
만들 수 없는 경우에는 **실물을 40초 녹화해서**(`Tests/RoamlingLogicTests/RuntimeTrace.txt`)
바이트 단위로 같은 답을 요구한다. 재생성은
`ROAMLING_WRITE_TRACE=<path> swift run RoamlingLogicTests`이고, **통과시키려고 다시 만들지
않는다.**

**그 `cargo test --release`를 2026-09-03에 Windows에서 돌렸고, 10개 중 5개가 깨졌다.**
포팅 결함이 아니라 `hypot`/`atan2`가 플랫폼 libm으로 새는 것이 원인이다 — macOS는 정확
반올림을 하고 MSVC UCRT는 하지 않는다. 어긋남은 전부 1 ULP이고 결정은 바뀌지 않는다.
**처방과 실측치는 `docs/windows.md` W4의 "실행 결과 · 처방" 절에 있다.**

**그 처방은 2026-09-03에 시행됐다.** `hypot`은 `(dx*dx + dy*dy).sqrt()`로 바뀌었고
(Swift 2곳 · Rust 2곳 + fixture 재생성, 한 커밋), `atan2`는 `look_direction_degrees` 한
필드만 1 ULP 면제됐다. 바뀐 fixture 5개가 Windows에서 깨진 5개와 정확히 일치했고,
**녹화된 세션은 한 바이트도 바뀌지 않았다** — 40초 동안 어떤 비교도 뒤집히지 않았다.
Windows에서 `cargo test --release`가 이제 초록이어야 한다. **아니라면 그것은 새로운
발견이므로 fixture를 다시 만들지 말고 원인을 찾는다.**

**이 경계가 Windows port의 전제다.** `docs/windows.md`에 모듈별 실측 이식 비용, 언어
선택 네 가지의 비교, 그리고 2026-09-01에 Windows에서 실행한 W0 스파이크 결과가 있다.
`RoamlingCore`는 실제 `Package.swift`로 Windows에서 무수정 빌드되고 Core 테스트가 통과한다. **포팅·언어 선택·Rust 재작성 논의를 시작하기 전에 그 문서를 읽는다** — 특히
11절이 Rust 전환 판단에 필요한 실측치를 모아 둔 브리프다.

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
`docs/behavior-flow.md`, **지금 시트에 무엇이 그려져 있는지는 `docs/art/mochi-v3-plan.md`의
"완료 — 실제로 만들어진 것"** 절에 있다. 같은 문서의 나머지는 v3를 만들기 전의 진단과
계획이라 현재 상태가 아니고, `docs/art/mochi-v2-animation-spec.md`는 v2 시트의 기록이다.
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
  Mochi v3의 정식 호출은 `docs/art/mochi-v3-plan.md`에 있다.

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
