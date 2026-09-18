# 문서·유지보수 점검 — 2026-09-18

사용자가 직접 클릭 잡기 수정과 함께 문서 전체 최신화 및 리팩터링 후보 검토를 요청했다.
현재 작업 트리의 코드와 문서를 대조한 기록이며, 새 릴리스 발행이나 실기기 검증 기록은 아니다.

## 갱신한 설명

| 영역 | 현재 기준과 근거 |
|---|---|
| 잡기 | 몸체 위 클릭이면 접근 속도·이동·작업 상태와 무관하게 잡는다. `PetRuntime::finish_tick`, `pointer_down`, `touch_down`, `begin_catch`. 숨김·상호작용 끔·중복 접촉과 클릭 반응 재생 중 예외는 `behavior-flow.md` 4절 |
| 일반 꼬리 흔들기 | 멈춘 `Idle`·`Sit`·`LookAtPointer`에서만 허용하며 작업 반응을 덮지 않는다. `finish_tick::allows_pointer_glance` |
| 모니터 경계 | 정착 후보는 공유 가장자리를 비우고, 통과는 이웃 화면 안쪽까지 이어진다. `display_policy::placement_frame`, `crossing` |
| 내용 회피 | 배회·작업·휴식이 `ClearanceMap`의 내용 제외·거리 구간을 공유한다. 수면 도착 후와 새 필드도 검사한다. OCR·화면 전체 최적화는 아니다 |
| 구현 언어 | 결정 로직은 Rust, Swift Core는 대조군. macOS는 UniFFI, Windows는 직접 링크, Android는 UniFFI/JNA. `CLAUDE.md`, `RustCore.swift`, `roamling-android/src/lib.rs` |
| 빌드·서명 | Swift 앞에 Rust/바인딩 빌드가 필요하고 macOS 번들 빌드는 기본적으로 서명 identity를 요구한다. `build-rust-core.sh`, `build-app.sh` |
| Android | A0/A2 당시 설명을 현재 A3 서비스 상태와 구분했다. 마우스도 직접 접촉을 쓰지만 Android의 터치 경로는 유지한다. `PreviewRuntime`, `CompanionService` |
| 팔레트 | Rust 마스코트 조립은 이미 존재한다. macOS는 Swift 조립과 시트 팔레트 FFI 경계를 유지한다. `roamling-pet::built_in_mochi`, `MascotPetFactory` |
| 비용 | 이전 실측은 그 날짜의 값이다. 새 clearance 정책의 성능을 재측정한 것으로 표현하지 않는다. `battery.md` |
| 첫 사용·업데이트 안내 | 기본 사용법과 미확인 변경을 독립 안내 revision으로 선택한다. 네이티브 창·메뉴 다시 보기·릴리스 작성 절차는 `usage-guide.md` |

영문·한글 README, 기여 안내, 문서 지도와 위 영역의 현재 안내를 수정했다. 추적된 Markdown 전체를
대상으로 오래된 상태 표현과 로컬 링크를 검색했다. 법률·라이선스 문구, 자산 프롬프트와
`history/`의 과거 실측은 변경하지 않았다. 문서 전체의 모든 외부 정보나 플랫폼 동작을 새로
실측한 것은 아니며, 원격 링크의 가용성도 이번 검사 범위가 아니다.

## 리팩터링 추천 순서

| 우선순위 | 추천 | 근거·범위 |
|---|---|---|
| 1 | 접근 반응과 직접 클릭의 이름 분리 | `catch_armed_until`, `CatchArmDistance`, `CatchApproachSpeed`, `CatchWindow`가 이제 클릭 허용 조건이 아니라 접근 반응·틱 속도를 뜻한다. 내부 이름·설정 표시 설명부터 정리하되 저장 키와 공개 FFI는 호환을 유지한다. 실제 잡기 경로는 이번에 `touch_down`과 공유했다 |
| 2 | 휘도 갱신과 평가 캐시의 수명 정리 | macOS `RoamlingRuntime.tick`은 `core.setLuminance`를 매 tick 호출하고 `PetRuntime::set_luminance`는 수면 검사 캐시를 매번 비운다. Windows `refresh_luminance`는 새 캡처 때만 전달한다. 동일 필드 재전달과 새 캡처를 구분하고, 이후 같은 평가에서 반복 생성되는 `ClearanceMap`을 공유하는 순서가 적절하다. 성능 이득은 측정 후 판단한다 |
| 3 | 포팅 대조 계약과 현재 배치 정책을 명시적으로 구분 | `PlacementDirector::new`와 `for_runtime`, 플래너의 `destination`과 `clear_destination`이 공존한다. 현재 정책 선택을 명시하는 내부 타입과 테스트 구성이 호출 실수를 줄인다. 기존 fixture를 바꾸거나 두 정책을 무작정 합치지 않는다 |
| 4 | `PetRuntime`의 입력·휴식·경계 이동 코드를 기능별 파일로 분리 | 한 파일에 상태 조정과 회귀 테스트가 함께 늘었다. 먼저 테스트 모듈을 분리하고, 상태 소유자는 하나로 유지한 채 내부 helper를 추출한다. 동작·타이밍 변경과 함께 진행하지 않는다 |

큰 구조 변경은 이번에 실행하지 않았다. 우선 1번은 용어 혼동을 줄이는 작은 정리이고,
2번은 코드로 확인된 중복 계산 경로다. 3·4번은 사용자 체감 확인 후 별도 변경으로 진행하는 편이 낫다.

## 검증 중 보완한 것

`roamling-agent/tests/windows_hook_shells.rs::check_shell`이 Git Bash 요청을 읽다가 `WouldBlock`로
실패했고 재실행은 통과했지만, 최종 빌드 뒤 PowerShell에서도 같은 오류가 재현됐다. listener는
nonblocking이고 받은 stream의 `read`는 바로 unwrap했다. 수신 stream에 `set_nonblocking(false)`를
명시해 기존 3초 제한으로 데이터를 기다리게 보완했다. 요청 내용·파일 생성·오류 출력 검사와 제품
훅 명령은 그대로다. 테스트 수신기의 경합이며 제품 훅 실패가 확인된 것은 아니다.
보완 뒤 두 셸 테스트를 10회 연속 실행해 모두 통과했고 최종 `scripts/test.ps1`도 통과했다.

0.6.5 발행 전 Windows CI에서는 PowerShell 첫 실행을 포함한 테스트가 약 24초 걸려 수신 대기
10초를 넘겼다. 셸 시작을 포함한 최초 연결 대기만 60초로 늘렸고, 연결 후 요청 읽기 3초와
제품 훅 명령·payload·출력·파일 검사 조건은 유지했다. 검증 러너의 시작 지연과 요청 지연을 구분한다.

이전 잠자리 수정에서 빠진 무캡처 진단 문구 `tucking into a safe zone, spot unvetted`도
복구했다. `RuntimeLogicTests.swift`가 이 문구로 무권한 휴식 경로를 확인한다. 기대값을 바꾸는
대신 기존 진단 계약을 복구했으며, Swift 테스트를 Windows에서 실행한 것은 아니다.

문서 검사: Markdown 51개에서 로컬 링크 49개를 검사해 누락 0개. 코드·문서·스크립트의
docs 디렉터리의 Markdown 참조 27개도 누락 0개. README에 남아 있던 이동 전 `mvp`·`research` 경로를
현재 `history/` 경로로 고쳤다. fixture와 `RuntimeTrace.txt`는 변경하지 않았다.

최종 Windows 검증: 코어 단위 75개, 기존 differential 비교, 셸 51개 통과(기존 네트워크
테스트 1개 제외). 릴리스 빌드 후 저장소의 `rust/target/release/roamling.exe`를 재실행해
정상 응답을 확인했다. 실제 마우스 사용감은 사용자 확인 대상이다.

macOS에서는 실제 클릭 전달, 다중 모니터 경계의 깜박임, Swift 하네스·녹화 트레이스 대조가
남아 있다. Windows 공통 코어 테스트만으로 이 항목들을 통과 처리하지 않는다.

이후 R13 사용 안내를 추가한 최종 검사에서는 Windows 셸 53개가 통과했다(네트워크 1개 제외).
실행 파일 증가량과 실제 안내 창의 컨트롤 확인, macOS 미검증 범위는 `usage-guide.md`에 기록했다.

0.6.5 macOS 검사에서 기존 수면 검사 입력의 결함도 확인했다. 처음 한 번만 커서를 펫 위치에
두어 펫이 회피하면 커서가 멀어졌는데, 계속 펫 위에 있는 입력이라고 가정했다.
`RuntimeLogicTests`의 매 tick 입력이 현재 펫 위치를 따라가도록 고쳤으며 수면 금지 assertion은
유지했다. 동일 조건을 Rust 코어 600 tick 회귀 검사로도 확인한다.

기존 전체 녹화는 이전 꼬리 반응을 포함하므로 새 동작과 달랐다. 사용자의 별도 승인에 따라
수동 `record-runtime-trace.yml`에서 검토용 녹화를 산출하고, 원본 보존·차이 검토 후에만
0.6.5 기준을 갱신한다. 일반 검증·릴리스 워크플로는 계속 녹화를 비교하며 자동 재생성하지 않는다.
원본 해시·산출 근거·전체 구간별 차이 검토는 [0.6.5 녹화 검토](runtime-trace-review-0.6.5.md)에 있다.

최종 [0.6.5 릴리스 검증](https://github.com/creatorKoo/Roamling/actions/runs/35296443724)은
Windows·macOS 모두 통과했다. Swift 197개·새 녹화 비교, Rust 코어 76개·기존 differential,
Windows 셸 53개(네트워크 1개 제외), Mac 서명·패키지 실행과 발행을 확인했다.
공개 파일 6개의 해시와 피드·업데이트 파일 서명도 검증했다. 승인 전 원본 녹화는 보존했다.

## 리팩터 1 실행 — 접근 반응과 잡기의 이름 분리 (2026-09-18)

사용자가 "1, 3으로 하고"로 착수를 정했다. 코드를 바꾸기 전에 지금 흐름을 코드 위치와 함께 적는다.

### 지금 "catch arm"이 실제로 하는 일

1. **판정** — `rust/roamling-core/src/pointer.rs` `PointerInteractionModel::evaluate`: 커서가
   `catch_distance` 안이고 속도가 `catch_pointer_speed` 이상, 접근(closing) 속도가
   `catch_closing_speed` 이상이면 `PointerProximity::Catchable`.
2. **값의 출처** — `tuning.rs` `RuntimeTuning::pointer_configuration`: `catch_arm_distance`가
   `catch_distance`로, `catch_approach_speed`가 `catch_pointer_speed`로, 그 0.48배(최소 120)가
   closing 속도로 들어간다.
3. **유지 시간** — `pet_runtime.rs` `finish_tick`: `decision.should_arm_catch()`이고 상호작용이
   켜져 있고 숨김이 아니고 클릭 반응 대기가 아니면 `catch_armed_until = max(기존, now + catch_window)`.
   `catch_is_armed`는 같은 게이트에 `now <= catch_armed_until`.
4. **켜졌을 때 일어나는 것** — 셋뿐이다.
   - `preferred_tick_interval`: 1/60초로 틱 (셸이 매 프레임 부른다).
   - `make_situation`의 `is_pointer_owned`: 배치가 펫을 옮기지 않는다.
   - `finish_tick`의 분기(`catch_is_armed && crossing_clear.is_none() && allows_pointer_glance`):
     경로 취소, `behavior.handle(Pointer(Catchable))` → `behavior.rs`에서 `LookAtPointer`,
     배회를 1초 미룬다. 앉아 있을 때만이다(`allows_pointer_glance`).
5. **하지 않는 것** — 클릭 허용. R11(2026-09-18) 뒤로 `pointer_down`은 `touch_down`과 같은 몸체
   접촉 검증만 쓰고, `TickOutput.interaction_enabled`는 이 값을 보지 않는다
   (`pet_runtime.rs` 테스트 "the shell must receive the unarmed click").
6. **꺼지는 곳** — `set_interactions_enabled`, `set_hidden`, 잡기 시작, 숨김 해제 뒤 리셋.

즉 이 세 값은 **"빠르게 다가오는 커서를 앉은 펫이 알아채고 잠깐 쳐다보는" 반응**의 반경·속도·
유지 시간이다. 이름의 "catch"는 R11 이전의 뜻이다.

### 바꾸는 이름

| 지금 | 바꾼 뒤 | 어디 |
|---|---|---|
| `RuntimeTuningKey::CatchArmDistance` | `ApproachDistance` | `tuning.rs`, `roamling-win/tuning.rs`, `tests/tuning_differential.rs` |
| `RuntimeTuningKey::CatchApproachSpeed` | `ApproachSpeed` | 같은 곳 |
| `RuntimeTuningKey::CatchWindow` | `ApproachHold` | 같은 곳 |
| `RuntimeTuning.catch_arm_distance` 등 세 필드 | `approach_distance` · `approach_speed` · `approach_hold` | `tuning.rs`, `ffi.rs`의 변환 본문 |
| `PetRuntime.catch_armed_until` | `approach_hold_until` | `pet_runtime.rs` |
| `catch_is_armed` (지역 변수·인자) | `approach_held` | `pet_runtime.rs` |
| `PointerDecision::should_arm_catch` | `is_fast_approach` | `pointer.rs`, `pet_runtime.rs`, `tests/mechanics_differential.rs` |

**그대로 두는 이름과 이유.**

- `FfiTuning`의 필드와 `normalize_tuning`의 인자 이름(`catch_arm_distance` …) — uniffi가 Swift 인자
  라벨로 내보내는 공개 FFI. Swift는 이 라벨로 부른다.
- `PointerInteractionConfiguration.catch_distance` 등과 `PointerProximity::Catchable` — 같은 이유로
  FFI(`ffi.rs`의 pointer 함수들)이고, `Catchable`은 differential fixture와 behavior 입력에도 박혀 있다.
  "잡을 수 있는 거리·속도로 다가온 커서"라는 판정 이름으로는 여전히 맞다.
- Swift `RuntimeTuningKey.catchArmDistance` 등 — `Codable` 키가 그대로 macOS `UserDefaults` blob의
  JSON 키다. 이번엔 Swift를 만지지 않는다(맥 없이 컴파일 확인이 CI뿐이다).
- 문자열 키 `tuning.catchArm` · `tuning.catchSpeed` · `tuning.catchWindow` — Swift 패널이 같은 키로
  읽는다. **값만** 바꾼다.

### 저장 키 — 발견한 것과 결정

Windows는 `roamling-win/src/main.rs` `tuning_key`가 **`format!("{key:?}")`로 enum 변형 이름에서**
`roamling.runtimeTuning.catchArmDistance`를 만든다("표가 아니라 유도"). 변형 이름을 바꾸면 사용자가
튠한 값이 조용히 기본값으로 돌아간다. 이것이 리팩터 검토가 "저장 키 호환 유지"라고 적은 그 지점이다.

결정: `tuning.rs`에 `RuntimeTuningKey::storage_name()`을 두고 열한 개의 **기존 camelCase 이름을
표로** 적는다. Windows `tuning_key`는 그것을 쓴다. 코어 테스트가 열한 이름 전부를 macOS blob 키와
같은 문자열로 고정한다. "유도라 안 어긋난다"를 "표지만 테스트가 고정한다"로 바꾸는 것이고, 저장된
이름과 코드 이름이 **의도적으로 다른** 순간부터는 표가 정직한 쪽이다.

### 사용자에게 보이는 문구

패널의 세 슬라이더와 안내문이 아직 클릭 조건을 설명하고 있어 R11 이후 거짓이다. 키는 그대로,
값만 바꾼다.

| 키 | en | ko |
|---|---|---|
| `tuning.section.pointer` | Pointer | 포인터 |
| `tuning.catchArm` | Approach distance | 접근 반경 |
| `tuning.catchSpeed` | Approach speed | 접근 속도 |
| `tuning.catchWindow` | Approach hold | 접근 반응 시간 |
| `tuning.pointerNote` | 접근 반응 설명 + "잡기는 몸체 클릭, 이 값과 무관" | 같은 뜻 |

리팩터 게이트라 **동작·기본값·범위는 바꾸지 않는다.** `tuning_differential` fixture와 `RuntimeTrace`가
그대로 통과해야 한다. `docs/architecture.md`와 `docs/placement.md`의 `catchArmedUntil`·"catch arm"
표현도 같은 작업에서 고친다.
