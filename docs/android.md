# Android 첫 프로젝트 방향

**이름 변경 (2026-09-17):** 현재 UI의 이름은 `보리 / Bori`다. `MochiImages`·`MochiOverlay` 같은
내부 식별자는 유지한다. 아래 2026-09-16 검증 기록과 당시 캡처의 모치는 같은 마스코트의 이전 이름이다.

**상태: A2는 66775b2로 커밋, A3 서비스·알림·잠금 복귀 구현과 에뮬레이터 검사 완료 (2026-09-17).** 사용자 체감·Samsung 실기기와 A0 macOS 검증은 남아 있다. 첫 사용 대상은 아내분의 Android
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

첫 실행에서 사용자가 Android의 **다른 앱 위에 표시** 권한을 직접 허용하게 한다. 보리를 시작한
세션 동안 foreground service를 쓰고, 알림에서 언제든 숨기거나 끝낼 수 있게 한다.
숨김·잠금 중에는 창과 틱을 중단하며 재개/종료용 알림은 유지한다(`CompanionService`, `MochiOverlay.pause`).

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
전부 `rust/roamling-core`의 `PetLoop`(`src/ffi/runtime.rs`)에 있고, Android 셸이 하는 일은 Windows
셸과 같다 — 기계를 읽어 `FfiTickInput`(`ffi/runtime.rs`)을 채우고 `FfiTickOutput`
(`ffi/runtime.rs`)대로 그린다. Windows에서 그 본체는 `rust/roamling-win/src/main.rs`의 `tick`
이고, 입력을 모으는 자리가 651-676이다.

갈리는 것은 붙는 방식뿐이다. macOS는 uniffi Swift 바인딩(`scripts/build-rust-core.sh:38-41`),
Windows는 rlib 직결(`rust/roamling-core/Cargo.toml:11`의 `crate-type`), **Android는 uniffi Kotlin**.

**코어를 다시 쓰지 않는다.** 재구현의 숨은 비용은 코드가 아니라 **검증**이다 — 걷기 속도 같은
튜닝값은 사용자가 눈으로 보고 닫은 것이라 다시 쓰면 그 눈을 다시 빌려야 한다. 그 판단의 기록이
`docs/history/windows.md` 3절이고, 언어를 바꾸자는 논의를 시작하기 전에 읽을 문서도 그것이다.

seam의 정본 표는 `docs/architecture.md`의 "플랫폼 seam" 절에 있다. Android 열은
현재 A0–A3 코드를 적고, 아래의 첫 판 표는 이후 게이트까지 포함한 계획이다.

## 무엇이 어디에 사나

```text
rust/roamling-android/   새 크레이트. cdylib 하나(libroamling_android.so).
                         roamling-core + roamling-pet에 의존하고 자기 uniffi scaffolding을
                         갖는다. default_tuning, MascotAtlas, Player를 노출한다.
android/                 Gradle 프로젝트. 모듈 둘
  core/                  생성된 Kotlin 바인딩(미추적) + jniLibs/<abi>/*.so + JNA
  app/                   A0: debug 전용 CoreSmokeActivity. MainActivity 권한 화면과
                         MochiOverlay 미리보기. A2 PreviewRuntime이 공유 코어 틱·터치를 연결.
                         A3 CompanionService가 창·코어·알림·잠금 수명을 소유.
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

**`roamling-agent`와 업데이터를 Android 셸에서 사용하지 않는다.** `roamling-core/Cargo.toml`에
이미 `roamling-update` 의존이 있으므로 간접 의존까지 없다는 초기 초안은 정정한다.
A0에서는 기존 코어의 의존·FFI 표면을 보존하고 Android에 업데이트 호출 경로를 만들지 않는다.
배포는 APK 설치(adb 또는 파일)로 대신한다.

### A0 구현 범위

- `roamling-android`는 자체 scaffolding과 코어 scaffolding을 한 라이브러리로 내보낸다.
  Kotlin에 튜닝 기본값을 복사하지 않도록 `default_tuning()`만 새 표면으로 추가한다.
  `MascotAtlas`·`Player`는 A1에 남긴다. A0에서 이미지 디코딩·표시는 하지 않는다.
- `android/core`는 생성된 두 Kotlin 패키지와 두 ABI의 `.so` 및 JNA를 묶는다.
  `android/app`의 최소 Activity가 코어의 `PetLoop`를 만들고 디스플레이를 설정한 뒤
  `begin_tick` → `finish_tick`을 한 번 호출해 유한한 x/y를 검증하고 로그를 남긴 후 종료한다.
  A0에는 오버레이 권한·서비스·지속 타이머가 없었다. 현재 구현은 아래 A1–A3에 있다.
- 휘도 요청은 캡처 실행 자체가 아니다. 코어의 `request_luminance`는 요청을 반환하고
  Windows도 `refresh_luminance`에서 opt-in 여부를 검사한다. Android A0는 캡처 API를 호출하지
  않으며 반환된 요청은 처리하지 않는다. 권한 false이면 요청 목록도 비어야 한다는 검사는 잘못된 계약이다.
- Android 11(API 30)을 최소로 두고, 설치된 SDK 37.1로 컴파일한다. `currentWindowMetrics`의
  insets를 뺀 경계를 dp로 변환한다. 앱 버전은 Rust workspace의 버전에서 읽는다.
- Windows·macOS/Linux 빌드 스크립트와 Linux APK 빌드 CI를 함께 추가한다.
  이 Windows에서의 성공과 macOS 실측, 사용자 확인은 별도로 기록한다.

### A1 구현 흐름 — 사용자 "커밋하고 다음으로 가자" 승인

이번 범위는 권한 화면과 실제 `TYPE_APPLICATION_OVERLAY` 창에 기존 Mochi idle을 표시하는
미리보기다. A3의 foreground service 전이므로 **Activity가 보이는 동안만** 표시하고,
홈·뒤로가기·잠금으로 Activity가 멈추면 창과 애니메이션 콜백을 제거한다.

1. `MainActivity`: 표시 버튼 → `Settings.canDrawOverlays` 확인 → 없으면 Android 설정으로 이동.
   돌아오면 권한을 다시 확인한다. 거절·설정 화면 없음은 앱 안에서 설명하며 계속 재요청하지 않는다.
2. `MascotAtlas` (`rust/roamling-android/src/lib.rs`): `built_in_mochi()`를 한 번 디코딩해 보유한다.
   배경 스레드에서 바인딩을 통해 두 시트를 받는다. `PetImage`는 **이미 premultiplied RGBA8**이다
   (`rust/roamling-core/src/pet_image.rs::PetImage`). 초기 계획의 straight-alpha 가정은 정정한다.
3. `MochiImages`: Android `ARGB_8888` bitmap에 `copyPixelsFromBuffer`로 바이트를 복사한다.
   alpha를 다시 곱하거나 `setPixels`로 이중 변환하지 않는다. 작은 합성 픽셀 검사로 색·alpha를 확인한다.
4. `Player`: 기존 `AnimationResolver`·`PetAnimationPlayer`를 감싸 프레임의 시트와 사각형을 반환한다.
   Kotlin에 트랙 이름·프레임 시간표를 복사하지 않는다. A1은 초기 idle만 재생하고 위치는 고정한다.
5. `MochiOverlay`: 투명한 96×104 dp 창, 초점·터치 미획득, bitmap 필터링 없이 그린다.
   system bar·cutout insets 안에 배치한다. 숨기기·Activity 중단·회전 시 이전 창과 콜백을 정리한다.
   화면 회전 후 표시 의도를 복원하되 권한은 다시 검사한다. 잠금 위에 남기지 않는다.

UI 문구는 `android/app/src/main/res/values{,-ko}/strings.xml`에 같은 키로 둔다.
걷기·드래그·잠금 뒤 자동 복귀·상시 서비스는 A2/A3 범위다. macOS와 Samsung 실측은 별도로 남긴다.
공식 근거: [overlay 권한](https://developer.android.com/reference/android/provider/Settings#ACTION_MANAGE_OVERLAY_PERMISSION),
[창 종류](https://developer.android.com/reference/android/view/WindowManager.LayoutParams#TYPE_APPLICATION_OVERLAY),
[bitmap 복사](https://developer.android.com/reference/android/graphics/Bitmap#copyPixelsFromBuffer(java.nio.Buffer)).

### A2 구현 흐름 — 사용자 "응 다음 진행 고고" 승인 (2026-09-17)

아래는 구현 계약이다. A1의 Activity 수명을 유지하고 전체 화면의 system bar·cutout 안에서 움직인다.

1. `PreviewRuntime`이 기존 `PetLoop`와 `Player`를 소유한다. `default_tuning()`을 그대로 쓰고
   `set_displays`·`set_object_size`로 dp 좌표계를 설정한다. `Player.duration`은 resolver가 고른
   caught/dragged 트랙 길이를 반환하여 `set_animation_durations`에 전달한다.
2. `PreviewRuntime.tick`: 단조 시계 → `begin_tick` → `finish_tick`. 포커스·캡처 권한은 false,
   손가락이 없으면 화면 밖 포인터, 실제 마지막 터치 이후 경과를 idle로 준다. 위치와 capability,
   `delta_time * locomotion_rate`를 창과 플레이어에 전달한다. Kotlin에 행동 규칙을 복제하지 않는다.
3. **직접 터치에는 가짜 접근 좌표·속도를 만들지 않는다.** `PetRuntime::touch_down` /
   `PetLoop.touch_down`이 실제 펫 영역 안 접촉을 받고 hidden/interactions/이미 잡힌 상태를
   검사한 뒤 `begin_catch`를 부른다. 2026-09-18부터 데스크톱 `pointer_down`도 같은 직접 접촉
   검증을 사용한다. Android의 터치 경로는 그대로이며 마우스의 빠른 접근이 필수라는 설명은
   더 이상 현재 동작이 아니다(`docs/behavior-flow.md` 4절).
4. `MochiOverlay.SpriteView.onTouchEvent`: 첫 pointer ID만 소유한다. MOVE는 dp 좌표와 시작점부터
   거리를 기존 `pointer_dragged`에 전달하고, UP/CANCEL/소유 손가락 POINTER_UP은 `pointer_up`으로
   해제한다. 두 번째 손가락으로 소유권을 옮기지 않는다. 드래그 중에도 `set_scale`의 코어 clamp를
   적용하며, release 뒤 포인터를 화면 밖으로 지운다. 터치 영역은 96×104 dp 창 하나다.
5. `MochiOverlay.schedule`: `preferred_tick_interval`이 1/60이면 Choreographer, 그 외 Handler.
   `reschedule_after`를 존중하고 이전 예약은 취소한다. 변경된 위치·프레임만 창에 반영한다.
   `ACTION_OUTSIDE`와 `MainActivity.dispatchTouchEvent`는 사용자 입력 시각만 갱신한다.
6. `MainActivity`: 회전/재생성 시 마지막 dp 위치를 Bundle로 운반하고 새 경계에 clamp한다.
   숨김·홈·종료 시 창·타이머·native 객체를 해제한다. 장기 위치 저장·상시 서비스는 A3 범위다.

검증: 기존 렌더/수명 계측에 실제 이동·시스템 주입 드래그·취소·회전 경계를 더한다.
`PreviewRuntime`에 주입한 시계로 기본 휴식 시간과 깨우기를 검증하고 실제 OS 타이머와 구분한다.
공식 API 근거: [MotionEvent](https://developer.android.com/reference/android/view/MotionEvent),
[outside touch](https://developer.android.com/reference/android/view/WindowManager.LayoutParams#FLAG_WATCH_OUTSIDE_TOUCH).

### A2 결과 (2026-09-17, Windows 호스트)

- `PreviewRuntime`: 기본 코어 튜닝의 걷기·휴식·잠들기·깨우기, `Player`의 실제 트랙 길이와
  프레임을 연결했다. `MochiOverlay`는 코어 중심점을 창 왼쪽 위 좌표로 변환하고 변경 시에만
  창/프레임을 갱신한다. 터치 해제 후 포인터를 남기지 않는다.
- `PreviewTest` 3개 통과: premultiplied 픽셀, 실제 창 이동·시스템 입력 드래그·멀티터치 해제·취소·
  재생성·가로 회전 경계·Home 제거, 그리고 기본 75초 휴식·수면 주기·깨우기·드롭 경계.
  마지막 휴식 검사는 **주입한 단조 시계**로 시간을 진행했다. 장시간 실기기 배터리 검사가 아니다.
  UI 이동 검사는 스크린샷보다 먼저 출발 위치를 기록하여 짧은 첫 걸음 뒤의 휴식을 이동 실패로
  오인하지 않게 했다. 제품 튜닝과 기존 fixture/trace는 바꾸지 않았다.
- `build-android-core.ps1 -Jobs 2`: Windows host 바인딩 및 ARM64/x86_64 성공.
  Gradle `assembleDebug`, `assembleDebugAndroidTest`, `lintDebug` 성공. lint는 오류 0,
  기존 `DataExtractionRules` 경고 1. APK 안 4개 `.so`의 ELF LOAD `0x4000`, `zipalign -c -P 16 4` 통과.
- `test-android.ps1 -Serial emulator-5580`: A0 PASS. `test-android-preview.ps1 -Serial emulator-5580`:
  `OK (3 tests)` (15.335초). runner는 앱을 종료하고 기존 overlay app-op을 복원한다.
  캡처 `output/android-setup/a2-preview.png`, `a2-preview-landscape.png`를 직접 확인했다.
- `scripts/test.ps1`: 코어/펫/agent/update 및 Windows 셸 검사·release 빌드 **all green**.
  새 코어 `direct_touch_uses_the_shared_catch_drag_and_drop_without_arming_a_mouse` 검사도 통과.
- 재현 로그는 `output/android-setup/a2-{native-build,gradle,windows-tests,a0-smoke,instrumentation}.log`.
  최신 개발 APK는 `android/app/build/outputs/apk/debug/app-debug.apk`이며 테스트 AVD에 설치했다.
  이번 변경은 커밋·push·릴리스하지 않았다.

**A2 완료 당시에는 Activity 화면에서만** 동작했다. 현재 Home 유지/서비스/알림/잠금 복귀는 아래 A3에 구현되어 있다. 이 A2의
속도·드래그 체감은 사용자 확인을 기다린다. 이번 변경의 macOS 및 Android macOS 호스트 빌드,
Samsung/One UI 실측은 하지 않았다. 0.6.4의 이전 macOS CI 성공을 이번 변경 검증으로 세지 않는다.

### A3 구현 흐름 — 사용자 "응 커밋하고 다음 진행" 승인 (2026-09-17)

1. `MainActivity`는 `CompanionService.LocalBinder`로 상태를 구독하며, 더 이상 창·native 객체를
   소유하지 않는다. 화면에 있는 사용자의 보기 요청에서만 `startForegroundService`를 호출한다.
   overlay 설정 복귀 후 알림 권한을 안내한다. 제어 알림을 볼 수 있게 알림 허용을 시작 조건으로
   두고, 거절하면 설명과 다시 요청/설정 경로를 제공한다. OS가 알림 허용을 FGS 필수 조건으로
   강제하는 것은 아니며, 이 앱의 시작 UX 선택이다.
2. `CompanionService.onStartCommand`는 즉시 조용한 LOW 알림과 `specialUse` foreground를 시작한
   뒤 atlas를 비동기로 로드한다. Manifest에 FGS·SPECIAL_USE·POST_NOTIFICATIONS를 선언하고
   subtype을 설명한다. 요청 도중 숨김·종료 시 늦게 끝난 로더가 창을 되살리지 않도록 세대를 검사한다.
3. 서비스가 `MochiOverlay`·이미지·코어를 소유한다. Home/다른 앱/Activity 재생성은 계속 표시한다.
   `MochiOverlay.pause/show`는 코어 hidden 플래그·접촉 해제·모든 예약 취소·창 제거/재부착을
   함께 수행한다. 화면 밖 터치는 시각만 받으며, 화면 캡처·접근성·wake lock은 쓰지 않는다.
4. 동적 screen receiver가 OFF/ON/USER_PRESENT를 받고 `PowerManager.isInteractive`와
   `KeyguardManager.isKeyguardLocked`를 다시 확인한다. 잠금 위에는 창이 없고 틱도 없다.
   해제 후 사용자 숨김 상태가 아니면 마지막 자리에서 재개한다. 알림 숨김은 수동 상태라 해제해도
   유지한다. 서비스 자체를 새로 시작하는 unlock/boot receiver는 만들지 않는다.
   이 세 protected system broadcast의 receiver는 `RECEIVER_EXPORTED`로 등록한다. Android 17
   테스트 기기의 SystemUI는 system UID 1000이 아닌 `android.uid.systemui/10185`여서
   NOT_EXPORTED에서는 USER_PRESENT를 놓쳤다. 프레임을 계속 폴링하는 우회 대신 수신 경계를
   바로잡고, 표시 직전 OS의 interactive/keyguard 상태를 계속 재확인한다.
   근거: [시스템 broadcast의 UID와 exported 규칙](https://developer.android.com/develop/background-work/background-tasks/broadcasts).
5. 알림의 숨기기/다시 보기/종료는 명시적 immutable PendingIntent로 비공개 서비스에 전달한다.
   숨김·잠금 중 알림은 상태와 재개/종료 제어를 위해 유지하되 native 틱을 중지한다.
   종료는 창·콜백·이미지·알림을 정리하고 `stopSelf`한다. `START_NOT_STICKY`로 강제 종료나
   OS process 종료 뒤 자동 부활시키지 않는다. 필요하면 사용자가 앱에서 다시 시작한다.
6. `PreviewRuntime`의 persist_position 출력과 pause/stop/config 전환 때 마지막 중심 dp를
   SharedPreferences에 저장한다. `CompanionService.onConfigurationChanged`는 위치를 운반해
   새 metrics/density의 창을 만든다. 권한 회수(AppOps watcher)는 즉시 중단하고 설정 복귀 시 재검사한다.

검증: 기존 A2 이동/드래그·회전 검사를 서비스 소유 수명에 맞춰 유지하고, 실제 Home/다른 앱,
알림 PendingIntent 숨김/재개/종료, 화면 OFF/ON·keyguard 해제, 위치 저장, 권한 거절/회수,
서비스 중단 후 자동 부활 없음까지 에뮬레이터에서 확인한다. 삼성 배터리 정책·장시간 실측은 A4다.
근거: [FGS specialUse](https://developer.android.com/develop/background-work/services/fgs/service-types#special-use),
[백그라운드 시작 제한](https://developer.android.com/develop/background-work/services/fgs/restrictions-bg-start),
[알림 권한](https://developer.android.com/develop/ui/views/notifications/notification-permission),
[START_NOT_STICKY](https://developer.android.com/reference/android/app/Service#START_NOT_STICKY).

### A3 결과 (2026-09-17, Windows 호스트)

- `CompanionService`가 Activity와 독립적으로 창·native runtime·알림을 소유한다. Home/다른 앱에서도
  유지하고 알림의 숨김/다시 보기/종료를 처리한다. `MochiOverlay.pause`는 숨김·화면 꺼짐·잠금 때
  창과 틱을 중단하며, `USER_PRESENT` 뒤 수동 숨김이 아니면 저장 위치에서 복귀한다.
- Gradle `assembleDebug`·`assembleDebugAndroidTest`·`lintDebug` 성공. lint 오류 0, 기존
  `DataExtractionRules` 경고 1. `zipalign -c -P 16 4` 통과. A3는 Kotlin 셸 변경이며 A2의
  Rust 코어·native 라이브러리·fixture·trace는 변경하지 않았다.
- Android 17 / API 37 / 16 KB x86_64 에뮬레이터에서 `PreviewTest`·`CompanionServiceTest`
  **5개 통과**. 기존 걷기·드래그·회전·픽셀 검사와 Home/다른 앱 유지, 알림 action 실행,
  숨김/잠금 중 틱 중단, 실제 swipe 잠금 해제 후 복귀, 수동 숨김 유지, 종료·위치 복원,
  오버레이 권한 회수 시 종료를 확인했다. 홈 전환으로 회전이 바뀌어도 서비스의 현재 창을 검사한다.
- 잠금 해제 신호가 SystemUI의 별도 UID에서 와서 비공개 receiver에 전달되지 않는 결함을 재현했다.
  위 흐름의 보호된 시스템 broadcast 수신 설정으로 고친 뒤 최종 5개 검사를 다시 통과했다.
- 실제 권한 UI에서 알림 거절 시 시작하지 않고, 재요청에서 허용하면 시작됨을 확인했다
  (`MainActivity.requestShow`·`onRequestPermissionsResult`). 런처 위 보리, 조용한 알림과 펼친
  숨기기/종료 버튼을 캡처로 확인했고 실제 종료 버튼도 눌러 확인했다.
- 테스트 후 AVD의 잠금 설정·알림 권한·알림 요청 이력을 복구했다. 기존 overlay 권한 `allow`는
  유지했다. 창 없는 에뮬레이터를 종료하고 원래 설치본 데스크톱 펫을 다시 실행했다.

재현 명령은 아래 A1 절의 Gradle·`test-android-preview.ps1`과 같다. 계측 스크립트는 명시적인
에뮬레이터에만 임시 알림 권한과 swipe 잠금을 적용하고 `finally`에서 원복한다.
로그는 `output/android-setup/a3-{gradle,test-build,instrumentation}.log`, 캡처는
`a3-preview.png`·`a3-preview-landscape.png`·`a3-home.png`·`a3-notification.png`와 수동 검사의
`a3-notification-expanded.png`다. APK는 `android/app/build/outputs/apk/debug/app-debug.apk`다.
이 산출물은 모두 미추적이다.

Samsung/One UI의 장시간 유지·배터리 제한, PIN/생체 인증 잠금, ARM64 실행은 미검증이다.
A3에서 Windows 전체 게이트나 macOS CI를 새로 실행한 것은 아니다. 사용자 체감 확인 뒤 A4로 간다.

### A3 사용자 확인 순서

기존 `Roamling_API37_1` 에뮬레이터에는 A3 개발 APK가 설치되어 있다. 아래 명령으로 창을 열고
앱 목록에서 Roamling을 실행한다. 이미 해당 AVD가 실행 중이면 기존 창을 사용한다.

```powershell
& 'C:\Android\sdk\emulator\emulator.exe' -avd Roamling_API37_1 -memory 2048 -cores 2
```

1. **보리 보기**를 누르고 알림을 허용한다. 크기·걷기 속도와 손가락으로 잡아 옮기는 느낌을 본다.
2. 홈으로 나간 뒤 다른 앱을 열어도 보리가 남아 있고, 주변 버튼을 누르는 데 방해되는지 본다.
3. 알림을 펼쳐 **숨기기 → 다시 보기**를 확인한다. 숨긴 상태에서는 잠금·해제해도 숨김이 유지되어야 한다.
4. 보리를 표시한 상태에서 에뮬레이터 전원 버튼으로 화면을 끄고 다시 켠다. 잠금화면에는 보리가
   없고 잠금을 해제하면 돌아오는지 본다. AVD에 잠금이 꺼져 있으면 화면 꺼짐/켜짐만 확인된다.
5. 알림의 **종료**를 누르면 보리와 알림이 사라지는지, 앱을 다시 열기만 해서는 시작되지 않고
   **보리 보기**를 눌러야 다시 나오는지 확인한다.

이는 `MainActivity.requestShow`, `CompanionService`의 알림 action·screen receiver·`stopCompanion`,
`MochiOverlay`의 터치 경로에 대한 체감 확인이다. 자동 검사 통과와 별도로 결과를 받는다.
Samsung 배터리 제한과 PIN/생체 인증은 이 에뮬레이터 확인으로 완료 처리하지 않는다.

## Kotlin이 채우는 것 — 첫 판 계획

아래 표는 A3까지 포함한다. 현재 구현은 위 구현 흐름과 `docs/architecture.md`의 표를 따른다.

런타임이 기계에 닿는 통로는 `PlatformServices` 하나이고(`Sources/RoamlingEngine/
PlatformServices.swift:54-71`), 자리는 열하나다(`docs/architecture.md:586-629`). Android는 그중
**넷(safeZone · focus · capture · window)을 첫 판에서 비워 둔다.**

| 자리 | Android 답 | 첫 판 |
|---|---|---|
| display | `WindowManager.currentWindowMetrics` 경계 − `WindowInsets`(status·navigation·cutout) → `FfiDisplay` 하나 | 채움 |
| displayChanges | Activity 재생성(회전·크기 변경) → 위치 운반·디스플레이 다시 설정 | 채움 |
| safeZone | insets를 display 경계에서 이미 뺐으므로 빈 목록 | 빈값 |
| pointer | 오버레이 View의 `MotionEvent` 좌표(px→dp) | 채움 |
| userIdle | 마지막 터치 시각으로부터의 경과 | 채움 |
| focus | `focus_authorized=false`, `did_query_focus=false`, `queried_focus=null` | 스텁 |
| capture | `capture_authorized=false`, 휘도 없음 | 스텁 |
| window | 빈 목록 | 스텁 |
| overlay | `TYPE_APPLICATION_OVERLAY` + `FLAG_NOT_FOCUSABLE` + `FLAG_LAYOUT_IN_SCREEN` 창 하나를 insets 안에서 `LayoutParams.x/y`로 옮긴다 | 채움 |
| images | `MascotAtlas`의 RGBA8 → `Bitmap`(ARGB_8888) | 채움 |
| coordinateSpace | px ÷ density. y 뒤집기 없음 | 채움 |

**틱 입력에는 자리가 아닌 것이 둘 더 있다.** 아래 둘은 `PlatformServices`의 슬롯이 아니라
`FfiTickInput`(`rust/roamling-core/src/ffi/runtime.rs`)의 필드다 — 셸이 매 틱 직접 채운다.

| 틱 입력 | Android 답 | 첫 판 |
|---|---|---|
| `primary_button_down` | `ACTION_DOWN`~`ACTION_UP` 사이 true | 채움 |
| `affection_held` | 항상 false. 휴대전화에는 모디파이어 키가 없다 | 상수 |

**1 world pt = 1 dp로 둔다.** Windows가 `world_scale()`(`rust/roamling-win/src/platform.rs:51`)에서
픽셀을 DPI 배율로 나눠 world 단위를 만들고, 그 나누기를 `rect_to_world`(`platform.rs:72`)와
`pointer`(`platform.rs:129`)가 똑같이 쓴다. Android의 density가 그 배율 자리에 그대로 들어간다.
**y를 뒤집지 않는다** — Win32도 Android도 좌상단 원점·y 아래라 core world와 방향이 같다.
다만 **펫 위치는 중심점**이다(`DesktopWorldSnapshot::clamp` → `WorldRect::clamped_center`).
`MochiOverlay.render/show`만 `중심 px − 창 크기/2`로 `LayoutParams.x/y`를 만든다.
터치 raw 좌표는 화면 좌표 그대로 dp로 바꾸며, Bundle에도 코어의 중심을 저장한다.

**손가락이 없을 때의 포인터는 마지막 터치 자리가 아니라 화면 밖 좌표다.** 데스크톱 커서는 화면에
남아 있지만 손가락은 사라진다. 남겨 두면 펫이 있지도 않은 손을 계속 피한다.

**userIdle은 `lastTouchAt`에서 재고 상수로 주지 않는다.** `finish_tick`은
`user_idle_duration < 0.8`이면 쉬던 펫을 깨우므로(`rust/roamling-core/src/pet_runtime.rs` `finish_tick`),
0을 주면 영영 못 자고 큰 값을 주면 영영 못 깬다. 오버레이 밖 터치는 `FLAG_WATCH_OUTSIDE_TOUCH`의
`ACTION_OUTSIDE`(좌표 없이 시각만)와 Activity 터치로 받는다. 화면 켜짐·`USER_PRESENT`에
같은 시계를 리셋하며, 잠금 여부를 확인한 후 재개한다(`CompanionService.reconcileVisibility`).
**내용은 보지 않고 시각만 본다** — macOS가 `CGEventSource`의 경과 시간 하나만 쓰는 것과 같은 선이다.

**`pointer_is_over_pet`은 View가 곧 hit region이다.** 오버레이 창이 펫 크기라 터치가 그 안에
들어왔는지가 곧 답이다. Windows가 펫의 반쪽 크기를 만들어(`main.rs` `tick`의 `half_width` · `half_height`) 포인터와의 거리를
재는 계산(같은 함수의 `pointer_is_over_pet`)이 이쪽에는 필요 없다.

**길게 누름을 애정으로 볼지는 A2 뒤에 사용자가 정한다.** 그때까지 `affection_held`는 상수 false다.

**전면 앱 식별(focus)과 화면 캡처는 첫 판에서 안 한다.** UsageStats·접근성·MediaProjection 권한이
필요하고, 첫 판의 제외 목록("기존 프로젝트에서 가져올 것")에 이미 들어 있다.

## roamling-android가 노출하는 것 — A1 구현

`ffi/`에는 애니메이션이 없다 — `AnimationResolver`(`rust/roamling-core/src/animation.rs:297`)와
`PetAnimationPlayer`(`animation.rs:446`)는 uniffi로 나가 있지 않고, 내장 마스코트는 uniffi가 없는
`roamling-pet`에 있다. A0의 `default_tuning()`에 이어 새 크레이트가 감싸는 것은 아래 둘이다
(`rust/roamling-android/src/lib.rs`).

- **`MascotAtlas`** — `built_in_mochi()`(`rust/roamling-pet/src/lib.rs:211`)를 감싸 표준·확장 시트
  각각의 `FfiPetImage`(width·height·premultiplied RGBA8)와 `AtlasFrame`(시트·사각형)을 준다.
  프레임 배치는 기존 `PetAsset::frame_rect`에 위임한다. 아틀라스는
  `include_bytes!`로 `.so` 안에 들어가므로(`lib.rs:122-125`) **APK에 에셋을 따로 넣지 않는다.**
  리컬러(`lib.rs:226`)는 첫 판 제외이고 나중 게이트다.
- **`Player`** — 위 둘과 `PetAsset.tracks`(`rust/roamling-pet/src/lib.rs:51-65`)를 감싸,
  Kotlin은 capability와 delta를 주고 **시트와 프레임 사각형만** 받는다. A1은 idle capability와
  `Choreographer`의 경과 시간을 사용한다. A2의 이동 틱에서는 `TickOutput.delta_time`에
  `locomotion_rate`를 곱한다(`PreviewRuntime.tick`).
  **capability→track 해석은 Rust가 한다** —
  Kotlin이 트랙 이름을 알기 시작하면 `docs/state-contract.md`의 층 구조를 셸이 깰 수 있다.
- `MascotAtlas`는 불변 에셋을 소유한다. `Player`는 `Arc<MascotAtlas>`와 resolver를 보유하고,
  변하는 `PetAnimationPlayer`만 `Mutex`로 감싼다. 잘못된 capability·음수·비유한 delta는 상태 변경 없이 거부한다.

## 틱과 생명주기 — A2/A3 계획

A2는 `PreviewRuntime`에서 `PetLoop`를 진행하고 `MochiOverlay`가 주기·창·터치를 연결한다.
A3는 `CompanionService`가 이를 소유하며 위치를 SharedPreferences에 저장한다. 아래는 현재 흐름이다.

```text
Service 시작    PetLoop.new(저장된 자리 또는 화면 중앙, 기본 FfiTuning, seed)   ffi/runtime.rs PetLoop::new
                set_displays(1619) / set_object_size(1638) / MascotAtlas → Bitmap
매 틱           begin_tick(now)(1735) → 포커스는 묻지 않음 → finish_tick(1739)
                → 창 위치 갱신, Player로 프레임 하나 → 다음 틱 예약
터치            PetLoop.touch_down / pointer_dragged / pointer_up (ffi/runtime.rs)
자리 저장       persist_position이 참일 때 SharedPreferences에 x, y
SCREEN_OFF      set_hidden(true)(1667), 틱 중단, 창 제거
USER_PRESENT    set_hidden(false), 창 다시 붙임, 저장된 자리에서 재개(set_position 1678)
알림 [숨기기]   set_hidden(true) + 창 제거. 서비스는 유지
알림 [종료]     서비스 stop
```

- **틱 간격은 셸이 고르지 않는다.** `preferred_tick_interval`
  (`rust/roamling-core/src/pet_runtime.rs`)이 걷는 중 1/60, 잡힌 중 1/30,
  포인터를 보는 중 1/16(`pet_runtime.rs` `preferred_tick_interval`), 자는 중 1/2, 그 외 1/12을 준다. 1/60일 때만 `Choreographer` 프레임 콜백, 그 외에는 `Handler.postDelayed`.
- **터치 뒤의 재예약을 빠뜨리지 않는다.** `InteractionOutput`의 `reschedule_after`
  (`rust/roamling-core/src/ffi/runtime.rs`의 `FfiInteractionOutput`)가 차 있으면 그 시각에 한 번 더 틱한다.
- 시계는 `SystemClock.elapsedRealtime()`을 초로 바꾼 단조 시각이다. core는 f64 초만 받는다.
- foreground service는 사용자가 시작한 세션 동안 유지한다. 숨김·잠금은 틱 없는 일시정지이고
  알림 [종료]가 세션을 끝낸다. Android 14+의
  `foregroundServiceType`은 `specialUse`로 선언하고 사유를 매니페스트에 적는다.
- 배터리 산술은 `docs/battery.md` 그대로이고, **데스크톱의 지배 항목이 여기엔 아예 없다** —
  62 ms짜리 화면 캡처(`docs/battery.md:11`)를 첫 판이 하지 않는다. 남는 것은 틱 캐던스뿐이고
  그것은 위 표대로 core가 정한다.

## 빌드·버전

```sh
bash scripts/build-android-core.sh       # macOS/Linux
bash android/gradlew -p android :app:assembleDebug --no-daemon
```

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\scripts\build-android-core.ps1
.\android\gradlew.bat -p android :app:assembleDebug --no-daemon
# 부팅한 에뮬레이터의 실제 serial을 adb devices로 확인한 뒤:
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\scripts\test-android.ps1 -Serial emulator-5580
```

APK는 `android/app/build/outputs/apk/debug/app-debug.apk`에 생긴다. A0에는 런처 아이콘이
없으며 debug 전용 Activity를 명시적으로 실행한다. Windows 검사 스크립트는 현재 실행의
고유 token을 로그에서 확인하므로 이전 PASS를 성공으로 오인하지 않는다. 에뮬레이터를
자동으로 부팅하거나 실제 휴대전화에 설치하지 않는다.

macOS에서는 부팅한 에뮬레이터의 serial을 확인한 뒤 아래처럼 같은 검사를 실행할 수 있다.
출력의 token이 이번 실행과 일치하고 x/y가 유한한지 본다.

```sh
serial=emulator-5554  # adb devices에 나온 에뮬레이터
smoke_token=$(uuidgen)
adb -s "$serial" install -r android/app/build/outputs/apk/debug/app-debug.apk
adb -s "$serial" shell am start -W -n io.github.creatorkoo.roamling/.CoreSmokeActivity --es smoke_token "$smoke_token"
adb -s "$serial" logcat -d -s RoamlingA0:I '*:S' | grep "$smoke_token"
```

둘 다 host 라이브러리를 빌드한 뒤 `uniffi-bindgen`을 `--language kotlin`으로 부르고,
`cargo ndk -t arm64-v8a -t x86_64 --platform 30 build --locked --release
-p roamling-android --lib`로 빌드한 뒤 `libroamling_android.so`만 ABI별 `jniLibs`로 복사한다.
작업 디렉터리는 `rust/`다. `cargo ndk -o`는 의존 크레이트의 `libroamling_core.so`도 복사하므로
쓰지 않는다. 두 UniFFI 컴포넌트는 이미 Android 라이브러리 하나에 들어 있다.

- **바인딩은 host 빌드의 라이브러리에서 뽑는다.** FFI 표면은 타겟과 무관하므로 두 호스트가 같은
  Kotlin을 낸다. `scripts/build-rust-core.sh:38-41`이 Swift 쪽에서 쓰는 방식과 같고, 이쪽은
  한 라이브러리에서 `uniffi.roamling_core`·`uniffi.roamling_android` 두 패키지를 생성한다.
  Windows host DLL에서 두 패키지가 생성되는 것은 확인했다. macOS 산출물 비교는 남아 있다.
- `rust/roamling-core/uniffi.toml`은 Kotlin에서만 `Pointer`를 `PointerInteraction`으로 이름 붙인다.
  JNA의 `Pointer` import와 충돌했기 때문이다. Rust·Swift 이름과 동작은 그대로다.
  두 크레이트의 Kotlin 설정에 `android = true`를 지정한다. 생성 파일을 직접 수정하지 않는다.
- 생성물 두 디렉터리(`android/core/src/main/kotlin`, `jniLibs`)는 **미추적**이다. Swift 쪽
  `Sources/RoamlingCoreRs`와 같은 규칙 — 빌드 산출물이므로 손으로 고치지 않는다.
- **링크 플래그는 스크립트가 아니라 `rust/.cargo/config.toml`에 둔다.** Android 15의 16 KB 페이지
  정렬(`-Wl,-z,max-page-size=16384`)은 `[target.aarch64-linux-android]`·
  `[target.x86_64-linux-android]` 섹션에 걸고, MSVC의 `+crt-static`이 그 파일에서 Windows 타겟에만
  걸려 있는 것(`rust/.cargo/config.toml:12-13`)과 같은 방식이다. 타겟 키가 격리라 세 호스트가 서로
  무관해지고, 두 스크립트가 같은 플래그를 따로 들고 다니지 않는다.
- Kotlin 바인딩은 JNA 5.19.1 AAR, coroutines 1.10.2, AndroidX annotation 1.10.0을 사용한다
  (`android/core/build.gradle.kts`). Android용 생성 코드의 `RequiresApi`도 이 의존성에서 온다.
- Gradle 9.6.1 wrapper(SHA-256 고정)와 AGP 9.4.0을 쓴다. JBR 25에서도 실행할 수 있고,
  Kotlin은 AGP 내장 지원을 사용한다. [AGP 호환표](https://developer.android.com/build/releases/agp-9-4-0-release-notes),
  [Gradle Java 호환표](https://docs.gradle.org/current/userguide/compatibility.html).
- Gradle wrapper는 커밋한다. `gradlew`는 LF여야 하므로 `.gitattributes`에 `gradlew text eol=lf`
  한 줄을 둔다 — Windows checkout이 CRLF로 바꾸면 다른 호스트에서 깨진다.
- CI는 `.github/workflows/check-android.yml` 하나를 **ubuntu-latest**에 둔다(NDK가 러너 이미지에
  있고 셋 중 제일 싸다). `cargo ndk` 빌드, 브리지 Rust 테스트, Gradle `assembleDebug`·`assembleDebugAndroidTest`·`lintDebug`, APK 안의 네이티브
  라이브러리 4개 및 16 KB ELF·ZIP 정렬을 확인한다. 에뮬레이터 실행 검사는 로컬에서 한다. 릴리스 워크플로에는 첫 판에서
  넣지 않는다. 로컬에서 Rust 표면만 빠르게 확인하려면 `rust/`에서
  `cargo check -p roamling-android`를 실행한다. NDK 링크·APK·실행 검사는 대신하지 못한다.
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
이 절은 환경 준비 당시의 설치 기록이다. 실제 앱 검증은 아래 A0 결과에 별도로 기록한다.

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

IDE 첫 실행과 실제 창에서의 조작 확인은 남아 있다. macOS 환경과 Samsung One UI 실기기는
미검증이다. A0의 Gradle·Rust 연결 작업은 환경 준비 다음 단계다.

### A0 Windows 검증 결과 — 2026-09-16, 커밋 e3a4d45 당시

`scripts/build-android-core.ps1`의 최종 경로가 host 바인딩 생성과 ARM64·x86_64 빌드를 모두 통과했다.
`android/gradlew.bat -p android :app:assembleDebug --no-daemon`으로 debug APK를 만들었다.
당시 `CoreSmokeActivity`만 debug에 있었고 사용자용 화면·런처·오버레이는 없었다. 현재 상태는 A1–A3를 따른다.

- `scripts/test-android.ps1 -Serial emulator-5580`: 설치 성공, 이번 실행 token에 대한
  `A0 PASS ... x=205.71428571428572 y=457.14285714285717 state=0` 확인.
  `defaultTuning()`과 `PetLoop`가 서로 다른 Kotlin 패키지에서 한 `.so`를 호출했다.
- 게스트 Android 17 / API 37 / 페이지 크기 16384. APK의 ARM64·x86_64별
  `libroamling_android.so`와 JNA `libjnidispatch.so` 총 4개의 ELF LOAD 정렬이 모두 `0x4000`,
  `zipalign -c -P 16 4` 통과. ARM64는 빌드·정렬만 확인했고 실제 실행은 x86_64다.
- Android `lintDebug`: 오류 0, 경고 2. 앱 아이콘과 Android 12+ data extraction 규칙은
  각각 사용자용 화면·저장 설정을 붙일 때 남은 항목이다. A0는 저장 데이터를 만들지 않는다.
- `scripts/test.ps1`: 공통 Rust·차등 fixture 통과, Windows 셸 51개 통과·기존 네트워크 검사
  1개 ignored, Windows release 빌드 성공. fixture와 `RuntimeTrace.txt`는 변경하지 않았다.
- Kotlin 전용 `uniffi.toml` 적용 전후에 같은 host DLL에서 생성한 Swift·헤더·modulemap 3개가
  바이트 단위로 동일했다. Swift 컴파일이나 macOS 실행 검증은 아니다.
- PowerShell 파서·Bash 구문·ShellCheck 및 `git diff --check` 통과.
  `.github/workflows/check-android.yml`은 추가했지만 원격 CI는 아직 실행하지 않았다.
- 검사 뒤 에뮬레이터·해당 포트가 종료되고 adb 기기 목록이 빈 것을 확인했다.
  빌드 전 실행 중이던 `%LOCALAPPDATA%\Programs\Roamling\roamling.exe` 설치본을 복원했다.

APK: `android/app/build/outputs/apk/debug/app-debug.apk` (미추적).
로그: `output/android-setup/a0-{native-final,apk-build,smoke,lint,windows-tests}.log` (미추적).

이 A0 커밋에서 생성한 Windows Kotlin SHA-256은 아래와 같다. macOS에서 같은 소스로 생성한 뒤
`shasum -a 256 android/core/src/main/kotlin/uniffi/*/*.kt`로 비교한다.

```text
roamling_android.kt  6a9c3e95115fa85a38165ae9122293830bd5c8cde074ba4e6c9512c2812f2f2e
roamling_core.kt     d43e8f8e0df9d48f26916f145ee7ab058c2b81f263b398657352c271c857117a
```

**A0 전체 게이트는 아직 열려 있다.** macOS 빌드·생성물 비교와 사용자 확인이 남았다.
Samsung One UI, ARM64 실기기와 A1의 모치 표시는 A0 검사에 포함되지 않는다.

### A1 Windows 검증 결과 — 2026-09-16

A0를 `e3a4d45`로 커밋한 뒤 사용자의 "커밋하고 다음으로 가자" 승인으로 A1을 진행했다.
A0의 macOS 항목을 완료로 간주하지 않으며, A1도 사용자 화면 확인은 남아 있다.
`MainActivity`·`MochiImages`·`MochiOverlay`가 구현 위치다. 아이콘은 기존 `assets/Roamling.ico`의
256px PNG 항목을 그대로 가져왔고, 모치 원본 그림·코어 행동·fixture·trace는 변경하지 않았다.

- `scripts/build-android-core.ps1 -Jobs 2`: host 바인딩 생성·ARM64·x86_64 네이티브 빌드 성공.
  `cargo test --locked --release -p roamling-android --jobs 2`: 2개 통과. 기존 player와 모든 capability의
  프레임 진행을 비교하고, 잘못된 입력·프레임 경계·premultiplied 픽셀을 검증한다.
- Gradle `assembleDebug`·`assembleDebugAndroidTest`·`lintDebug` 통과. lint 오류 0, 경고 1:
  `DataExtractionRules`는 저장 설정을 붙일 때 남은 항목이다. 현재 앱은 사용자 설정을 저장하지 않는다.
- Android 17 / API 37 / 16KB 페이지의 x86_64 에뮬레이터에서 `PreviewTest` 2개 통과.
  합성 픽셀의 RGBA 채널·반투명 alpha, 실제 창 표시와 버튼 상태, 두 번의 표시/숨김,
  Activity 재생성·가로 회전·홈 이동 시 창 정리를 확인했다. 세로·가로 캡처도 눈으로 확인했다.
  최초 캡처에서 발견한 창 attach 후 버튼 상태 갱신 지연을 수정하고 검사를 다시 통과했다.
  애니메이션도 창 attach 뒤 시작하도록 순서를 맞췄으며, stage 영역의 실제 화면 픽셀이
  시간에 따라 바뀌는 검사로 idle 진행을 확인한다.
- 실제 Android 설정 UI에서도 권한을 주지 않고 돌아오면 안내와 비활성 숨김 버튼을 유지하고,
  권한을 켠 뒤 돌아오면 자동으로 창이 뜨고 숨김 버튼이 활성화되는 것을 확인했다.
- A1 APK로 `scripts/test-android.ps1` 재실행: 현재 token의 `A0 PASS`와 유한 좌표 확인.
  APK의 네이티브 라이브러리 4개 모두 ELF LOAD 정렬 `0x4000`, `zipalign -c -P 16 4` 통과.
- `scripts/test.ps1` 최종 재실행 통과: 공통 Rust·차등 fixture, Windows 51개 통과·기존 1개 ignored,
  release 빌드 성공. 첫 실행은 기존 `windows_hook_shells`의 PowerShell stdin 검사에서
  비차단 소켓 `WouldBlock(10035)`로 실패했으며, 해당 코드·테스트 변경 없이 전체 검사를 한 번 재실행했다.
- 계측 검사 스크립트는 임시 overlay app-op을 실행 전 모드로 복구했다. 최종 테스트 AVD에는
  실제 설정 UI 검사에서 승인한 오버레이 권한이 남아 있다(`allow`). 앱 언어는 기존 빈 목록으로 복구했다.
  창 없는 에뮬레이터를 종료하고 원래의 설치본 데스크톱 펫을 복원했다.

재현은 부팅된 **명시적인 에뮬레이터 serial**로만 한다. 아래 검사는 두 APK를 설치하고 임시로
오버레이 app-op을 허용한 뒤 `finally`에서 앱 종료·기존 모드 복구를 수행한다.

```powershell
.\android\gradlew.bat -p android :app:assembleDebug :app:assembleDebugAndroidTest :app:lintDebug --no-daemon
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\scripts\test-android-preview.ps1 -Serial emulator-5580
```

APK: `android/app/build/outputs/apk/debug/app-debug.apk`.
캡처: `output/android-setup/a1-preview.png`, `output/android-setup/a1-preview-landscape.png`.
로그: `output/android-setup/a1-{native,bridge-tests,gradle-final,instrumentation,a0-regression,windows-tests-recheck}.log`.
산출물은 모두 미추적이다. 현 A1 소스에서 생성한 Windows Kotlin SHA-256:

```text
roamling_android.kt  6d058658ccce8fd8dde5398162147d8ed5ac3ffd0ae44a7d391abacc126c3590
roamling_core.kt     d43e8f8e0df9d48f26916f145ee7ab058c2b81f263b398657352c271c857117a
```

macOS 빌드·동일 소스의 바인딩 비교, ARM64 실행, Samsung One UI, 실기기 디코드 시간과 배터리는 미검증이다.
CI에 브리지 검사와 계측 APK 빌드를 추가했으나 원격 CI 실행은 하지 않았다.
다른 앱을 사용하는 동안의 상시 표시·걷기·드래그·잠금 뒤 자동 복귀는 A1 범위에 포함되지 않는다.

## 실기기 첫날의 요청 둘 — 키보드 회피(R17)와 크기(R18), 2026-09-19

**상태: 방향 결정됨(2026-09-19) — 키보드는 "가"(위쪽 띠로 비켜 앉기), 크기는 0.1~1.0 전 범위.** 사용자가
써 보고 이상하면 다시 말하기로 했다. 원문과 결정은 `docs/requests.md` R17 · R18.

### R17. 키보드 — 잰 것

지금 오버레이는 펫 크기의 창 하나다(`MochiOverlay.show`, `FLAG_NOT_FOCUSABLE | FLAG_WATCH_OUTSIDE_TOUCH |
FLAG_LAYOUT_IN_SCREEN`, `setFitInsetsTypes(0)`). 이 창이 키보드를 알 수 있는지 임시 로그 빌드로 쟀다
(Android 17 에뮬레이터, 설정 검색창으로 키보드를 띄우고 `dumpsys input_method`의 `mInputShown`과 대조.
계측 코드는 지웠다).

| 신호 | 키보드 닫힘 | 열림 |
|---|---|---|
| 지금 펫 창의 `rootWindowInsets.isVisible(ime())` | false | **false** |
| `WindowManager.currentWindowMetrics`의 같은 값 | false | **false** |
| 새 감지용 창(1px 폭 · 세로 전체 · 터치 안 받음) + `FLAG_ALT_FOCUSABLE_IM`의 `isVisible(ime())` | false | **true** |
| 같은 감지용 창, 그 플래그 없이 | false | false |
| 감지용 창에서 키보드 **높이** — `getInsets(ime()).bottom`, 창 높이, `getWindowVisibleDisplayFrame` | 0 | **0** (셋 다) |

- **키보드가 떠 있는지는 알 수 있다.** `FLAG_NOT_FOCUSABLE`에 `FLAG_ALT_FOCUSABLE_IM`을 더한 창만 IME 상태를
  받는다. 닫으면 false로 돌아오는 것까지 확인했다.
- **높이는 알 수 없다.** 오버레이 층은 키보드보다 위라 시스템이 inset을 0으로 준다.
  `getInsetsIgnoringVisibility(ime())`는 예외를 던진다(계측 중 앱이 그것으로 한 번 죽었다). 숨은 API는 안 쓴다.
- **삼성 키보드에서도 같다 (2026-09-19, SM-F946N · One UI 7.0 · 접은 커버 화면 904×2316).** 구현한 감지용
  창이 `dumpsys input_method`의 `mInputShown`과 같은 순간에 `keyboard visible=true/false`를 찍었다.
  펼친 화면과 분할·플로팅 키보드는 사용자가 써 보며 확인한다.

### R17. 흐름과 선택지

감지: `CompanionService`가 펫 창과 함께 감지용 창을 띄우고 닫는다 → `setOnApplyWindowInsetsListener`로
`imeVisible`의 변화를 받는다 → `MochiOverlay`에 전달. 높이를 모르므로 "키보드 바로 위까지"는 만들 수 없다.
남는 선택지는 셋이다.

- **가. 위쪽으로 비켜서 앉아 있기.** 키보드가 뜨면 보리가 갈 수 있는 영역을 화면 위쪽 띠(상태바 아래)로
  줄이고, 닫히면 원래대로 돌린다. 코어는 이미 이것을 한다 — `PetLoop.handleDisplayChange`
  (`pet_runtime.rs` `handle_display_change`)가 새 영역 안으로 위치를 **clamp**한다. 다만 지금은 **순간이동**
  이라, 걸어서 올라가게 하려면 코어에 손을 대야 한다(데스크톱과 공유하는 코드). 타이핑 중에는 배회도
  멈춘다(`setRoamingEnabled(false)`, FFI에 이미 있다).
- **나. 타이핑하는 동안 숨기.** 키보드가 뜨면 기존 숨김 경로(`MochiOverlay.pause`)로 사라지고 닫히면
  그 자리에 돌아온다. 플로팅·분할 키보드에서도 틀릴 수가 없다. 대신 타이핑 중에는 보리가 없다.
- **다. 아래쪽 고정 비율을 키보드로 가정.** 추측이라 폴드의 플로팅 키보드에서 틀리고, 폰에서는 키보드
  바로 위가 **지금 쓰고 있는 글**이라 거기로 밀어 올리면 오히려 더 거슬린다. 권하지 않는다.

**추천은 가.** 요청의 말("피해 주었으면")에 맞고, 위쪽 띠는 대개 앱 제목줄이라 쓰는 글을 가리지 않는다.
첫 판은 순간이동으로 만들어 체감을 보고, 어색하면 그때 걷기를 코어에 더한다.

### R18. 크기 — 지금과 선택지

- 크기는 `PreviewRuntime.WIDTH/HEIGHT`(96×104 dp) 상수다. 코어에는 이미 길이 있다 —
  `PetLoop.setObjectSize` / `setScale`(`pet_runtime.rs` `set_scale`, 새 크기로 위치를 clamp). 창 크기
  (`MochiOverlay.show`의 `width/height`)와 그리기(`SpriteView.onDraw`는 창 크기에 맞춰 늘린다)가 같이 따라야 한다.
- 슬라이더가 설 곳은 `MainActivity`뿐이다(유일한 화면). 값은 위치와 같은 `SharedPreferences("companion")`에
  두고, 보리가 떠 있으면 움직이는 동안 바로 반영한다.
- **하한이 문제다.** 1 dp는 1/160 인치다.

  | 배율 | 크기 | 실제 |
  |---|---|---|
  | 1.0 | 96×104 dp | 약 15×16.5 mm |
  | 0.5 | 48×52 dp | 약 7.6 mm — Android가 권하는 최소 터치 크기(48 dp)와 같다 |
  | 0.3 | 29×31 dp | 약 4.6 mm |
  | 0.1 | 10×10 dp | **약 1.5 mm** — 보이지 않고 잡을 수 없다 |

  창이 곧 터치 영역이라 작아질수록 잡기도 같이 어려워진다. 터치 영역만 크게 두면 그 투명한 여백이 밑의
  앱 터치를 가로채므로 하지 않는다.
- 걷는 속도는 크기와 무관한 dp/s라, 작게 하면 몸에 비해 빨리 걷는 것처럼 보인다. 첫 판은 그대로 두고 본다.
- 1배 미만에서는 픽셀 아트가 비정수 배율로 줄어 거칠어진다. 줄일 때만 bilinear 필터를 켠다.

### R17 · R18 구현과 확인 (2026-09-19)

**키보드.** `MochiOverlay.watchKeyboard`가 펫 창과 같이 감지용 창을 띄우고 `pause`에서 같이 내린다.
`onImeChanged` → `applyWorld`가 코어의 세계를 바꾼다: 키보드가 있으면 상태바 아래의 띠(펫 키의 1.5배와
쓸 수 있는 화면의 28% 중 큰 쪽 — `KEYBOARD_BAND_BODIES`, `KEYBOARD_BAND_SHARE`), 없으면 전체.
`PreviewRuntime.setWorld`는 `PetLoop.handleDisplayChange`(그 영역 안으로 clamp, 순간이동)와
`setRoamingEnabled`를 부른다 — 띠 안에서는 앉아 있고, 키보드가 내려가면 다시 돌아다닌다. Kotlin에는
"어디로 갈지"가 없다. 세계의 모양만 알려 주고 나머지는 코어가 한다.

**크기.** `PreviewRuntime`의 `WIDTH/HEIGHT` 상수가 `BASE_WIDTH/BASE_HEIGHT × scale`이 됐다(0.1~1.0,
`clampScale`). `MainActivity`의 바(0.01 단위 90칸) → `CompanionService.setScale`이 `SharedPreferences`
`"scale"`에 적고 `MochiOverlay.resize`를 부른다 → 코어 `setScale`이 새 몸집으로 위치를 다시 clamp하고 창
크기가 따라간다. 1배 미만에서는 `SpriteView.smooth`로 bilinear 필터를 켠다. 보리가 떠 있는 동안 바를
움직이면 바로 반영된다.

**실기기 확인 (SM-F946N).**

| 한 것 | 결과 |
|---|---|
| 설정 검색창으로 키보드를 띄움 | 로그 `keyboard visible=true`, 보리 창 y 1802 → **401**(위쪽 띠), 그대로 앉아 있음 |
| 키보드를 닫음 | `keyboard visible=false`. 화면을 터치하는 동안 띠 밖으로 나와 다시 돌아다님 |
| 닫은 뒤 1분간 제자리였던 것 | 버그가 아니다 — adb의 키 입력만 있고 **터치가 없어서** 코어가 자리 비움으로 보고 재웠다. 터치하자 움직였다 |
| 바를 네 군데 탭 | 1.00 → 0.55 → 0.28 → 0.10 → 1.00, 창 크기 252×273 → 139×150 → 71×76 → 25×27 px |
| 0.55로 두고 앱을 강제 종료 후 다시 실행 | 바와 보리 둘 다 0.55로 돌아옴 |

크래시 로그 없음. 에뮬레이터 계측은 새 테스트 `sizeIsClampedAndTheKeyboardBandHoldsTheCompanionStill`
(`PreviewTest.kt`)을 더해 6개 통과, `lintDebug` 통과.

**아직 모르는 것.** 0.1~0.3배에서 실제로 잡을 수 있는지, 걷는 속도가 몸집에 비해 어색한지, 순간이동이
거슬리는지, 폴드를 펼친 화면과 플로팅 키보드에서의 동작 — 전부 사용자가 써 보고 말해 주기로 했다.

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

- macOS에서 같은 소스의 빌드·Kotlin 바인딩 일치 확인 (A0). Windows의 두 컴포넌트 생성·실행과
  cargo-ndk의 16 KB rustflags 반영은 위 결과로 확인했다.
- Windows에서 NDK 경로의 공백·한글 문제. 현재 `C:\Android\sdk` 경로만 실측했다.
- premultiplied RGBA8의 Android `Bitmap` 채널·alpha 처리는 A1 합성 픽셀 테스트와 화면으로 확인했다.
- `image` 크레이트의 WebP 디코드가 휴대전화 CPU에서 얼마나 걸리는지 (A1). **데스크톱 수치를
  그대로 옮기지 않는다** — 맥에서 잰 것은 시트 한 장 디코드 24.09 ms(Rust native)이고,
  거기에 uniffi를 건너면 39.37 ms가 된다. **그 차이 15.3 ms는 디코드가 아니라 11.5 MB가 FFI를
  건너는 비용**이다(`docs/history/windows.md:2313-2317`). Android도 RGBA를 Kotlin에 넘기므로
  둘을 따로 재야 한다.
- Android 14 `specialUse` 서비스 타입이 sideload에서 걸리는 것이 없는지 (A3).
- 제조사 배터리 최적화가 foreground service를 죽이는지. 특히 Samsung. 실기기로 본다 (A3).
- 손가락이 사라진 뒤의 "포인터 위치"가 실제로 어떻게 느껴지는지 (A2).
- 잠금화면 위젯의 기기·버전별 지원 (A5).
