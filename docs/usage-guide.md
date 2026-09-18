# 사용 안내 — 첫 실행과 주요 변경

R13에서 승인한 데스크톱 안내다. Windows·macOS 네이티브 modeless 창으로 표시하며 새 의존성,
이미지, 웹뷰, 네트워크 요청을 추가하지 않는다. Android 안내는 이번 범위가 아니다.

## 흐름

`Sources/RoamlingShell/Resources/UsageGuide.txt`가 안내 버전·기본 항목·변경 항목의 공통 정본이다.
문구는 기존 한영 `Localizable.strings`의 `<항목>.title`·`<항목>.body`로 관리한다.
셸의 `UsageGuide`/`usage_guide`가 이를 읽고 저장된 `roamling.guideSeenRevision`과 비교한다.

- 값이 없거나 0: 현재 기본 사용법을 한 화면에 표시한다. 기존 설치에도 처음 도입 시 한 번 뜬다.
- 저장값보다 최신 안내 버전이 큼: 저장값 뒤의 변경 항목만 합친다. 여러 업데이트를 건너뛰어도 한 창이다.
- 같거나 더 큼: 자동 표시하지 않는다. 다운그레이드로 이미 본 버전을 되돌리지 않는다.
- 메뉴의 ‘보리 사용법…’: 버전과 무관하게 현재 기본 사용법을 다시 연다. 중복 창은 만들지 않는다.
- 확인 버튼 또는 창 닫기: 표시한 버전을 저장한다. 창 생성 실패·프로세스 종료는 확인으로 기록하지 않는다.
- `ROAMLING_SMOKE_TEST=1`: 자동 안내와 읽음 기록을 생략한다.

Windows `main`/`tick`은 창에서 완료한 버전을 받아 기존 Settings 인스턴스로 저장한다.
macOS AppDelegate는 안내 모델을 `UsageGuideWindowController`에 넘기고 닫기 콜백에서 UserDefaults에 저장한다.
자동 표시는 작업을 막거나 기존 창을 비활성화하지 않는다. 수동 다시 보기는 창을 앞으로 가져온다.
Ctrl/Command로 회피를 멈추고 잡는 방법을 첫 항목으로 설명한다. 앱 설정·권한을 자동으로 켜지 않는다.

## 릴리스할 때

1. 사용자에게 알려야 할 조작·기능 변화인지 판단한다. 오류·성능 수정만이면 안내 버전을 유지한다.
2. 필요할 때만 `UsageGuide.txt`의 `revision`을 증가시키고 그 번호의 변경 항목을 추가한다.
3. 새 항목의 한영 title/body를 같이 추가한다. 과거 변경 항목은 버전 건너뛰기를 위해 보존한다.
4. 기본 항목은 현재 사용법으로 유지한다. 새 안내 문구를 과거 버전 키에 덮어써 재표시를 기대하지 않는다.
5. 신규·이미 확인함·여러 버전 건너뛰기·다운그레이드·메뉴 다시 보기·닫기 기록을 검증한다.

앱 버전·태그·업데이트 다운로드와 안내 버전은 독립적이다. 업데이트된 앱을 실행할 때만 안내를 판단한다.

## 검증 (2026-09-18)

- `scripts/test.ps1` 통과: 공통 코어 단위 75개·기존 differential 비교, Windows 셸 53개
  (네트워크 검사 1개 제외), release 빌드. 기존 fixture·RuntimeTrace는 변경하지 않았다.
- `usage_guide::tests`: 최초·변경·여러 버전 건너뛰기·수동·다운그레이드, 네이티브 창 생성,
  본문과 버튼의 축소 시 배치, X·버튼 닫기 후 재실행용 저장, 종료 시 미기록을 검사했다.
- 메뉴 도달성·종료 마지막 위치와 한영 문자열 키 일치 검사 통과.
- 최종 release를 재실행해 정상 응답과 `RoamlingUsageGuide` 창의 실제 표시 상태를 확인했다.
  실행 중 컨트롤에서 한글 제목·본문 4개 항목·‘시작하기’ 버튼을 읽었으며, 닫기 전에는 읽음 키가
  없었다. 스크린샷·사용자 체감 확인을 대신한 것은 아니다. 안내 창은 사용자가 확인하도록 열어 두었다.
- 안내 추가 전 3,235,840 bytes → 추가 후 3,256,832 bytes. 증가 20,992 bytes = 20.5 KiB,
  약 0.65%. Windows release exe 비교이며 macOS 번들 증가량을 측정한 것은 아니다.
- Swift 모델·메뉴 검사도 추가했지만 이 Windows 기계에서는 Swift/AppKit을 컴파일하지 않았다.
  macOS 서명 빌드와 창 표시·재실행·포커스 실물 확인은 별도로 남아 있다.

## 0.6.5 발행 검증

사용자가 Windows 안내를 확인하고 2026-09-18 발행을 승인했다.
[Windows 전체 검사](https://github.com/creatorKoo/Roamling/actions/runs/35294304103)는
테스트·패키징·실행·DLL 의존성까지 통과했다.
[macOS 검사](https://github.com/creatorKoo/Roamling/actions/runs/35295558933)는 Swift 197개와
승인된 새 녹화 비교를 통과했지만, 앱 빌드에서 안내 창의 `greatestFiniteMagnitude` 타입 추론이
모호해 중단됐다. `CGFloat`를 명시한 뒤
[최종 릴리스 실행](https://github.com/creatorKoo/Roamling/actions/runs/35296443724)에서
양 플랫폼 테스트·빌드·Mac 서명·패키징·패키지 실행·공개 단계가 모두 통과했다.
공개 ZIP에 안내 revision 1과 앱·빌드 버전 0.6.5가 포함된 것도 확인했다.
공개 파일 6개의 SHA-256, appcast와 Windows exe·Mac ZIP의 Ed25519 서명을 별도로 검증했다.
[v0.6.5](https://github.com/creatorKoo/Roamling/releases/tag/v0.6.5)는 정식 최신 릴리스다.
실제 Mac의 팝업·포커스 사용감은 CI 실행과 별개다.
녹화 보존과 변경 근거는 [녹화 검토](runtime-trace-review-0.6.5.md)에 있다.
