# 릴리스 — 서명, 패키징, 배포 (macOS)

`CLAUDE.md`에는 규칙만 있고 절차는 여기 있다. **dmg 배치나 서명을 손대기 전에 이 문서를
읽는다** — dmg에서 원인을 세 번 잘못 짚었고, `.p12` 형식을 "개선"했다가 릴리스가 한 번 멈췄다.

Windows 쪽(설치 경로 · 업데이트 피드 · 릴리스가 맞춰야 하는 세 버전)은 `docs/windows.md`.

## 빌드와 서명

```sh
./scripts/build-app.sh release   # build/Roamling.app
```

`build-app.sh`는 시작할 때 git-ignore된 `scripts/signing.env`가 있으면 source한다. 거기에
`ROAMLING_CODESIGN_IDENTITY`를 넣어 두면 매번 환경변수를 지정하지 않아도 된다. 설정 방법은
저장소의 `scripts/signing.env.example`에 있다. **identity 이름은 머신의 keychain에
종속되므로 script에 하드코딩하지 않는다** — 기여자의 빌드가 그 이름을 찾지 못해 실패한다.

**`build-app.sh`는 identity 없이는 빌드를 거부한다**(`ROAMLING_ALLOW_ADHOC=1`로만 우회).
ad-hoc이면 designated requirement가 cdhash로 고정돼 빌드할 때마다 macOS가 다른 앱으로 보고
Accessibility 권한이 사라진다.

## 자체 서명 인증서 (2026-09-04)

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

## dmg 창은 커밋된 두 파일이다

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
