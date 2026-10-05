# 저장소는 둘이다 — 소스는 비공개, 배포는 공개

**지금 이렇다 (2026-10-05부터).** 어떻게 옮겼고 무엇을 재 봤는지는 `docs/requests.md` R29.

## 무엇이 어디에

| | 비공개 `creatorKoo/borissal` — 이 저장소 | 공개 `creatorKoo/Roamling` |
|---|---|---|
| 내용 | 소스 전체, 이력, 문서 | `README.md` · `README.ko.md` · `ARTWORK.md` · `TRADEMARKS.md` |
| 소스 | 전부 | 0.6.10까지만 — 태그 `v0.6.10` 하나. GPL로 이미 나간 것이다 |
| 릴리스 · 업데이트 피드 | 옛 릴리스 19개가 남아 있지만 밖에서는 못 받는다 | **여기에 올린다** |
| 워크플로 | Check macOS · Check Windows · Record runtime trace · Release | 없다 |
| secret | 넷 (아래 "발행") | 없다 |
| 이 PC의 작업 폴더 | `Roamling` | `Roamling-public` (옆 폴더) |

`borissal`은 2026-10-04까지 공개 `Roamling`이던 **바로 그 저장소**다 — 이름을 바꾸고 비공개로 돌렸다. 그래서 secret과 Actions
기록이 옮겨지지 않고 그대로 있다. 지금의 공개 `Roamling`은 그 이름을 넘겨받은 새 저장소다.

## 바꾸면 안 되는 것 셋

하나라도 달라지면 설치된 사본이 조용히 떨어져 나간다.

1. **피드 주소 — 곧 공개 저장소의 이름.** 두 셸에 컴파일돼 있다(`rust/roamling-win/src/update.rs` `FEED` · `FEED_SIGNATURE`,
   `Sources/RoamlingMac/MacUpdater.swift`의 두 URL). 공개 저장소의 이름을 바꾸거나 비공개로 돌리지 않는다. 비공개 저장소의
   릴리스 파일은 로그인 없이 받을 수 없다.
2. **업데이트 서명 키.** 앱이 든 것은 공개키뿐이다(`rust/roamling-update/src/lib.rs` `PUBLIC_KEY_HEX`). 비밀키는 이 저장소의
   secret `ROAMLING_UPDATE_SECRET_KEY`에만 있고 GitHub는 secret을 다시 보여 주지 않는다. 키가 달라지면 설치된 사본은 새 판을
   전부 거부한다(`release.yml` 머리 주석).
3. **맥 서명 인증서.** designated requirement가 인증서에 묶여 있다(`docs/release.md` "자체 서명 인증서"). 달라지면
   접근성·화면 기록 권한이 날아간다.

## 발행 — 태그는 여기에, 릴리스는 저기에

`v*` 태그를 **이 저장소에** 밀면 `.github/workflows/release.yml`이 돌고, 마지막 잡이 공개 저장소에 릴리스를 만든다.

- 어디로 올릴지는 워크플로의 `PUBLIC_REPOSITORY` 하나다. 서명되는 매니페스트 안의 내려받기 주소("Sign the release")와
  `gh release create --repo`("Publish")가 같이 그것을 쓴다.
- **secret 넷.** `ROAMLING_UPDATE_SECRET_KEY` · `MACOS_CERT_P12` · `MACOS_CERT_PASSWORD`는 그대로 있다.
  **`ROAMLING_PUBLIC_RELEASE_TOKEN`은 아직 없다** — 공개 저장소에 릴리스를 쓸 토큰이고, 워크플로의 기본 토큰은 자기 저장소
  밖에 닿지 않아서 따로 필요하다. 없거나 만료됐으면 맨 앞의 `preflight` 잡이 1분 안에 멈춘다. 읽기만 되는 토큰은 거기서
  못 가려내고 맨 끝에서 실패한다.
- **릴리스 노트는 손으로 쓴다.** `docs/release-notes/<버전>.md`가 있으면 그것을 올리고, 없으면 제목 한 줄이다. 공개 저장소에는
  우리 커밋이 없어서 자동 생성할 것이 없고, 그 몇 줄이 밖에서 읽을 수 있는 유일한 변경 기록이다.
- **공개 저장소의 태그는 소스가 아니다.** 릴리스에는 태그가 있어야 해서 그쪽 `main`에 붙는다(`--target main`).
- **리허설.** `gh workflow run release.yml -f version=<버전> -f prerelease=true`로 올리면 설치된 사본에게는 보이지 않는다 —
  피드가 `releases/latest`이고 GitHub의 latest는 prerelease를 건너뛴다. 파일과 서명을 받아 확인한 뒤
  `gh release edit v<버전> --repo creatorKoo/Roamling --prerelease=false --latest`로 올린다.
- **고친 워크플로는 아직 한 번도 돌지 않았다.** 토큰이 생긴 뒤 첫 닫힌 판의 리허설이 첫 실행이다. prerelease가 정말
  피드에서 빠지는지도 그때 `releases/latest/download/appcast.json`을 직접 받아 확인한다.

버전 세 곳이 태그와 같아야 한다는 것은 그대로다(`docs/windows.md` "릴리스할 때 사람이 지켜야 하는 것").

## 함정

- **공개 저장소를 이 작업 폴더의 원격으로 달지 않는다.** 브랜치든 태그든 한 번 잘못 밀면 닫은 소스가 통째로 공개되고
  되돌릴 수 없다. 발행은 워크플로가 API로 올리므로 여기서 공개 쪽에 `git push`할 일이 없다. 공개 저장소의 README를 고칠 때는
  옆 폴더 `Roamling-public`에서 한다.
- **옛 주소로 남은 작업 사본.** `origin`이 `…/creatorKoo/Roamling`인 사본은 이제 **공개 저장소를 가리킨다.** 맥의 사본이
  그렇다. 거기서 `git pull`은 이력이 달라 멈추지만 태그를 밀면 위의 사고다. 먼저
  `git remote set-url origin https://github.com/creatorKoo/borissal.git`.
- **CI가 분을 쓴다.** 공개일 때는 무료였다. 비공개는 Free 월 2,000분이고 결제 수단이 없으면 한도에서 멈춘다. 맥 검사 한 번이
  약 10분, 릴리스 한 번이 맥 7~9분 + Windows 13~14분이다(2026-10-04 실측). 맥 1분이 한도에서 몇 분으로 깎이는지는 GitHub
  문서에서 확인하지 못했다 — 분당 요금은 Linux의 약 10배다. 숫자와 출처는 R29.
- **토큰에는 만료일이 있다.** 만료되면 발행이 `preflight`에서 멈춘다. 만든 날과 만료일을 아래 "남은 일"에 적는다.
- **Android 앱의 `core/`는 아직 공개 주소를 가리킨다.** 그 앱이 고정한 커밋 `e37240e`는 태그 `v0.6.10`의 조상이라 공개
  저장소에서 받아진다(2026-10-05에 `git fetch --depth=1`로 확인). 0.6.10 뒤의 코어로 올리려면 `.gitmodules`의 주소를 이
  저장소로 바꾸고, `check-android.yml`의 체크아웃에 이 저장소를 읽을 열쇠를 줘야 한다 — 한 저장소의 기본 토큰은 다른 비공개
  저장소를 못 읽는다.

## 아직 GPL이라고 말하는 곳

소스는 닫혔지만 **닫힌 판은 아직 내지 않았다.** 첫 닫힌 판 전에 아래를 바꾼다. 문구는 사용자가 정한다(R29).

| 어디 | 지금 |
|---|---|
| 정보 창 | "GNU GPL v3.0 only 라이선스로 배포합니다" — `Localizable.strings` `alert.about.body` (en · ko) |
| 소스 링크 | 정보 창의 두 번째 버튼 "소스 보기" — `ShellPrompt.about`의 `menu.viewSource`, `ShellPrompt.sourceURL`, `rust/roamling-win/src/shell.rs` `SOURCE_URL` |
| Windows 설치기 | 설치 중 `LICENSE`(GPL 전문)를 보여 준다 — `installer/roamling.iss` `LicenseFile` |
| 패키지 메타 | `rust/Cargo.toml` `license = "GPL-3.0-only"` |
| 파일 머리 | SPDX `GPL-3.0-only` 219개 |
| 이 저장소의 글 | `LICENSE` · `README.md` · `README.ko.md` "라이선스와 기여" · `CLA.md` · `CONTRIBUTING.md` · `TRADEMARKS.md` |
| 공개 저장소의 글 | `README`의 "소스" 절은 이미 "0.6.10 뒤는 실행 파일로만"이라고 적는다. `TRADEMARKS.md`는 옛 글 그대로다 |
| 서드파티 고지 | **없다.** 지금까지는 공개된 소스가 그 자리였다. 닫힌 바이너리는 의존성의 고지를 실어야 한다 |

Android를 옮길 때의 선례가 있다 — 옮긴 파일의 SPDX 머리를 비공개용 표기로 바꿨다(R23 "비공개 저장소의 모양").

## 남은 일

| | 누가 | 무엇 |
|---|---|---|
| 1 | 사용자 | 공개 저장소에 릴리스를 쓸 토큰을 만들어 이 저장소의 secret `ROAMLING_PUBLIC_RELEASE_TOKEN`에 넣는다. fine-grained 토큰, 저장소는 `Roamling` 하나, 권한은 Contents 읽기·쓰기. 토큰이 대화 기록에 남지 않게 직접 넣는다 |
| 2 | 사용자 | 닫힌 판의 라이선스 문구 — 정보 창 한 줄, 설치기가 보여 줄 글, `LICENSE`에 둘 글 |
| 3 | 사용자 | CI 한도를 어떻게 넘길지 — Pro(월 3,000분) · 결제 수단과 상한 · 맥에 직접 러너 두기 |
| 4 | 사용자 | 맥의 작업 사본에서 `origin`을 이 저장소로 바꾼다 (위 "함정") |
| 5 | Claude | 2가 정해지면 위 표의 문구를 바꾸고 서드파티 고지를 만든다 |
| 6 | Claude | Android 저장소의 `core/` 주소와 체크아웃 열쇠 — 그 앱이 0.6.10 뒤의 코어를 필요로 할 때 |
| 7 | Claude | 첫 닫힌 판 — 리허설(prerelease) → 확인 → latest |
