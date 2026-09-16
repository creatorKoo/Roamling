# Android 첫 프로젝트 방향

**상태: 설계 초안 (2026-09-14) · 구현 시작 전.** 첫 사용 대상은 아내분의 Android
휴대전화다. 데스크톱판의 모든 기능을 옮기기보다, 먼저 모치가 휴대전화 안에서 살아 있는 것처럼
보이게 한다.

## 만들고 싶은 경험

```text
잠금 해제 중  다른 앱 위를 작은 모치가 걷고 쉰다
잠금          잠금화면 위젯에서 기다리거나 잠든다
다시 해제     마지막 자리에서 깨어나 다시 돌아다닌다
```

잠금화면 위젯을 제공하지 않는 기기에서는 알림에 조용한 상태만 보여 준다. 나중에 사용자가 직접
선택할 수 있는 라이브 배경화면을 더하면, 지원 기기에서는 잠금 중에도 시계와 알림 뒤에서 모치가
움직이는 선택지를 줄 수 있다. 일반 오버레이를 잠금화면 위에 억지로 띄우지는 않는다.

## 첫 판에 넣을 것

- 다른 앱 위에 표시하는 작은 투명 오버레이
- 승인된 Mochi 그림의 `idle`과 `walking`
- 화면 경계 안에서 걷기와 쉬기
- 모치를 손가락으로 잡아 옮기기
- 숨기기와 종료가 분명한 상시 알림
- 잠금과 해제를 감지해 오버레이를 멈추고 다시 시작하기
- 지원되는 기기의 잠금화면 위젯에는 쉬거나 자는 자세 표시

첫 실행에서 사용자가 Android의 **다른 앱 위에 표시** 권한을 직접 허용하게 한다. 모치가 보이는
동안만 foreground service를 쓰고, 알림에서 언제든 숨기거나 끝낼 수 있게 한다.

## 기존 프로젝트에서 가져올 것

행동과 이동을 Android용으로 다시 만들지 않는다. `rust/roamling-core`의 `PetLoop`를 Android용
라이브러리로 빌드하고 UniFFI Kotlin 바인딩으로 부른다. 그림도 기존 내장 Mochi atlas를 쓴다.
Android 쪽은 Kotlin으로 권한 화면, 오버레이, 터치, 알림과 잠금 상태만 맡는다.

첫 판에서는 Claude/Codex 연동, 다른 앱 내용 읽기, 화면 캡처, 접근성 권한, 여러 펫, 자동 업데이트를
넣지 않는다. 우선 실제 휴대전화에서 **걷고 · 잡히고 · 잠금 뒤 다시 깨어나는지**를 확인한다.

## iPhone은 나중에

Dynamic Island 지원 iPhone에서는 잠금 해제 중 작은 모치를 Live Activity로 보이고, 잠금 중에는
잠금화면 Live Activity로 이어 갈 수 있다. 상시 자리는 잠금화면 위젯이 맡는다. 다만 이것은
데스크톱이나 Android 오버레이처럼 계속 걷는 창이 아니며 Live Activity도 시간 제한이 있는
세션이다. 짧은 자세 변화와 상태 표현으로 생각한다. 일반 노치 기기는 Dynamic Island에 모치를
둘 수 없다.

## 붙이는 방식 — 세 번째 seam

**Android는 플랫폼 seam의 세 번째 구현이지 두 번째 제품이 아니다.** 펫이 무엇을 할지 정하는 코드는
전부 `rust/roamling-core`의 `PetLoop`(`src/ffi.rs:1602`)에 있고, Android 셸이 하는 일은 Windows
셸과 같다 — 기계를 읽어 `FfiTickInput`(`ffi.rs:1501-1513`)을 채우고 `FfiTickOutput`
(`ffi.rs:1516-1528`)대로 그린다. Windows에서 그 본체는 `rust/roamling-win/src/main.rs:532`의 `tick`
이고, 입력을 모으는 자리가 651-676이다.

갈리는 것은 붙는 방식뿐이다. macOS는 uniffi Swift 바인딩(`scripts/build-rust-core.sh:38-41`),
Windows는 rlib 직결(`rust/roamling-core/Cargo.toml:11`의 `crate-type`), **Android는 uniffi Kotlin**.

**코어를 다시 쓰지 않는다.** 재구현의 숨은 비용은 코드가 아니라 **검증**이다 — 걷기 속도 같은
튜닝값은 사용자가 눈으로 보고 닫은 것이라 다시 쓰면 그 눈을 다시 빌려야 한다. 그 판단의 기록이
`docs/history/windows.md` 3절이고, 언어를 바꾸자는 논의를 시작하기 전에 읽을 문서도 그것이다.

seam의 정본 표는 `docs/architecture.md:586-629`에 있고 지금은 macOS·Windows 두 열뿐이다.
**세 번째 열은 A0에서 더한다** — 코드가 생기기 전에 적으면 그 표가 그날부터 거짓이다.

## 무엇이 어디에 사나

```text
rust/roamling-android/   새 크레이트. cdylib 하나(libroamling_android.so).
                         roamling-core + roamling-pet에 의존하고 자기 uniffi scaffolding을
                         갖는다. core에 없는 둘만 노출한다
android/                 Gradle 프로젝트. 모듈 둘
  core/                  생성된 Kotlin 바인딩(미추적) + jniLibs/<abi>/*.so + JNA
  app/                   Kotlin 셸: 권한 화면, 오버레이 View, foreground service, 알림,
                         잠금 수신기, 틱 루프, 설정(SharedPreferences)
scripts/build-android-core.sh    build-rust-core.sh의 형제
scripts/build-android-core.ps1   같은 산출물을 내는 Windows 쪽 (개발 환경 절)
```

**Android(JNI/NDK) 타입은 `roamling-android`에 들어오지 않는다.** `platform.rs`가 Win32를
가두듯(`docs/windows.md:3-4`) OS는 Kotlin에 가둔다.

**왜 새 크레이트인가.** 의존 방향은 `roamling-pet` → `roamling-core`이고 그 반대가 아니다
(`rust/roamling-pet/Cargo.toml`의 `roamling-core` 의존). 마스코트 바이트와 애니메이션 플레이어를
Kotlin에 주려면 둘을 한 `.so`에 링크하는 상위 크레이트가 필요하고, 그 자리는 `roamling-win`이
Windows에서 맡은 자리와 같다. `roamling-pet`에 uniffi를 넣지 않는 이유는 Windows가 그 크레이트를
rlib으로 직접 링크하므로 FFI 짐을 얹을 이유가 없어서다.

**default-members에 넣는다**(`rust/Cargo.toml:8`). `roamling-android`는 OS 크레이트에 의존하지
않는 순수 Rust + uniffi라, 맨 `cargo test`가 두 호스트에서 그대로 빌드한다. `roamling-win`이
default set 밖이라 **태그 푸시 전까지 아무도 빌드하지 않던** 구멍
(`.github/workflows/check-windows.yml` 서문)을 이쪽에는 만들지 않는다. NDK 타겟 크로스 빌드만
스크립트와 CI가 한다.

**`roamling-agent`·`roamling-update`는 링크하지 않는다.** 첫 판에 agent 연동도 자동 업데이트도
없다("첫 판에 넣을 것"). 배포는 APK 설치(adb 또는 파일)로 대신한다.

## Kotlin이 채우는 것

런타임이 기계에 닿는 통로는 `PlatformServices` 하나이고(`Sources/RoamlingEngine/
PlatformServices.swift:54-71`), 자리는 열하나다(`docs/architecture.md:586-629`). Android는 그중
**넷(safeZone · focus · capture · window)을 첫 판에서 비워 둔다.**

| 자리 | Android 답 | 첫 판 |
|---|---|---|
| display | `WindowManager.currentWindowMetrics` 경계 − `WindowInsets`(status·navigation·cutout) → `FfiDisplay` 하나 | 채움 |
| displayChanges | `onConfigurationChanged`(회전·폴더블 접힘) → 디스플레이 다시 설정 | 채움 |
| safeZone | insets를 display 경계에서 이미 뺐으므로 빈 목록 | 빈값 |
| pointer | 오버레이 View의 `MotionEvent` 좌표(px→dp) | 채움 |
| userIdle | 마지막 터치 시각으로부터의 경과 | 채움 |
| focus | `focus_authorized=false`, `did_query_focus=false`, `queried_focus=null` | 스텁 |
| capture | `capture_authorized=false`, 휘도 없음 | 스텁 |
| window | 빈 목록 | 스텁 |
| overlay | `TYPE_APPLICATION_OVERLAY` + `FLAG_NOT_FOCUSABLE` + `FLAG_LAYOUT_NO_LIMITS` 창 하나를 `LayoutParams.x/y`로 옮긴다 | 채움 |
| images | `MascotAtlas`의 RGBA8 → `Bitmap`(ARGB_8888) | 채움 |
| coordinateSpace | px ÷ density. y 뒤집기 없음 | 채움 |

**틱 입력에는 자리가 아닌 것이 둘 더 있다.** 아래 둘은 `PlatformServices`의 슬롯이 아니라
`FfiTickInput`(`rust/roamling-core/src/ffi.rs:1501-1513`)의 필드다 — 셸이 매 틱 직접 채운다.

| 틱 입력 | Android 답 | 첫 판 |
|---|---|---|
| `primary_button_down` | `ACTION_DOWN`~`ACTION_UP` 사이 true | 채움 |
| `affection_held` | 항상 false. 휴대전화에는 모디파이어 키가 없다 | 상수 |

**1 world pt = 1 dp로 둔다.** Windows가 `world_scale()`(`rust/roamling-win/src/platform.rs:51`)에서
픽셀을 DPI 배율로 나눠 world 단위를 만들고, 그 나누기를 `rect_to_world`(`platform.rs:72`)와
`pointer`(`platform.rs:129`)가 똑같이 쓴다. Android의 density가 그 배율 자리에 그대로 들어간다.
**y를 뒤집지 않는다** — Win32도 Android도 좌상단 원점·y 아래라 core world와 방향이 같다.

**손가락이 없을 때의 포인터는 마지막 터치 자리가 아니라 화면 밖 좌표다.** 데스크톱 커서는 화면에
남아 있지만 손가락은 사라진다. 남겨 두면 펫이 있지도 않은 손을 계속 피한다.

**userIdle은 `lastTouchAt`에서 재고 상수로 주지 않는다.** `finish_tick`은
`user_idle_duration < 0.8`이면 쉬던 펫을 깨우므로(`rust/roamling-core/src/pet_runtime.rs:493`),
0을 주면 영영 못 자고 큰 값을 주면 영영 못 깬다. 오버레이 밖 터치는 `FLAG_WATCH_OUTSIDE_TOUCH`의
`ACTION_OUTSIDE`(좌표 없이 시각만)로 받고, 화면 켜짐·`USER_PRESENT`도 같은 시계를 리셋한다.
**내용은 보지 않고 시각만 본다** — macOS가 `CGEventSource`의 경과 시간 하나만 쓰는 것과 같은 선이다.

**`pointer_is_over_pet`은 View가 곧 hit region이다.** 오버레이 창이 펫 크기라 터치가 그 안에
들어왔는지가 곧 답이다. Windows가 펫의 반쪽 크기를 만들어(`main.rs:659-660`) 포인터와의 거리를
재는 계산(`main.rs:673-674`)이 이쪽에는 필요 없다.

**길게 누름을 애정으로 볼지는 A2 뒤에 사용자가 정한다.** 그때까지 `affection_held`는 상수 false다.

**전면 앱 식별(focus)과 화면 캡처는 첫 판에서 안 한다.** UsageStats·접근성·MediaProjection 권한이
필요하고, 첫 판의 제외 목록("기존 프로젝트에서 가져올 것")에 이미 들어 있다.

## roamling-android가 노출하는 것

`ffi.rs`에는 애니메이션이 없다 — `AnimationResolver`(`rust/roamling-core/src/animation.rs:297`)와
`PetAnimationPlayer`(`animation.rs:446`)는 uniffi로 나가 있지 않고, 내장 마스코트는 uniffi가 없는
`roamling-pet`에 있다. 새 크레이트가 감싸는 것은 그 둘뿐이다.

- **`MascotAtlas`** — `built_in_mochi()`(`rust/roamling-pet/src/lib.rs:211`)를 감싸 표준·확장 시트
  각각의 `width`/`height`/`rgba`, `frame_width`·`frame_height`(192×208, `lib.rs:112-113`),
  `columns`(8, `lib.rs:114`), `frame_rect(index)`(`lib.rs:75`)를 준다. 아틀라스는
  `include_bytes!`로 `.so` 안에 들어가므로(`lib.rs:122-125`) **APK에 에셋을 따로 넣지 않는다.**
  리컬러(`lib.rs:226`)는 첫 판 제외이고 나중 게이트다.
- **`Player`** — 위 둘과 `PetAsset.tracks`(`rust/roamling-pet/src/lib.rs:51-65`)를 감싸,
  Kotlin은 매 틱 capability와 delta를 주고 **프레임 번호만** 받는다. delta는
  `TickOutput.delta_time`에 `locomotion_rate`를 곱한 값이다
  (`rust/roamling-core/src/pet_runtime.rs:135-144`의 주석). **capability→track 해석은 Rust가 한다** —
  Kotlin이 트랙 이름을 알기 시작하면 `docs/state-contract.md`의 층 구조를 셸이 깰 수 있다.
- 둘 다 `PetLoop`(`ffi.rs:1602-1604`)과 같은 관용구다: `Mutex` 안의 상태를 uniffi 객체가 감싼다.

## 틱과 생명주기

```text
Service 시작    PetLoop.new(저장된 자리 또는 화면 중앙, 기본 FfiTuning, seed)   ffi.rs:1609
                set_displays(1619) / set_object_size(1638) / MascotAtlas → Bitmap
매 틱           begin_tick(now)(1735) → 포커스는 묻지 않음 → finish_tick(1739)
                → 창 위치 갱신, Player로 프레임 하나 → 다음 틱 예약
터치            pointer_down(1786) / pointer_dragged(1795) / pointer_up(1803)
자리 저장       persist_position이 참일 때 SharedPreferences에 x, y
SCREEN_OFF      set_hidden(true)(1667), 틱 중단, 창 제거
USER_PRESENT    set_hidden(false), 창 다시 붙임, 저장된 자리에서 재개(set_position 1678)
알림 [숨기기]   set_hidden(true) + 창 제거. 서비스는 유지
알림 [종료]     서비스 stop
```

- **틱 간격은 셸이 고르지 않는다.** `preferred_tick_interval`
  (`rust/roamling-core/src/pet_runtime.rs:298-316`)이 걷는 중 1/60, 잡힌 중 1/30,
  포인터를 보는 중 1/16(`pet_runtime.rs:312`), 자는 중 1/2, 그 외 1/12을 준다. 1/60일 때만 `Choreographer` 프레임 콜백, 그 외에는 `Handler.postDelayed`.
- **터치 뒤의 재예약을 빠뜨리지 않는다.** `InteractionOutput`의 `reschedule_after`
  (`rust/roamling-core/src/ffi.rs:1538`)가 차 있으면 그 시각에 한 번 더 틱한다.
- 시계는 `SystemClock.elapsedRealtime()`을 초로 바꾼 단조 시각이다. core는 f64 초만 받는다.
- foreground service는 펫이 보이는 동안만 쓴다("첫 판에 넣을 것"). Android 14+의
  `foregroundServiceType`은 `specialUse`로 선언하고 사유를 매니페스트에 적는다.
- 배터리 산술은 `docs/battery.md` 그대로이고, **데스크톱의 지배 항목이 여기엔 아예 없다** —
  62 ms짜리 화면 캡처(`docs/battery.md:11`)를 첫 판이 하지 않는다. 남는 것은 틱 캐던스뿐이고
  그것은 위 표대로 core가 정한다.

## 빌드·버전

```sh
./scripts/build-android-core.sh          # macOS
```

```powershell
.\scripts\build-android-core.ps1         # Windows
```

둘 다 같은 일을 한다 — `cargo ndk -t arm64-v8a -t x86_64 -o android/core/src/main/jniLibs build
--release -p roamling-android`로 ABI별 `.so`를 놓고, 이어서 같은 `uniffi-bindgen`
(`rust/roamling-core/src/bin/uniffi-bindgen.rs`)을 `--language kotlin`으로 부른다.

- **바인딩은 host 빌드의 라이브러리에서 뽑는다.** FFI 표면은 타겟과 무관하므로 두 호스트가 같은
  Kotlin을 낸다. `scripts/build-rust-core.sh:38-41`이 Swift 쪽에서 쓰는 방식과 같고, 이쪽은
  **한 `.so`에 두 크레이트의 scaffolding이 있을 때 `generate --library`가 둘 다 뽑는지**가 아직
  확인 전이다(아래 "확인 필요").
- 생성물 두 디렉터리(`android/core/src/main/kotlin`, `jniLibs`)는 **미추적**이다. Swift 쪽
  `Sources/RoamlingCoreRs`와 같은 규칙 — 빌드 산출물이므로 손으로 고치지 않는다.
- **링크 플래그는 스크립트가 아니라 `rust/.cargo/config.toml`에 둔다.** Android 15의 16 KB 페이지
  정렬(`-Wl,-z,max-page-size=16384`)은 `[target.aarch64-linux-android]`·
  `[target.x86_64-linux-android]` 섹션에 걸고, MSVC의 `+crt-static`이 그 파일에서 Windows 타겟에만
  걸려 있는 것(`rust/.cargo/config.toml:12-13`)과 같은 방식이다. 타겟 키가 격리라 세 호스트가 서로
  무관해지고, 두 스크립트가 같은 플래그를 따로 들고 다니지 않는다.
- Kotlin 바인딩은 JNA(`net.java.dev.jna:jna:<ver>@aar`)를 요구한다. `android/core/build.gradle`의 의존.
- Gradle wrapper는 커밋한다. `gradlew`는 LF여야 하므로 `.gitattributes`에 `gradlew text eol=lf`
  한 줄을 둔다 — Windows checkout이 CRLF로 바꾸면 다른 호스트에서 깨진다.
- CI는 `.github/workflows/check-android.yml` 하나를 **ubuntu-latest**에 둔다(NDK가 러너 이미지에
  있고 셋 중 제일 싸다). `cargo ndk` 빌드와 Gradle `assembleDebug`까지. 릴리스 워크플로에는 첫 판에서
  넣지 않는다. 로컬의 값싼 대응은 두 호스트 공통으로
  `cargo check -p roamling-android --target aarch64-linux-android`이고, 이것은 맥에서 Windows 셸을
  타입 검사하는 관용구(`docs/windows.md:35-37`)와 같다.
- **버전에 네 번째 자리를 만들지 않는다.** Gradle이 빌드할 때 `rust/Cargo.toml:11`의 `version`을
  읽어 `versionName`으로 쓰고 `versionCode`는 그것을 정수화한다. 태그와 맞춰야 하는 자리를 늘리지
  않는 것이 목적이다.

## 개발 환경 — 두 호스트

**일상 개발은 Android 에뮬레이터를 기본으로 한다 (사용자 결정 2026-09-16).** 테스트폰은 있지만
상시 연결할 수 없다. 아내분 기기는 Galaxy S25+ 또는 S26+로 추정되며 정확한 모델과
Android·One UI 버전은 미확인이다(`docs/requests.md` R6). 모델 확인은 A0 착수를 막지 않는다.

- Windows는 x86_64 휴대전화 AVD 하나로 시작한다. 설치 시 SDK Manager에서 제공하는 최신
  안정 Android 이미지를 선택하고 실제 API 레벨을 기록한다. 실기기 버전은 나중에 별도로 확인한다.
- A0의 코어 호출과 A1~A3의 오버레이·걷기·드래그·알림·잠금 전환은 에뮬레이터에서 반복 검증한다.
  각 단계의 화면과 조작감은 사용자가 에뮬레이터로 확인한다.
- 일반 Android AVD의 결과를 Samsung One UI 검증으로 간주하지 않는다. A3의 제조사 배터리
  제한·잠금 뒤 복귀는 실기기 확인 전까지 미확인으로 남기고, A4는 아내분 실제 휴대전화에서 닫는다.
- 에뮬레이터 사용과 Windows 가상화 가속 설정은
  [Android Emulator 공식 안내](https://developer.android.com/studio/run/emulator)와
  [하드웨어 가속 안내](https://developer.android.com/studio/run/emulator-acceleration)를 따른다.
  도구 설치와 Windows 기능 활성화 여부는 환경 준비 때 확인한다.

**개발은 macOS와 Windows 양쪽에서 한다 (사용자 결정 2026-09-14).** 그래서 이 절만 보고 어느 쪽
기계에서든 A0를 이어갈 수 있어야 한다. **여기 없는 도구는 필요 없다.**

| | macOS | Windows |
|---|---|---|
| Rust | rustup + `rustup target add aarch64-linux-android x86_64-linux-android` | 같음 (`~/.cargo/bin`이 PATH에) |
| cargo-ndk | `cargo install cargo-ndk` | 같음 |
| Android SDK/NDK | Android Studio의 SDK Manager, `ANDROID_HOME`·`ANDROID_NDK_HOME` | 같음. 경로에 공백·한글이 없게(`C:\Android\sdk`) |
| JDK | Android Studio 내장 JBR. 실제 버전과 Gradle 호환성은 A0에서 확인 | 같음. 이 Windows 설치의 JBR은 25.0.3 |
| 실기기 | USB 디버깅을 켜고 `adb devices` | 같음. USB 드라이버는 제조사 것 |
| 에뮬레이터 ABI | Apple Silicon → **arm64-v8a** 이미지 | **x86_64** 이미지 |
| 빌드 스크립트 | `./scripts/build-android-core.sh` | `.\scripts\build-android-core.ps1` |
| 코어 테스트 | `./scripts/test.sh` | `.\scripts\test.ps1` |
| 호스트 라이브러리 | `rust/target/release/libroamling_android.dylib` | `rust\target\release\roamling_android.dll` |

- **Windows에서는 새 셸 하나가 선행 절차의 전부다.** 맥에서 `source ~/.cargo/env`로 하는 일을
  rustup 설치 관리자가 `%USERPROFILE%\.cargo\bin`을 사용자 PATH에 넣어 두는 것으로 대신한다 —
  설치 직후라면 셸을 새로 열면 `cargo`가 잡힌다.
- **Windows의 `ANDROID_HOME`·`ANDROID_NDK_HOME`은 사용자 환경 변수에 건다.** 시스템 속성의
  환경 변수 대화상자나
  `[Environment]::SetEnvironmentVariable('ANDROID_NDK_HOME', 'C:\Android\sdk\ndk\<ver>', 'User')`
  로 넣고 **셸을 새로 연다** — 현재 셸에만 넣으면 다음 세션이 같은 자리에서 다시 막힌다.
- **호스트에 따라 갈리는 것은 둘뿐이다** — 바인딩을 뽑는 호스트 라이브러리의 파일 이름과 에뮬레이터
  ABI. 그래서 빌드 스크립트의 `-t` 둘이 각각 한 호스트의 에뮬레이터 몫이 된다. 타겟 `.so`와 생성된
  Kotlin은 두 호스트에서 같아야 한다(`lto`·`codegen-units`는 workspace 프로필이라 공통,
  `rust/Cargo.toml:15-17`).
- Windows에서도 `cargo test`가 `roamling-android`를 돈다. default set에 넣기로 한 결과이고,
  `roamling-win`처럼 CI만이 방어선인 구멍을 만들지 않기 위해서다.
- Windows 셸과 Android 셸은 서로의 도구를 요구하지 않는다. `.cargo/config.toml`의 타겟 키가 그 격리다.

### Windows 환경 준비 결과 — 2026-09-16

사용자가 원격 작업 중이라 Windows 기능·BIOS·드라이버 변경과 재부팅 없이 준비했다.
공식 ZIP 배포본을 사용자 폴더에 풀고 시작 메뉴 바로가기를 만들었으며 **IDE 첫 실행은 하지 않았다.**
이 절은 이 PC의 설치 기록이고, 위의 Android 빌드 스크립트·Gradle 프로젝트가 구현됐다는 뜻은 아니다.

| 구성 | 설치 결과 |
|---|---|
| Android Studio | Quail 4 / 2026.1.4.7, `%LOCALAPPDATA%\Programs\android-studio` |
| 내장 Java | JBR 25.0.3, 위 경로의 `jbr` |
| SDK 루트 | `C:\Android\sdk` |
| SDK 패키지 | `platforms;android-37.1`, `build-tools;37.0.0`, `platform-tools` 37.0.1 |
| 명령줄 도구 | 공식 다운로드 페이지의 ZIP, revision 22.0 (`cmdline-tools\latest`) |
| NDK | r30 / 30.0.16248370 |
| Emulator | 37.1.11 |
| 가상 기기 | `Roamling_API37_1`, `medium_phone`, x86_64, CPU 2개, RAM 2048MB |
| 시스템 이미지 | `system-images;android-37.1;google_apis_ps16k;x86_64`, revision 9 |
| Rust | `aarch64-linux-android`·`x86_64-linux-android` 설치, `cargo-ndk` 4.1.2 |

사용자 범위에 `ANDROID_HOME`, `ANDROID_NDK_HOME`, `JAVA_HOME`을 설정하고 PATH에는
SDK의 `platform-tools`, `emulator`, `cmdline-tools\latest\bin`을 추가했다. 기존 항목은 보존했다.
Studio와 명령줄 도구 ZIP은 [공식 다운로드 페이지](https://developer.android.com/studio)의
SHA-256과 비교했고, SDK 패키지는 [sdkmanager](https://developer.android.com/tools/sdkmanager)로 설치했다.

**확인한 것:**

- `emulator -accel-check` exit 0, `WHPX(10.0.26200) is installed and usable`.
  WMI의 `HypervisorPlatform.InstallState=2` 조회와 달랐으므로 실제 도구 검사와 부팅 결과를 우선한다.
  Windows 기능을 바꿀 필요가 없었고, BIOS 가상화가 꺼졌다고 단정하지 않는다.
- 창 없는 부팅 약 77초 뒤 `sys.boot_completed=1`, `adb` 연결 `device`.
  게스트는 Android 17, API 37, 페이지 크기 16384바이트라고 응답했다.
- 임시 독립 크레이트를 `cargo ndk`로 ARM64·x86_64 둘 다 빌드했다.
  `llvm-readelf -l`에서 두 `.so`의 LOAD 정렬이 모두 `0x4000`이다.
  **Roamling 코어나 APK를 빌드·실행한 검사는 아니다.**
- 검사 뒤 해당 에뮬레이터를 종료하고 adb 기기 목록이 빈 것을 확인했다.
  캡처는 첫 화면 전환 중 모습이어서 런처 홈 화면의 육안 검증으로 세지 않는다.

다운로드·검증 로그와 임시 크레이트는 미추적 `output/android-setup/`에 있다. SDK 설치 로그에
`sdkmanager`의 Android CLI 전환 권고가 있으나 설치 exit는 0이다.

다음에 새 터미널에서 가상 기기를 직접 볼 때는 아래처럼 실행한다. 현재 세션에 환경 변수가
아직 반영되지 않았더라도 실행 파일의 절대 경로를 쓰면 된다.

```powershell
& 'C:\Android\sdk\emulator\emulator.exe' -avd Roamling_API37_1 -memory 2048 -cores 2
& 'C:\Android\sdk\platform-tools\adb.exe' devices
```

남은 것은 IDE 첫 실행, 실제 창에서의 조작 확인, A0의 Gradle·Rust 연결이다. 내장 JBR 25를
지원하는 Gradle wrapper 버전은 A0에서 고정한다. macOS 환경과 Samsung One UI 실기기는 미검증이다.

## 게이트

**각 게이트는 사용자의 실사용 확인으로 닫는다.** 한 단계의 체감 품질을 닫고 피드백을 받은 뒤에
다음으로 간다.

| | 무엇 | 닫는 조건 |
|---|---|---|
| A0 | 빌드 뚫기 — 크레이트, 스크립트 쌍, Gradle 뼈대 | 에뮬레이터에서 `PetLoop`를 만들고 틱 한 번 돌려 x, y를 로그로 본다. 화면에는 아무것도 안 뜬다 |
| A1 | 모치가 보인다 — 권한 화면(`Settings.ACTION_MANAGE_OVERLAY_PERMISSION` 인텐트), 오버레이, Bitmap, idle | 움직이지 않아도 좋다. 그림이 제자리에 뜬다 |
| A2 | 걷고 잡힌다 | 화면 경계 안 걷기·쉬기와 드래그 |
| A3 | 잠금과 알림 | foreground service, 숨기기/종료, SCREEN_OFF/USER_PRESENT |
| A4 | 아내분 실사용 | 첫 판의 exit. `docs/requests.md`의 R6를 닫는다 |
| A5 | 잠금화면 자리 | A4 뒤에 실기기로 지원 여부를 보고 결정 |

- **A0는 두 호스트에서 닫는다.** macOS와 Windows 양쪽에서 스크립트가 돌고 **생성된 Kotlin 바인딩이
  같아야** 한다. 한쪽에서만 닫힌 A0는 열린 것으로 본다. §"확인 필요"의 빌드 항목도 여기서 닫힌다.
- **A2에서 튜닝값을 사용자가 본다.** 데스크톱에서 눈으로 닫은 값(걷기 속도 등)이 dp에서 어떻게
  느껴지는지는 재 봐야 안다. 바꿀 것이 있으면 `FfiTuning`으로 넘기고 **core를 고치지 않는다** —
  고치면 두 데스크톱의 체감이 같이 움직인다.
- A5의 기본값은 "만들고 싶은 경험"에 적힌 대체안, 즉 알림에 조용한 상태를 보여 주는 쪽이다.

## 확인 필요

- uniffi 0.32 Kotlin library mode가 **한 `.so` 안의 두 크레이트**를 한 번에 뽑는지 (A0).
- cargo-ndk가 `rust/.cargo/config.toml`의 android 타겟 rustflags를 존중하는지. 16 KB 페이지 정렬은
  `readelf -l`로 본다 (A0).
- Windows에서 NDK 경로의 공백·한글 문제 (A0).
- straight alpha RGBA8을 Android `Bitmap`에 넣을 때의 premultiply 처리 (A1).
- `image` 크레이트의 WebP 디코드가 휴대전화 CPU에서 얼마나 걸리는지 (A1). **데스크톱 수치를
  그대로 옮기지 않는다** — 맥에서 잰 것은 시트 한 장 디코드 24.09 ms(Rust native)이고,
  거기에 uniffi를 건너면 39.37 ms가 된다. **그 차이 15.3 ms는 디코드가 아니라 11.5 MB가 FFI를
  건너는 비용**이다(`docs/history/windows.md:2313-2317`). Android도 RGBA를 Kotlin에 넘기므로
  둘을 따로 재야 한다.
- Android 14 `specialUse` 서비스 타입이 sideload에서 걸리는 것이 없는지 (A3).
- 제조사 배터리 최적화가 foreground service를 죽이는지. 특히 Samsung. 실기기로 본다 (A3).
- 손가락이 사라진 뒤의 "포인터 위치"가 실제로 어떻게 느껴지는지 (A2).
- 잠금화면 위젯의 기기·버전별 지원 (A5).
