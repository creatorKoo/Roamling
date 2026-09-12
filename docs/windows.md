# Windows

Windows 셸은 `rust/roamling-win`이고 코어를 rlib으로 **직접 링크한다** — 이쪽은 FFI가 없다.
Win32 타입은 `platform.rs`를 넘지 않는다. Windows에 Swift는 없다: 그쪽 빌드는 전부 Rust다.

**이 문서는 지금 유효한 것만 적는다.** 게이트 W0~W7은 전부 닫혔고, 그 결정 과정·스파이크 실측·
게이트별 기록은 `docs/history/windows.md`에 있다(2,400줄). **무엇을 왜 그렇게 정했는지 다시
파기 전에 그 문서를 먼저 본다** — 언어 선택 네 가지 비교, W0 스파이크 실행 결과, 캡처 경로가
실측으로 뒤집힌 과정, 1 ULP 부동소수점 처방이 거기 있다.

## 게이트

| | 무엇 | 상태 |
|---|---|---|
| W0 | 스파이크 (Windows 머신) | ✅ 2026-09-01 |
| W1 | Runtime 추출 (macOS, 동작 변화 0) | ✅ 2026-09-02 |
| W2 | 이미지 파이프라인 탈-CoreGraphics | ✅ 2026-09-02 |
| W2b | 이식 가능한 디코더 | ✅ 2026-09-07 |
| W3 · W3b | Sources 이식 · 셸 표면을 데이터로 | ✅ 2026-09-02 |
| W4 | Windows 최소 루프 | ✅ 2026-09-04 |
| W5 | 나머지 provider | ✅ 2026-09-04 |
| W6 | 패키징 (Inno Setup, per-user) | ✅ 2026-09-04 |
| W7 | 자동 업데이트 (양 플랫폼 공통) | ✅ 2026-09-04 |
| **W8** | **지정 앱 활동 source (셸 배선)** | ⏳ **미완 — 아래** |

## 릴리스 없이 시험하기

`.github/workflows/check-windows.yml`이 릴리스의 Windows 잡에서 **발행만 뺀 것**을 돈다
(`gh workflow run check-windows.yml`). 셸이나 인스톨러를 건드린 푸시에서는 자동으로도 돈다.

**이게 필요한 이유가 macOS 쪽과 다르다.** `roamling-win`은 workspace default members에 없어서
개발 기계의 `cargo test`가 **한 번도 빌드하지 않고**, 실제로 빌드되는 유일한 자리가 태그 푸시였다.
공유 코어의 변경이 Windows 셸을 깨도 릴리스 전까지 안 보인다.

더 싼 답도 있다: **맥에서 `cargo check -p roamling-win --target x86_64-pc-windows-msvc`가 돈다.**
`cargo check`는 링크를 안 하니 MSVC 툴체인이 필요 없고, 진짜 `windows` 크레이트 바인딩에 대고
타입 검사가 된다(실측 38초). `rustup target add x86_64-pc-windows-msvc`가 선행 조건이다.

## 빌드·테스트·실행

```powershell
.\scripts\test.ps1        # core + pet + 셸, 실패 시 non-zero
.\scripts\run.ps1         # 끄고 -> 빌드 -> 다시 켜기. release, 트레이만
.\scripts\run.ps1 -Debug  # 같은 것의 debug 빌드. 콘솔에 상태 로그가 찍힌다
```

**빌드한 뒤에는 앱을 다시 켠다.** 상주 펫이라서만이 아니라, **실행 중인 사본이 자기
`roamling.exe`를 잡고 있어** `cargo build`가 "액세스가 거부되었습니다"로 실패한다.
`run.ps1`이 그 셋을 묶어 둔 이유다.

**`roamling-win`은 workspace `default-members`에 없다.** 맨 `cargo test`가 macOS에서
`windows` 크레이트를 빌드하려다 깨지기 때문이다. Windows에서는 `cargo build -p roamling-win`.

**`cargo test --release`가 초록이어야 한다.** 2026-09-03에 differential 픽스처 10개 중 5개가
깨졌는데 포팅 결함이 아니라 `hypot`/`atan2`가 플랫폼 libm으로 새는 것이 원인이었다. 처방은
그날 시행됐다(`docs/history/windows.md` W4의 "실행 결과"·"처방"). **지금도 깨진다면 그것은 새로운
발견이므로 픽스처를 다시 만들지 말고 원인을 찾는다.**

## 파일이 놓이는 자리

| | 어디 |
|---|---|
| 설치 | `%LOCALAPPDATA%\Programs\Roamling` — **per-user라 UAC 프롬프트가 없다** |
| 설정 | `%APPDATA%\Roamling\settings.txt` — 평평한 `key=value`. **키는 macOS와 같다** |
| 펫 | `$ROAMLING_PET_PATH` → `%APPDATA%\Roamling\Pets` → `~/.codex/pets` → `~/.petdex/pets` |

탐색 순서와 설정 키가 macOS와 같은 것은 우연이 아니다 — 규칙이 공유 코어에 있고 셸은 읽고
쓰기만 한다. 한쪽만 바꾸면 두 플랫폼의 동작이 갈린다.

**진짜 단일 파일이다.** `rust/.cargo/config.toml`이 MSVC 타깃에 `+crt-static`을 걸어 OS 밖
DLL이 0개다. 그래서 W6이 DLL 수집도 스테이징도 없이 끝났다.

## 권한 모델 — macOS와 갈리는 자리

**Windows에서는 권한 프롬프트가 없다.** Screen Recording 승인도 Accessibility 승인도 없이
캡처와 창 질의가 된다. macOS의 "OS 권한 승인이 곧 사용자 동의"가 여기서는 성립하지 않는다.

그래서 **Windows의 capture는 opt-in 설정 뒤에 둔다.** 기본값을 켜 두면 "Never annoying"이
아니라 몰래 보는 쪽이 된다. 프라이버시 원칙(디스크 미기록, 로그 미기록, 내용 미해석)은 양쪽
같다.

**키보드 훅(`WH_KEYBOARD_LL`)은 쓰지 않는다** — 타임스탬프만 본다 해도 키로거로 보인다.
macOS도 같은 이유로 `CGEventSource.secondsSinceLastEventType(_:eventType:)` 하나만 쓴다.

## 자동 업데이트 — 지금 켜져 있다

결정 로직은 `rust/roamling-update`에 있고 양 플랫폼이 공유한다. 바이트를 가져오고 파일을
바꾸는 것만 셸이 한다 — Windows는 WinHTTP, macOS는 URLSession.

**서명이 둘이다.** 매니페스트 서명(`appcast.json.sig`)이 버전과 URL을 못 고치게 하고,
아티팩트 서명이 도착한 바이트가 우리 것인지를 말한다. 매니페스트 서명이 없으면 피드를 쥔 쪽이
"9.9.9"라고 주장하면서 **진짜로 우리가 서명한 옛 아티팩트**를 가리킬 수 있다 — 조용한
다운그레이드다.

**`PUBLIC_KEY_HEX`가 전부 0이면 업데이트는 꺼진 상태이고, 그게 안전한 기본값이다.** 서명을
확인할 수 없는 빌드는 업데이트하지 않는다. 지금은 실제 키가 들어 있고 양 플랫폼의 자동
업데이트가 켜져 있다.

**실행 중인 exe는 이름을 바꿔서 교체한다.** Windows는 실행 중인 파일을 지우거나 덮어쓰지
못하지만 이름은 바꿀 수 있다 — 잠금이 이름이 아니라 내용에 걸린다. `roamling.exe` →
`roamling.exe.old`, 새 바이트를 `roamling.exe`로. 헬퍼 프로세스도 예약 작업도 재시작 요구도
없다. 두 번째 rename이 실패하면 옛 파일을 되돌린다. macOS는 여기서 갈린다 — 프로세스가
경로가 아니라 inode를 들고 있어서 실행 중에 번들을 바꿀 수 있다.

**피드는 GitHub 릴리스에 얹혀 있다.** `/releases/latest/download/appcast.json`이 항상 최신
릴리스의 자산으로 리다이렉트된다. 매니페스트 안의 아티팩트 URL은 **태그가 박힌 주소**다 —
`latest`는 서명 아래에서 움직인다.

### 릴리스할 때 사람이 지켜야 하는 것

버전을 올리고 같은 번호로 태그를 민다. **세 곳이 태그와 같아야 하고, 워크플로가 대조해서
다르면 실패시킨다.**

| 어디 | 왜 |
|---|---|
| `rust/Cargo.toml` | 업데이터가 비교하는 값. 어긋나면 설치된 빌드가 **영원히 자기를 업데이트한다** |
| `CFBundleShortVersionString` | 정보 창에 보이는 값 |
| `CFBundleVersion` | macOS가 빌드로 취급하는 값. 두 릴리스 동안 `1`에 머물렀고 LaunchServices가 그 값으로 캐시한다 |

비밀키는 GitHub secret `ROAMLING_UPDATE_SECRET_KEY`에 있다. **키를 다시 만들 일이 생기면
사용자가 직접 돌린다** — 에이전트가 `keygen`을 돌리면 비밀키가 대화 기록에 남는다.

## W8 — 지정 앱 활동 source (Windows 셸 배선) ⏳ 미완

**결정 로직은 macOS 쪽에 붙어 있고 Windows는 셸 배선만 하면 된다.** 규칙은 2026-09-11에
정해졌고(같은 날 v2로 고침), **2026-09-12에 상태형 source로 옮겨졌다** — 이 절은 그 뒤의
API를 기준으로 쓴 것이다. 설계는 `docs/state-sources.md`, 흐름은 `docs/behavior-flow.md` §5b,
그 이전 구조의 기록은 `docs/history/focus-activity-flow.md`.

`rust/roamling-core/src/focus_activity.rs`가 상태 기계 전부를 들고 있다.

```text
지정 앱이 앞에 옴            Beside            옆으로 와서 그냥 앉음(idle), 점프·갸웃 없음
그 세션의 첫 키 입력          Active + SittingStarted   점프, 이어서 running
세션 안의 다음 키 입력        Active            running
마지막 키 뒤 10초             Paused            갸웃
갸웃 5초                      Away              자리를 비우고 돌아다님
돌아다니는 중 다시 키 입력    Active            걸어와서 running, 점프 없음
키 없이 보기만 함             Beside 유지       계속 앉아 있음 (0.5초마다 같은 선언을 다시 냄)
3초 이상 떠남                 Away              세션 타이핑 3분 이상이면 SittingEnded 이정표를
                                                같이 실어 보낸다 — 자리를 비운 뒤라도, 펫이
                                                있는 자리에서 흔든다
앞의 앱을 모름(None)          직전 선언 재발신   "모름"이지 "떠남"이 아니다. 이정표만 떼고 같은
                                                선언을 다시 낸다 — 안 그러면 2초 만료가 세션을
                                                끝낸다
agent가 자리를 지킴           선언은 계속       director가 상태 전이를 보관한다. 인사는 자리가
                                                나면 그때 쓰고, 작별은 버린다
```

셸이 0.5초마다 `observe`에 **입력 넷**을 넘긴다 — `app`, `watched`, `seconds_since_key`, `now`.
돌아오는 것은 `Vec<StateDeclaration>`이고, 보통 하나다(지정 앱에서 다른 지정 앱으로 바로 옮기면
옛 source의 `Away`와 새 source의 `Beside`가 한 샘플에 같이 온다). 셸은 거기에 **창 위치 하나만**
얹어 `pet.declare_state(declaration, now)`를 부른다.

**펫의 사실을 되먹이지 않는다.** 옛 판은 `dispatched_event` · `arrival_pending` · `pet_resting` ·
`agent_on_duty` 넷을 셸이 날라야 했는데, 전부 director 안에 있는 사실이었다. 상태형에서는
director가 직접 본다.

**0.5초 샘플을 거르지 않는다.** 선언은 2초(`STATE_EXPIRY`) 동안 갱신이 없으면 `Away`로 만료한다.
그게 "셸이 말을 멈췄다"를 감지하는 유일한 장치라서, 샘플이 멎으면 좌석도 멎는다.

### 손댈 파일

- `rust/roamling-win/src/focus.rs` (이미 있는 파일 — 창 위치·캐럿을 묻는 곳에 더한다) —
  `pub fn foreground_application() -> Option<String>`:
  `GetForegroundWindow` → `GetWindowThreadProcessId` →
  `OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION)` → `QueryFullProcessImageNameW` → 파일명만
  (`Hwp.exe`). 자기 PID면 `None`. **0.5초마다만 부른다.**
- `rust/roamling-win/src/platform.rs` — 키보드만의 idle. `GetLastInputInfo`의 시각이
  갱신됐는데 포인터 좌표가 직전 샘플과 같으면 키보드로 본다. `keyboard_idle_duration()`.
  한계를 문서에 적어 둘 것: 휠·클릭이 키보드로 잡힐 수 있다.
- `rust/roamling-win/src/settings.rs` — `WORK_APPS = "roamling.workApps"`. **키와 형식이
  macOS와 같다**(쉼표로 구분한 한 줄, 비면 키를 지운다). 기본값은
  `"Hwp.exe,Hshow.exe,Hcell.exe"`를 검토하되 **그 기계에서 실제 exe 이름을 확인할 것**
  (작업 관리자 → 세부 정보). 한컴오피스 2020/2022 기준의 추정치다.
- `rust/roamling-win/src/main.rs`의 `tick` — agent_events를 배수하기 직전에 샘플을 넣는다.
  agent 이벤트와 달리 `pending`에 합치지 않는다 — 상태 선언은 이벤트가 아니라서 attention을
  거치지 않고 `declare_state`로 곧장 들어간다.

  ```rust
  let frontmost = focus::foreground_application();
  let watched = frontmost.as_deref().is_some_and(|app| work_apps.contains(app));
  let hint = if watched { focus::activity_location_hint() } else { None };
  for mut declaration in app.focus_activity.observe(
      frontmost.as_deref(), watched, keyboard_idle, now,
  ) {
      if declaration.focused { declaration.hint = hint.clone(); }
      let requests = app.pet.declare_state(declaration, now);
      // 창 위치를 물어본 만큼 luminance 갱신을 예약한다 — agent 경로와 같다.
  }
  ```

  **창 위치는 `focused`인 선언에만 얹는다.** 떠나는 source의 `Away`에 지금 앞에 있는 창을
  붙이면 펫이 남의 창으로 걸어간다.
  **앞에 있는 앱을 못 알아내면 `None`을 넘긴다. 그건 "모름"이지 "사용자가 떠남"이 아니다** —
  자기 창(트레이 메뉴·설정 창)이 앞에 온 것을 이탈로 읽으면 세션이 끝나 버린다. 코어가 그때
  직전 선언을 이정표 없이 다시 내주므로 셸이 따로 할 일은 없다.
  **출처 종류도 셸이 할 일이 없다.** 지정 앱 선언은 코어가 `System`으로 만들고, Windows의
  agent 이벤트는 `CompanionEvent::new`의 기본값으로 `Agent`가 된다.
- `rust/roamling-win/src/tray.rs` — `MenuState.work_apps: Vec<(String, bool)>`, 하위 메뉴,
  명령 id 블록 하나(agent처럼 10단위), `main.rs` 메뉴 핸들러에 toggle. 문자열은
  `menu.workApps` · `menu.workApps.none`. 목록에 띄울 "최근 앞에 있었던 앱"은
  `focus_activity.recent_apps()`가 준다.

**macOS 쪽 기본값은 빈 목록이다.** Windows에서 기본을 채울지는 그 기계에서 이름을 확인한
뒤에 정한다 — 틀린 이름을 기본값으로 넣으면 아무 일도 안 일어나는 것을 사용자가
"고장"으로 읽는다.

## 남은 리스크

W0가 둘을 없앴고(Swift GUI 상주앱 전례, `.lproj` 로컬라이제이션) 다중 디스플레이는 같은 날
닫혔다. 남은 것은 셋이다.

1. **COM interop** — UIA가 필요해지는 지점. `GetGUIThreadInfo`로 피할 수 있는 데까지 피한다.
2. **툴체인 환경이 macOS보다 무겁다.** vcvars64 + `SDKROOT`이 없으면 빌드가 깨진 툴체인처럼
   실패한다. CI와 기여자 문서에 그대로 비용이 된다.
3. **음수 좌표 배치가 미검증이다.** 보조 화면을 primary 왼쪽/위로 옮기면 1분이면 확인된다.
   혼합 DPI(1.5배·3.0배)는 2026-09-01에 통과했다.
