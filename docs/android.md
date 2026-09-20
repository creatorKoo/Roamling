# Android — 비공개 저장소로 옮겼다 (2026-09-20)

**Android 앱은 이 저장소에 없다.** 2026-09-20에 비공개 저장소로 옮겼고, 그 앱은 공개하지 않는다
(사용자 결정, `docs/requests.md` R23). 이 파일은 그 사실과, **이 공개 저장소에서 무엇이 Android에 닿는지**를
적는 자리로 남긴다 — 다른 문서의 옛 링크도 여기로 온다.

## 옮겨 간 것과 남은 것

- 옮겨 갔다: `android/`(Gradle 프로젝트), `rust/roamling-android/`(UniFFI 진입점 크레이트),
  `scripts/build-android-core.*` · `scripts/test-android*.ps1`, `check-android` 워크플로, 그리고 이 파일에
  있던 설계·게이트·실측 기록 전부.
- 옮기기 전의 모습은 이 저장소의 이력에 그대로 있다 — 마지막은 `a61fccd`(v0.6.8)이다. 이력은 다시 쓰지
  않았고, 거기까지 공개된 것은 공개된 조건 그대로 남는다.
- **남았다:** 코어 그 자체. Android 앱은 이 저장소를 서브모듈로 고정해 `rust/roamling-core`와
  `rust/roamling-pet`을 그대로 빌드한다. 그래서 아래 둘은 Android 전용처럼 보여도 코어의 일부라 여기 있다.
  - `rust/roamling-core/uniffi.toml` — Kotlin 바인딩 설정(`android = true`, `Pointer` 이름 바꿈).
  - `rust/.cargo/config.toml`의 `*-linux-android` 링커 플래그(16 KB 페이지).

## 이 저장소에서 일할 때 지킬 것

- **FFI 표면(`rust/roamling-core/src/ffi/`)은 Android와의 계약이다.** Swift만 보고 이름·인자·레코드를 바꾸면
  Android 쪽 Kotlin 바인딩이 같이 바뀐다. 이 저장소의 CI는 더 이상 그것을 잡지 못한다 — Check Android가
  같이 옮겨 갔다. FFI를 바꾼 릴리스는 Android 저장소에서 코어를 올려 빌드해 봐야 끝난 것이다.
- `PetLoop`가 셸에 요구하는 입력(`FfiTickInput`)과 그 뜻을 바꾸는 것도 같다. Android 셸은 포인터를 손가락이
  닿아 있을 때만 주고, 캡처·포커스 권한은 늘 false로 준다.
- 내장 보리 그림(`Sources/RoamlingPet/Resources/BuiltInPets/`)은 Android 앱에도 그대로 들어간다.
  그림의 조건은 `ARTWORK.md`.
- Android에 대한 요청은 이제 비공개 저장소의 문서에 적는다. 분리 전의 기록(R6 · R17 · R18 · R23)은
  `docs/requests.md`에 남아 있다.
