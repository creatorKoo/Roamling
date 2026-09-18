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
