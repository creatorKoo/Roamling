[English](README.md) | **한국어**

# 🐾 Roamling

**데스크톱에 진짜로 사는 작은 친구.**

Roamling은 macOS와 Windows용 네이티브 동반자 런타임입니다. Petdex 호환 생물이
모니터 사이를 돌아다니고, 포인터를 피하고, 잡아서 끌 수 있으며, 앞으로는 코딩·게임·
미디어를 비롯한 데스크톱 생활 전반에 반응하게 됩니다.

> Cute first. Useful second. Never annoying.

펫이 무엇을 할지 정하는 코드는 양 플랫폼이 공유하는 Rust 코어 하나에 있고, 창·트레이·
입력만 각 플랫폼이 따로 가집니다. 자세한 내용은 `docs/windows.md`에 있습니다.

## 지금까지 된 것

- 네이티브 AppKit 메뉴 막대 앱과, 활성화되지 않는 투명 오버레이
- Codex/Petdex v1(8×9)·v2(8×11) 펫 로딩, 사용자 정의 애니메이션 정의, 그리고
  기능이 없을 때의 단계적 대체
- 직접 만든 내장 마스코트 **Mochi**와 **FatMochi** — idle·걷기·수면·잡힘·기지개·
  착지 애니메이션을 그려 넣었고 메뉴에서 고를 수 있으며, 코드로 그리는 비상용
  대체 그림도 있습니다
- 전역 데스크톱 좌표, 디스플레이 위상, 핫플러그 처리, 화면을 가로지르는 연속 경로
- 눈에 보이는 멈춤이 있는 차분한 배회, 짧아진 근거리 이동, 더 뚜렷해진 다중 화면 탐색
- 포인터 인식, 상한이 있는 회피, 빠른 접근을 감지한 잡기, 클릭·끌기·놓기, 모니터를
  넘는 끌기, 구석에 몰렸을 때 이어진 가장자리로 빠져나가기
- 이동·포인터·잡기·클릭 영역 값을 실시간으로 바꾸는 **동작 세부 설정** 창. 설정은
  저장되고 한 번에 기본값으로 되돌릴 수 있습니다
- 권한이 필요 없는 유휴 감지, 그리고 앉기·안전한 잠자리로 이동·수면·기상·기지개
- 각 디스플레이의 보이는 영역 안에 머무르며 포인터를 피하는 기본 배치
- 선택 설치하는 Claude Code 훅 연동 — 토큰으로 인증하는 로컬 수신기, 권한이 필요 없는
  대략적인 작업 창 배치, 그리고 시작·주의·완료·실패 반응
- 선택 설치하는 Codex 0.147+ 훅 연동 — 기존 훅과 `notify`를 보존하며, 여러 출처를
  묶는 주의 모델과 히스테리시스, 반응 정책을 공유합니다
- 잡기가 준비된 동안에만 입력 영역이 켜지는 스프라이트 크기 오버레이. 평소에는 아래
  앱을 그대로 두고 클릭이 통과합니다
- 기하·화면 경로·이동·포인터 상호작용·동작 전이·주의·반응·펫 애니메이션 대체에 대한
  순수 로직 테스트

## Windows

[최신 릴리스](https://github.com/creatorKoo/Roamling/releases/latest)에서
`Roamling-Setup.exe`를 받아 실행하세요. 사용자 단위로 `%LOCALAPPDATA%\Programs\Roamling`에
설치되며 **관리자 승인을 묻지 않습니다.** 같은 릴리스의 `roamling.exe`는 포터블 형태입니다 —
파일 하나, 따로 설치할 런타임 없음, Windows 자체 DLL 말고는 아무것도 필요 없음.

**설치 파일은 아직 코드 서명이 되어 있지 않습니다.** 그래서 처음 실행할 때 Windows가
*"Windows의 PC 보호"* 경고를 띄웁니다. **추가 정보 → 실행**을 누르세요. 서명은 할 일
목록에 있고, 그전까지는 이게 정직한 상태입니다. **업데이트에는 영향이 없습니다** —
Roamling은 설치 관리자를 다시 실행하지 않고 자기 실행 파일을 갈아끼우므로, 경고는 첫
설치 때 한 번만 나옵니다.

### 업데이트

Roamling은 켤 때와 하루에 한 번 새 버전을 확인하고, 배경에서 받아서 제자리에 놓습니다.
**대화상자도 재시작 요구도 없습니다** — 새 버전은 그냥 다음에 Roamling을 켤 때 실행되는
것이 됩니다. 기다리는 업데이트가 있으면 트레이 메뉴가 조용히 알려 줍니다.

모든 릴리스는 Ed25519 키로 서명되고 그 공개 반쪽이 앱 안에 들어 있습니다. 버전 피드와
실행 파일 **둘 다** 그 키로 검사한 뒤에야 무언가가 쓰입니다. 서명을 확인할 수 없는 빌드는
그냥 업데이트하지 않고 **거부합니다.** 전부 끄려면 트레이 메뉴의 **자동 업데이트**를
누르세요.

### Windows에서 빌드하기

여기에 Swift는 없습니다 — Windows 빌드는 전부 Rust입니다.
[Rust 툴체인](https://rustup.rs)이 필요하고, 설치 파일을 만들려면
[Inno Setup 6](https://jrsoftware.org/isinfo.php)도 필요합니다.

```powershell
.\scripts\test.ps1        # 코어·펫·에이전트·업데이트·셸 테스트
.\scripts\run.ps1         # 끄고 -> 빌드 -> 다시 켜기. 실행 중인 사본이 자기 exe를 잡고 있습니다
.\scripts\run.ps1 -Debug  # 같은 것의 debug 빌드. 콘솔에 상태 로그가 찍힙니다
```

## macOS에서 빌드하고 실행하기

macOS 13 이상과 Swift 6이 필요합니다. 명령줄 타깃 빌드는 Apple Command Line Tools로
되지만, 서명된 배포용 앱을 만들려면 결국 전체 Xcode가 필요합니다.

```sh
swift build
./scripts/test.sh
swift run Roamling
```

로컬 `.app` 번들 만들기:

```sh
./scripts/build-app.sh release
open build/Roamling.app
```

번들은 기본적으로 ad-hoc 서명되는데, 그러면 macOS가 다시 빌드할 때마다 다른 앱으로 보고
허용했던 권한을 잊어버립니다. 빌드 사이에 권한을 유지하려면 `scripts/signing.env.example`을
`scripts/signing.env`(git 미추적)로 복사하고 `ROAMLING_CODESIGN_IDENTITY`에 코드 서명
identity를 넣으세요. 무료 자체 서명 인증서면 충분하고, 만드는 방법은 예제 파일에 있습니다.
같은 변수를 환경변수로 지정해도 됩니다.

테스트는 의존성 없는 실행 파일 하니스를 쓰므로, 호환되는 XCTest 러너가 없는 최소
Command Line Tools 설치에서도 돕니다. 실패한 단위 케이스가 하나라도 있으면 non-zero로
끝납니다. 로컬 CLT의 컴파일러와 SDK가 어긋나면 두 스크립트 모두
`ROAMLING_SWIFT_SDK=/path/to/MacOSX.sdk`로 우회할 수 있습니다.

## 펫 패키지

Roamling은 다음 위치를 순서대로 찾습니다:

```text
$ROAMLING_PET_PATH
%APPDATA%\Roamling\Pets                              (Windows)
~/Library/Application Support/Roamling/Pets          (macOS)
~/.codex/pets
~/.petdex/pets
```

`ROAMLING_PET_PATH`는 패키지 디렉터리 하나를 가리켜도 되고 패키지들이 담긴 디렉터리를
가리켜도 됩니다. 패키지에는 `pet.json`과 거기서 참조하는 PNG 또는 WebP 아틀라스가
들어 있습니다. 찾은 Petdex 호환 패키지는 메뉴에서 고를 수 있고, 그 선택은 다시 켜도
유지됩니다.

## 에이전트 연동

Claude Code 연동은 **Roamling → Claude Code → 통합 설치…**에서 직접 설치하기 전까지
꺼져 있습니다. 설치해도 기존 `~/.claude/settings.json`의 값과 훅은 그대로 두고 백업을
한 번 만들며, 같은 메뉴에서 제거할 수 있습니다. 수신기는 `127.0.0.1`에서만 듣고 프롬프트
내용·도구 입출력·대화 기록·소스 코드를 저장하지 않습니다.

Codex 연동도 **Roamling → Codex → 통합 설치…**에서 선택적으로 설치합니다. Roamling의
핸들러만 `~/.codex/hooks.json`에 합치고 백업을 한 번 만들며, `~/.codex/config.toml`과
기존 `notify`, 형제 훅은 건드리지 않습니다. 설치 후 Codex를 재시작하고 새 훅 신뢰
요청을 승인하세요. Codex 수신기는 인증되는 별도의 `127.0.0.1` 포트를 쓰고 같은
무저장 원칙을 따릅니다.

## 저장소 안내

```text
Sources/RoamlingCore/   OS 비의존 기하·world·동작·이벤트
Sources/RoamlingPet/    Petdex/Codex 매니페스트, 아틀라스 런타임, 대체
Sources/RoamlingSources/ 활동 어댑터와 로컬 훅 전송
Sources/RoamlingMac/    AppKit 디스플레이·포인터·오버레이·앱 런타임
Sources/RoamlingApp/    실행 파일 진입점
rust/roamling-core/     펫의 결정. 양 플랫폼이 공유합니다
rust/roamling-agent/    Claude Code·Codex 훅, 정규화, 수신기
rust/roamling-pet/      시트 디코딩, 내장 마스코트, 펫 패키지
rust/roamling-update/   버전 피드 파싱과 릴리스 서명 검증
rust/roamling-win/      Windows 셸: 창·트레이·입력·틱 루프
installer/roamling.iss  Windows 설치 파일
Tests/                  순수 로직과 로더 테스트
docs/research.md        upstream/API 조사와 출처
docs/architecture.md    경계·결정·마일스톤 구조
docs/mvp.md             현재 MVP 게이트·범위·수용 기준
docs/windows.md         Windows 이식 실측·결정·게이트
```

Roamling은 독립 프로젝트이며 OpenAI, Anthropic, Petdex, 그리고 조사 노트에 언급된
비교 프로젝트들과 제휴 관계가 없습니다.

## 라이선스와 기여

Copyright (C) 2026 GooBeom Jeoung.

Roamling 소스 코드는 [GNU General Public License v3.0 only](LICENSE)로 배포됩니다.
그 라이선스 아래에서 사용·수정·배포·판매할 수 있으며, 파생 저작물은 같은 자유를
유지하고 대응 소스를 제공해야 합니다.

펫 패키지는 각 제작자의 라이선스를 따릅니다. 펫을 설치하거나 불러오는 것이 그 라이선스를
바꾸지는 않습니다. Roamling이라는 이름과 브랜딩은 소스 라이선스와 별개로 다루며,
[TRADEMARKS.md](TRADEMARKS.md)를 참고하세요.

기여자는 자기 작업의 소유권을 유지하지만
[기여자 라이선스 계약](CLA.md)에 동의해야 합니다. 이 계약은 받아들여진 모든 기여가 GPL
아래에서 계속 제공되게 하면서 공식 상업 배포도 가능하게 합니다.
[CONTRIBUTING.md](CONTRIBUTING.md)를 참고하세요.
