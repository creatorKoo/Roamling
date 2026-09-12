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
| **W8** | **지정 앱 활동 source (셸 배선)** | ⏳ **배선 완료 · 실사용 확인 대기** |

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

## W8 — 지정 앱 활동 source (Windows 셸 배선) ⏳ 실사용 확인 대기

**결정 로직은 공용 코어에 있고 Windows 셸도 2026-09-12에 배선됐다.** 규칙은 2026-09-11에
정해졌고(같은 날 v2로 고침), 다음 날 상태형 source로 옮겨졌다. 설계는
`docs/state-sources.md`, 흐름은 `docs/behavior-flow.md` §5b, 그 이전 구조의 기록은
`docs/history/focus-activity-flow.md`.

`rust/roamling-core/src/focus_activity.rs`가 상태 기계 전부를 들고 있다.

```text
지정 앱이 앞에 옴            Beside            옆으로 와서 그냥 앉음(idle), 점프·갸웃 없음
그 세션의 첫 키 입력          Active + SittingStarted   점프, 이어서 running
세션 안의 다음 키 입력        Active            running
마지막 키 뒤 10초             Paused            갸웃
갸웃 10초                     Away              자리를 비우고 돌아다님
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

**만료는 셸이 부르지 않는다.** `pet_runtime.rs`의 `begin_tick`이 매 틱 `expire_states`를 이미
부른다(453행). 셸이 할 일은 선언을 계속 내는 것뿐이고, 2초 만료는 공짜로 따라온다.

### 구현

- `rust/roamling-win/src/focus.rs` (창 위치·캐럿을 묻는 곳) —
  `pub fn foreground_application() -> Option<String>`:
  `GetForegroundWindow` → `GetWindowThreadProcessId` →
  `OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION)` → `QueryFullProcessImageNameW` → 파일명만
  (`Hwp.exe`). 자기 PID면 `None`. **0.5초마다만 부른다.**
- `rust/roamling-win/src/platform.rs` — 키보드만의 idle. **`GetAsyncKeyState`를 폴링한다
  (사용자 결정 2026-09-12).** 0.5초마다 문자·숫자·편집 키 범위를 훑어 "직전 샘플 이후 눌린
  키가 있나"만 보고 `last_key_at`을 갱신한다. **낮은 비트가 정확히 그 뜻이라** 샘플 사이에
  눌렸다 뗀 키도 놓치지 않는다 — 높은 비트("지금 눌려 있음")로는 빠른 타이핑이 통째로 샌다.
  훅이 아니라 상태 읽기이므로 `WH_KEYBOARD_LL` 금지 규칙을 지나고, 어떤 키였는지는 남기지
  않으며, 권한도 필요 없다. IME를 거치는 한글 입력도 물리 키 상태라 그대로 잡힌다.

  처음 적혀 있던 처방("`GetLastInputInfo`의 시각이 갱신됐는데 포인터 좌표가 그대로면 키로
  친다")은 **버렸다.** 휠·클릭이 타이핑으로 잡히므로, 한글에서 마우스로 스크롤만 하며 읽는
  사람이 계속 `Active`가 되고 갸웃도 손 흔들기도 영영 오지 않는다 — G 규칙 첫째("반응은 앱이
  아니라 키 입력에 한다")가 막으려던 바로 그 상태다.

  **낮은 비트는 프로세스 안에서 한 번만 읽힌다.** 읽는 순간 지워지므로 두 곳에서 같은 키를
  물으면 한쪽이 못 본다. 지금 `primary_button_down()`과 `affection_held()`는 높은 비트만
  쓰므로 겹치지 않지만, **문자 키의 낮은 비트를 읽는 곳은 이 함수 하나로 유지한다.**

  **지정 앱이 앞에 없어도 매 샘플 폴링한다.** 멈춰 두면 낮은 비트가 쌓였다가 다음 호출에
  한꺼번에 보고되고, 앱에 도착한 직후의 그 한 번이 `SittingStarted`를 잘못 띄운다. 코어의
  Cmd-Tab 규칙(`in_front_since`)은 도착 **이전**의 키만 걸러 주므로 이것을 막지 못한다.
- `rust/roamling-win/src/settings.rs` — `WORK_APPS = "roamling.workApps"`. 쉼표로 구분한 한
  줄이다. 키가 없으면 아래 기본값, 빈 값이면 빈 목록이다. **키와 형식은 macOS와 같지만 값은
  다르다** — macOS는 bundle id, Windows는 exe 파일명(`Hwp.exe`)이다. 설정이 두 기계 사이를 오가지
  않으므로 문제는 없지만, "macOS와 같다"를 값까지 같다고 읽지 않는다.

  **이름은 전부 이 기계에서 확인했다 (2026-09-12)** — 목록은 아래 "기본값" 절.
  옛 추정치 `Hshow.exe` · `Hcell.exe`는 **철자가 틀렸고**(실제는 `HShow.exe` · `HCell.exe`),
  MS Office는 디스크에서 **전부 대문자**다(`WINWORD.EXE` · `POWERPNT.EXE` · `EXCEL.EXE`).
  Windows 파일명은 대소문자를 안 가리지만 `QueryFullProcessImageNameW`는 디스크의 철자를 그대로
  돌려주고 `Vec<String>`의 비교는 대소문자를 가린다 — 그대로 뒀으면 **여섯 중 `Hwp.exe` 하나만
  되고 나머지 다섯은 조용히 아무 일도 안 했을 것이다.** **비교는 `eq_ignore_ascii_case`로 한다**
  (설정 파일을 손으로 고치는 사람도 있다). 문서가 근거로 삼던 한컴오피스 2020/2022는 이 기계에 없다.
- `rust/roamling-win/src/main.rs`의 `tick` — agent_events를 배수하기 직전에 0.5초마다 샘플한다.
  agent 이벤트와 달리 `pending`에 합치지 않는다 — 상태 선언은 이벤트가 아니라서 attention을
  거치지 않고 `declare_state`로 곧장 들어간다.

  ```rust
  let frontmost = focus::foreground_application();
  let keyboard_idle = platform::keyboard_idle_duration();
  // 대소문자를 가리지 않는다 — 위 settings.rs 항목 참조. `HShow.exe`가 그 이유다.
  let watched = frontmost
      .as_deref()
      .is_some_and(|app| work_apps.iter().any(|want| want.eq_ignore_ascii_case(app)));
  let hint = if watched { focus::activity_location_hint() } else { None };
  let declarations = app.focus_activity.observe(
       frontmost.as_deref(), watched, keyboard_idle, now,
  );
  let mut luminance_requests = Vec::new();
  for mut declaration in declarations {
       if declaration.focused { declaration.hint = hint.clone(); }
       luminance_requests.extend(app.pet.declare_state(declaration, now));
  }
  refresh_luminance(app, &luminance_requests, now);
  ```

  **창 위치는 `focused`인 선언에만 얹는다.** 떠나는 source의 `Away`에 지금 앞에 있는 창을
  붙이면 펫이 남의 창으로 걸어간다.
  **앞에 있는 앱을 못 알아내면 `None`을 넘긴다. 그건 "모름"이지 "사용자가 떠남"이 아니다** —
  자기 창(트레이 메뉴·설정 창)이 앞에 온 것을 이탈로 읽으면 세션이 끝나 버린다. 코어가 그때
  직전 선언을 이정표 없이 다시 내주므로 셸이 따로 할 일은 없다.
  **출처 종류도 셸이 할 일이 없다.** 지정 앱 선언은 코어가 `System`으로 만들고, Windows의
  agent 이벤트는 `CompanionEvent::new`의 기본값으로 `Agent`가 된다.
- `rust/roamling-win/src/tray.rs` — `MenuState.work_apps: Vec<(String, String, bool)>`(라벨 · exe ·
  선택), 하위 메뉴,
  명령 id 블록, `main.rs` 메뉴 핸들러에 toggle. 목록에 띄울 "최근 앞에 있었던 앱"은
  `focus_activity.recent_apps()`가 준다. 문자열 `menu.workApps` · `menu.workApps.none`은
  en/ko 양쪽의 기존 값을 쓰고, `strings.rs`의 `the_keys_the_tray_needs_exist`가 두 키를 검사한다.

  **명령 id는 200 블록에 두고, agent arm의 위쪽 끝은 좁혔다.** 이전 agent arm은
  `(CMD_AGENT_BASE..CMD_PET_BUILT_IN)` — **100~999를 통째로** claim했지만 실제로 쓰는 것은
  100~119뿐이었다. match arm은 위에서부터 평가되므로 그대로 두면 앱을 하나 체크할 때마다
  훅 설치 확인 창이 뜬다. 지금 끝은 `CMD_AGENT_BASE + 2 * CMD_AGENT_STRIDE`다.
  `tray.rs`의 `two items share a command id` 테스트 fixture에도 `work_apps`를 넣어 충돌을 검사한다.

  **메뉴에 띄우는 이름은 exe 버전 정보의 `FileDescription`이다 (사용자 결정 2026-09-12).**
  없으면 exe 파일명으로 떨어뜨린다. 목록이 `Hwp.exe` · `WINWORD.EXE`로 뜨면 비개발자는 무슨
  프로그램인지 모르고, **그것이 이 기능을 요청한 사람이다.** macOS 쪽 대응은
  `NSRunningApplication.localizedName`이다.

  **여섯 개를 다 재 봤다 (2026-09-12).** 버전 리소스의 언어 블록은 **중립(0000) 하나뿐이고
  한국어 블록은 없다.**

  | exe | `FileDescription` |
  |---|---|
  | `WINWORD.EXE` · `POWERPNT.EXE` · `EXCEL.EXE` | `Microsoft Word` · `Microsoft PowerPoint` · `Microsoft Excel` |
  | `Hwp.exe` · `HShow.exe` · `HCell.exe` | `HWP 2024` · `Show 2024` · `Cell 2024` |

  MS 셋은 정확하다. **한컴 셋은 "한글·한쇼·한셀"이 아니지만 그대로 간다** — 아는 이름은
  아니어도 파일명보다는 프로그램처럼 읽힌다. `ProductName`은 MS 셋이 전부 `Microsoft Office`로
  뭉개져서 더 나쁘다.

  **이것은 뒤집힌 결정이다.** 처음 `FileDescription`을 쟀을 때 `Show 2024` 하나만 보고 "버전
  정보를 들이지 않는다"로 적었었다. MS 셋을 같이 재 보니 틀린 판단이었다 — 대다수가 쓰는 셋이
  정확히 나오는데 한컴 하나 때문에 여섯 전부를 파일명으로 두고 있었다.

  **시작 메뉴 바로가기는 쓰지 않는다.** 거기에는 `한글 2024` · `한쇼 2024` · `한셀 2024`가
  그대로 있다(실측). 하지만 `.lnk` 대상 해석에 **IShellLink COM**과 시작 메뉴 재귀 스캔이
  필요하고, COM interop은 이 문서 "남은 리스크" 1번이 피하라고 적어 둔 바로 그것이다.
  **내장 예외 표(한컴 셋만 한국어로)도 쓰지 않는다** — 남의 제품명을 코드에 박는 값은
  치르지 않기로 했다(사용자 결정).

  `Cargo.toml`에 `Win32_Storage_FileSystem` feature가 필요하다
  (`GetFileVersionInfoSizeW` · `GetFileVersionInfoW` · `VerQueryValueW`).

  **알아낸 이름은 설정에 저장한다 (사용자 결정 2026-09-12).** 첫 판은 캐시가 메모리에만 있었고
  **그 exe가 포그라운드였던 적이 있을 때만** 채워졌다 — 새로 켜면 메뉴가 `Hwp.exe` ·
  `WINWORD.EXE`로 뜨고, 앱을 한 번 띄워야 이름이 붙고, 재시작하면 도로 파일명이 됐다.
  **메뉴를 처음 여는 순간이 정확히 이 기능이 필요한 순간이라** 그대로 두면 안 고쳐진 것으로 보인다.

  키는 앱마다 하나씩 둔다 — `roamling.workAppLabel.<소문자 exe>`. 한 줄에 몰아넣지 않는 이유는
  라벨에 쉼표가 들어갈 수 있어서이고, `roamling.runtimeTuning.<이름>`이 이미 같은 꼴이다.
  **값이 파일명과 같으면 저장하지 않는다** — 그건 이름이 아니라 "아직 못 알아냈다"이고, 저장해
  두면 나중에 진짜 이름을 알아내도 그 자리를 막는다.

  **지정 앱만 저장한다 — 앞에 왔던 앱 전부가 아니다.** 첫 구현이 이 선을 긋지 않아서, 펫을 8초
  띄운 것만으로 `roamling.workAppLabel.windowsterminal.exe=Windows Terminal Host`가 적혔다
  (2026-09-12 실측). 지정 앱이 아닌데도 적힌 것이다. 그대로 두면 **설정 파일이 "이 사람이 무슨
  프로그램을 써 왔는가"의 기록으로 자란다** — 캡처에서 디스크 미기록·로그 미기록을 지키는 것과
  같은 이유로 하지 않는다. 메모리 캐시는 최근 목록을 위해 전부 들고 있어도 되지만, **디스크로
  가는 것은 `work_apps`에 있는 것뿐이다.** 저장이 필요한 이유 자체가 "아직 한 번도 안 띄운 지정
  앱을 이름으로 보여주기"이므로 그 범위면 충분하다.

  **macOS도 같은 구멍이 둘 다 있다.** `applicationDisplayName`이 `NSRunningApplication`만 훑으므로
  실행 중이 아닌 지정 앱은 `com.microsoft.Word`로 그대로 뜨고, 메뉴가 최근 앱의 이름까지 물으므로
  지정하지 않은 앱도 같이 저장된다. 두 규칙을 그대로 적용한다.

### 기본값 — 양 플랫폼 모두 채웠다 (사용자 결정 2026-09-12)

빈 목록을 기본값으로 삼던 결정을 바꿨다. 요청한 사람이 Windows 사용자이고, 그분은 메뉴를 한 번
열고 "끄기"를 못 찾았던 첫 비개발자 사용자다(`docs/requests.md` R3) — **고를 줄 알아야 켜지는 기능은 그분에게
없는 기능이다.** MS Office도 같이 넣는다(사용자: "이건 많이들 쓰니까").

| | Windows — exe 파일명 | macOS — bundle id |
|---|---|---|
| 한컴 | `Hwp.exe` · `HShow.exe` · `HCell.exe` | **없음 — 넣지 않는다 (아래)** |
| MS Office | `WINWORD.EXE` · `POWERPNT.EXE` · `EXCEL.EXE` | `com.microsoft.Word` · `com.microsoft.Powerpoint` · `com.microsoft.Excel` |

Windows 쪽 여섯은 이 기계에서 경로까지 확인했다 (2026-09-12) — 한컴오피스 2024
(`...\Hnc\Office 2024\HOffice130\Bin\`), MS는 Office16 Click-to-Run
(`C:\Program Files\Microsoft Office\root\Office16\`).

**macOS 한컴 bundle id는 넣지 않았다.** `focus_activity.rs`의 `com.hancom.hwp` ·
`com.hancom.show`는 테스트 픽스처이지 확인된 값이 아니다. 틀린 이름은 "아무 일도 안 일어나는
것"이 되고 사용자는 그것을 고장으로 읽으므로, 확인되지 않은 id를 기본값으로 취급하지 않는다.
MS 셋만 안정된 값이라 넣었다.

#### 빈 목록을 만들 수 있어야 한다 — 기본값이 생기면서 새로 생기는 함정

예전 양쪽 셸은 **목록이 비면 키를 지웠다**(`toggleWorkApp`의 `removeObject`, `Settings::clear`).
기본값이 빈 목록일 때는 그게 맞았다. **기본값이 채워지면 그 코드는 "전부 껐다"를 "기본값으로
되돌려라"로 바꿔 버린다** — 사용자가 여섯 개를 하나씩 끄고 나면 다음 실행에 여섯 개가 전부
돌아온다. 끌 방법이 없는 기능이 되는 것이고, 이건 `CLAUDE.md`의 E 항목(저장된 설정이 옛
기본값을 얼린다)과 정확히 거울상이다.

**"한 번도 안 정함"과 "비우기로 정함"을 갈라야 한다.** 목록이 비면 키를 **지우지 말고 빈 값으로
저장한다.** 키가 없을 때만 기본값이 답하고, 빈 문자열은 빈 목록으로 읽는다. 양쪽 셸 모두.

#### 이것은 기존 결정을 뒤집었다

`CLAUDE.md`의 G 규칙 넷째와 `docs/behavior-flow.md` §5b 끝 문단은 "기본값은 빈 목록"이라고
적혀 있었다. 구현과 함께 두 문장도 새 기본값과 명시적인 빈 목록 저장 규칙으로 고쳤다.

빈 목록을 지키던 이유는 "잘못 짐작하면 never annoying을 어긴다"였다. **여기서는 짐작이 아니고**,
지정 앱이 앞에 와도 펫은 **옆에 앉기만 한다** — 점프는 그 세션의 첫 키 입력에만 나가고, 갸웃은
타이핑이 멈춰야 나온다. 켜져 있다는 것만으로 성가셔지는 구조가 아니고, 메뉴에서 하나씩 끌 수
있다(끌 수 있게 만드는 것이 바로 위 항목이다).

### 검증

2026-09-12 이 기계에서 `.\scripts\test.ps1`이 exit 0이었다. 공용 코어 38개와 모든 differential
fixture, Windows 셸 44개가 통과했고 네트워크 의존 테스트 1개만 기존대로 ignored였다. 앱 이름의
설정 저장 → 재시작 → 읽기와 쉼표가 든 라벨뿐 아니라, 지정 앱이 아닌 라벨과 파일명뿐인 라벨을
저장하지 않는 경우, 시작할 때 지정 앱이 아닌 기존 키를 디스크에서 지우는 경우까지 포함한다.
`RuntimeTrace.txt`는 바꾸지 않았다.

수정한 Debug 앱을 이 기계의 실제 사용자 컨텍스트에서 다시 켜고 `%APPDATA%\Roamling\settings.txt`를
직접 확인했다. 실행 전의 `roamling.workAppLabel.windowsterminal.exe=Windows Terminal Host`는
시작 직후 사라졌고, Windows Terminal이 포그라운드인 채 9초가 지난 뒤에도 `workAppLabel` 키는
다시 생기지 않았다. 이 기계에서는 Swift를 컴파일할 수 없으므로 macOS 앱 이름 저장·시작 정리
하네스를 포함한 Swift 변경은 소스까지만 고쳤고 컴파일은 미검증이다. W8은 실제 한컴·Office에서
흐름을 확인할 때까지 실사용 확인 대기로 둔다.

## 남은 리스크

W0가 둘을 없앴고(Swift GUI 상주앱 전례, `.lproj` 로컬라이제이션) 다중 디스플레이는 같은 날
닫혔다. 남은 것은 셋이다.

1. **COM interop** — UIA가 필요해지는 지점. `GetGUIThreadInfo`로 피할 수 있는 데까지 피한다.
2. **툴체인 환경이 macOS보다 무겁다.** vcvars64 + `SDKROOT`이 없으면 빌드가 깨진 툴체인처럼
   실패한다. CI와 기여자 문서에 그대로 비용이 된다.
3. **음수 좌표 배치가 미검증이다.** 보조 화면을 primary 왼쪽/위로 옮기면 1분이면 확인된다.
   혼합 DPI(1.5배·3.0배)는 2026-09-01에 통과했다.
