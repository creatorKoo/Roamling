# Roamling upstream research

조사 기준일: **2026-08-26**. README만으로 형식을 추정하지 않고 공개 소스,
설치된 공식 리소스, 생성된 현재 버전 schema를 함께 확인했다. upstream은 계속
변하므로 아래 commit과 날짜는 compatibility fixture를 갱신할 때의 기준점이다.

## Local development environment

- 시작 상태: 파일과 Git metadata가 없는 빈 directory. 새 `main` branch Git
  repository로 초기화했다.
- host: Apple Silicon, macOS 26.5.2.
- compiler: Apple Swift 6.3.3, Command Line Tools only. full Xcode는 설치되어 있지 않다.
- 이 host의 기본 `MacOSX26.5.sdk`는 Swift 6.3.2로 만들어져 compiler 6.3.3과
  mismatch였다. 설치된 `MacOSX15.4.sdk`를 명시해 debug/release compile, tests,
  bundle을 검증했다. 이는 source 문제가 아니라 로컬 CLT 설치 일관성 문제다.
- release helper는 local ad-hoc signing까지 수행한다. Developer ID signing,
  notarization, universal binary archive는 full Xcode/배포 identity가 필요한 release
  engineering 작업으로 남는다.

## 결론 요약

- Petdex는 별도의 동작 runtime이라기보다 Codex pet package의 설치/배포
  ecosystem으로 취급하는 편이 맞다. Roamling은 gallery/backend를 복제하지 않는다.
- 현재 호환 표면에는 적어도 두 종류가 공존한다. v1은 8×9, v2는 8×11이며
  v2의 마지막 두 행은 16방향 시선 pose다.
- 공개 Codex TUI 소스에는 v1 고정 atlas 검사가 남아 있는 반면, 현재 ChatGPT
  데스크톱 앱의 공식 `hatch-pet` 계약은 v2를 요구한다. 한 구현의 내부 상수를
  “Petdex 규격 전체”로 간주하면 안 된다.
- Petdex의 package source asset은 제출자가 소유하거나 라이선스한다. Petdex
  코드의 MIT license가 gallery asset에 자동 적용되지 않는다. 따라서 Roamling은
  license가 확인되지 않은 pet을 기본 번들에 재배포하지 않는다.
- macOS에서는 모니터 전체 크기 overlay보다 sprite 크기의 비활성 `NSPanel`이
  간섭과 전력 측면에서 낫다. 평상시 `ignoresMouseEvents = true`, 잡기 가능한 짧은
  구간과 drag 동안만 입력을 켠다.
- Claude Code는 공식 lifecycle hook이 가장 안정적인 integration surface다.
  Codex는 app-server JSON-RPC notification/server-request schema를 우선 검토하되,
  설치된 Codex 버전에서 schema를 생성하여 맞춰야 한다.

## OpenAI Codex pets

### 사용자 기능

OpenAI의 현재 [Pets 문서](https://learn.chatgpt.com/docs/pets)는 desktop pet이 다른
창 위에 떠 있고, `/pet`으로 깨우며, 위치를 기억하고, 상태 우선순위를 `Needs input
> Blocked > Ready > Running`으로 처리한다고 설명한다. built-in과 local custom pet,
reduced-motion 대응도 명시한다.

### 공식 v2 package 계약

설치된 ChatGPT 앱 `26.814.41957`의 공식 curated resource에서 다음 파일을 직접
확인했다.

```text
/Applications/ChatGPT.app/Contents/Resources/skills/skills/.curated/hatch-pet/
├── references/codex-pet-contract.md
└── references/animation-rows.md
```

최소 manifest는 다음과 같다.

```json
{
  "id": "pet-name",
  "displayName": "Pet Name",
  "description": "One short sentence.",
  "spriteVersionNumber": 2,
  "spritesheetPath": "spritesheet.webp"
}
```

| version | atlas | grid | cell | 의미 |
| --- | --- | --- | --- | --- |
| 생략 또는 1 | 1536×1872 | 8×9 | 192×208 | 기존 9개 표준 state |
| 2 | 1536×2288 | 8×11 | 192×208 | 표준 state + 16방향 look |

`spriteVersionNumber`를 생략하면 v1로 해석된다. 따라서 2288px atlas를 version 없이
추측해 v2로 읽지 않는다. 잘못 잘린 pet보다 명확한 validation error가 안전하다.

### 표준 animation

| row | canonical name | frames | frame timing |
| ---: | --- | ---: | --- |
| 0 | `idle` | 6 | 280, 110, 110, 140, 140, 320 ms |
| 1 | `running-right` | 8 | 120 ms, 마지막 220 ms |
| 2 | `running-left` | 8 | 120 ms, 마지막 220 ms |
| 3 | `waving` | 4 | 140 ms, 마지막 280 ms |
| 4 | `jumping` | 5 | 140 ms, 마지막 280 ms |
| 5 | `failed` | 8 | 140 ms, 마지막 240 ms |
| 6 | `waiting` | 6 | 150 ms, 마지막 260 ms |
| 7 | `running` | 6 | 120 ms, 마지막 220 ms |
| 8 | `review` | 6 | 150 ms, 마지막 280 ms |
| 9–10 | look poses | 16 | 000°부터 22.5° 간격, 시계 방향 |

000°는 neutral이 아니라 위쪽이다. neutral/deadzone은 `idle`로 돌아간다.

이 표의 `frames`는 동일 sprite를 runtime에서 흔드는 횟수가 아니라 pet 제작자가 채우는
서로 다른 atlas cell이다. Petdex의 현재
[`pet-states.ts`](https://github.com/crafter-station/petdex/blob/main/src/lib/pet-states.ts)도
idle을 breathing/blinking loop, 좌우 running row를 directional locomotion으로 설명한다.
따라서 다리 교차, 체중 이동, 꼬리 지연 같은 자연스러운 동작은 규격 밖 기능이 아니라
각 frame artwork에 들어가야 하는 표준적인 제작 책임이다. Roamling이 임시 key pose에
transform만 주었을 때 떠 보였던 것은 Petdex 제약이 아니라 built-in audition asset의
한계였다.

### 공개 Codex source에서 추가로 확인한 custom surface

[Codex `model.rs`](https://github.com/openai/codex/blob/main/codex-rs/tui/src/pets/model.rs)는
선택적인 `frame { width, height, columns, rows }`와 `animations` map을 decode한다.
각 animation은 frame index 목록, `fps`, `loop`, `fallback`을 가질 수 있고 기본
animation 위에 병합된다. path traversal, atlas 밖 frame, 60 초과 FPS, 지나치게 많은
frame은 거부한다. 이름 alias(`move_right`, `move_left`, `wave`, `bounce`, `sad`)도 있다.

반면 같은 시점의 [`catalog.rs`](https://github.com/openai/codex/blob/main/codex-rs/tui/src/pets/catalog.rs)와
[`asset_pack.rs`](https://github.com/openai/codex/blob/main/codex-rs/tui/src/pets/asset_pack.rs)는
여전히 8×9/1536×1872 built-in 경로와 4 MiB download limit를 사용한다. 이 차이는
미공개 동작을 추측해서 맞출 일이 아니라, Roamling loader가 명시적 v1/v2 및 manifest
override를 순서대로 해석해야 한다는 근거다.

OpenAI Codex repository source는
[Apache-2.0](https://github.com/openai/codex/blob/main/LICENSE)이다. Roamling은 해당
runtime code나 built-in asset을 복사하지 않고 공개 package contract만 독립적으로
구현한다.

## Petdex

조사 commit:
[`c7fbe8a9`](https://github.com/crafter-station/petdex/tree/c7fbe8a9c9c45900e98dacfaad4f41627e2c760a)
(2026-08-21).

### package와 검증

Petdex package root는 `pet.json`과 그 manifest가 가리키는 PNG/WebP atlas다.
[`sprite-version.ts`](https://github.com/crafter-station/petdex/blob/c7fbe8a9c9c45900e98dacfaad4f41627e2c760a/src/lib/sprite-version.ts)는
version 1/2만 허용하고 생략을 v1로 처리한다.
[`pet-states.ts`](https://github.com/crafter-station/petdex/blob/c7fbe8a9c9c45900e98dacfaad4f41627e2c760a/src/lib/pet-states.ts)는
위 9개 row와 timing을 canonical table로 둔다.
[`submissions-validation.ts`](https://github.com/crafter-station/petdex/blob/c7fbe8a9c9c45900e98dacfaad4f41627e2c760a/src/lib/submissions-validation.ts)는
8×9 또는 8×11의 정확한 비율과 trusted asset URL 등을 검사한다.

### public API

- `GET /api/manifest`: cacheable `petdex-v1.json`으로 307 redirect. 각 pet에 slug,
  display name, kind, submitter, spritesheet/pet.json/zip URL, sprite version이 있다.
- `GET /api/manifest/v2`: tuple 기반 compact snapshot. `assetBase`와 `fields`가 있어
  payload 크기를 줄인다.
- `GET /api/manifest/full`: 더 풍부하지만 인증/rate limit가 있는 surface다.

실제 구조는 [`public-manifest.ts`](https://github.com/crafter-station/petdex/blob/c7fbe8a9c9c45900e98dacfaad4f41627e2c760a/src/lib/public-manifest.ts)와
[route source](https://github.com/crafter-station/petdex/tree/c7fbe8a9c9c45900e98dacfaad4f41627e2c760a/src/app/api/manifest)에서 확인했다.
slim manifest 자체는 animation row를 반복하지 않는다. client는 `pet.json`을 받고
표준 계약을 적용해야 한다.

Petdex CLI는 gallery asset을 내려받아 `~/.petdex/pets/<slug>`와
`~/.codex/pets/<slug>`에 함께 설치한다. import 가능한 안정 SDK라기보다 CLI이므로,
Roamling runtime은 이 두 directory를 read-only discovery location으로 취급한다.

### Petdex Desktop

[`sprite.zig`](https://github.com/crafter-station/petdex/blob/c7fbe8a9c9c45900e98dacfaad4f41627e2c760a/packages/petdex-desktop-native/src/sprite.zig)는
9개 state row/timing을 native Zig로 구현한다. desktop runtime은 atlas의 8×9/8×11
aspect를 판별하고, 100 ms state polling, 250 ms minimum dwell, app-owned drag와 momentum,
작은 transparent window를 사용한다. local state endpoint는
`127.0.0.1:7777/state`이며 per-boot token을
`~/.petdex/runtime/update-token`에 mode 0600으로 둔다. 자세한 사용자 계약은
[Petdex docs](https://petdex.dev/docs)에 있다.

재사용할 아이디어는 표준 timing, 짧은 dwell, sprite에 맞춘 작은 window다. Roamling은
해당 state endpoint에 결합하지 않고 나중에 하나의 optional `ActivitySource` adapter로만
취급한다.

### license

Petdex source는 [MIT](https://github.com/crafter-station/petdex/blob/main/LICENSE)다.
하지만 submission type에는 source/author 정보가 별도로 있고 gallery asset은 각
제출자가 소유하거나 제공한 license를 따른다. 그러므로:

- package를 읽어 실행하는 호환성은 제공한다.
- 사용자가 설치한 asset의 license를 Roamling license로 바꾸지 않는다.
- license가 확인되지 않은 gallery asset을 repository나 release에 복사하지 않는다.

## Existing desktop companion projects

소스 코드를 복사하지 않고 책임 분리와 실패 지점을 조사했다.

### Clawd on Desk

조사 commit:
[`c103b4b9`](https://github.com/rullerzhou-afk/clawd-on-desk/tree/c103b4b945004917d08840bb714444edb396574f).

Electron 기반이며 render window와 별도의 작은 hit window를 두고, hit geometry의
single writer가 `setIgnoreMouseEvents`를 제어한다. Codex pet importer, 다수 agent hook,
permission bubble 등 성숙한 상태 integration이 있지만 roaming보다 agent status UI에
초점이 있다. 별도 hit surface와 단일 입력 상태 소유권은 유용한 교훈이다. 프로젝트는
AGPL-3.0-only이므로 코드를 가져오지 않는다.

### AgentPet

조사 commit:
[`4fd4167a`](https://github.com/ntd4996/agentpet/tree/4fd4167aba89a0f2a0d0a7015b970ed5e6884740).

macOS는 Swift/SwiftUI `NSPanel`, Windows/Linux는 Tauri다. 여러 agent와 project별
pet, display change 시 위치 clamp를 구현한다. macOS panel은 borderless,
non-activating, floating, all-Spaces/fullscreen auxiliary다. source는 MIT다. 다만
창 전체를 drag surface로 쓰는 corner companion이므로 Roamling의 평상시 click-through
요구에는 별도 input gating이 필요하다.

### CodePet (JellyTony)

조사 commit:
[`ed736d7f`](https://github.com/JellyTony/codepet/tree/ed736d7f00d05945a50fcefa3f0c2021c2dd2260).

native Swift/AppKit `NSPanel`과 Petdex/Codex 9-row loader를 사용한다. Claude Code의
고빈도 이벤트는 loopback HTTP hook, `SessionStart`는 terminal identity를 얻는 command
hook으로 나눈다. MIT source다. transcript에서 title/summary를 얻는 기능은 Roamling의
초기 privacy 원칙(내용을 읽지 않음)과 맞지 않아 재사용하지 않는다.

### CodePet (seongwon)

조사 commit:
[`783787ca`](https://github.com/seongwon-kim/codepet/tree/783787cae078385a5a5e51740a64253293fcdb88)
(repository 이름 충돌 때문에 owner를 함께 기록한다).

Electron이 display마다 full-size transparent overlay를 만들고 40 ms cursor poll로
pet rectangle 위에서만 input을 켠다. 여러 monitor에서 동작하지만 full-display window와
지속 polling 비용이 Roamling 목표와 맞지 않는다. package에는 명시적 source license가
없고 asset은 attribution 조건이 있으므로 코드/asset을 가져오지 않는다.

## Claude Code integration

Anthropic의 공식 [Hooks reference](https://code.claude.com/docs/en/hooks)를 기준으로
하고 설치된 `claude 2.1.227`도 확인했다. 현재 terminal, IDE, desktop, web에서
lifecycle hook을 지원하며 주요 event는
다음과 같다.

- `SessionStart`, `SessionEnd`, `UserPromptSubmit`
- `PreToolUse`, `PostToolUse`, `PostToolUseFailure`
- `PermissionRequest`, `Notification`
- `Stop`, `StopFailure`

command hook은 JSON stdin을 받고, HTTP hook은 같은 JSON을 POST한다. HTTP handler는
`url`, optional `headers`, `allowedEnvVars`, timeout을 지원한다. 빈 2xx response는 성공이고
HTTP non-2xx, connection failure, timeout은 agent action을 block하지 않는다. 현재 common
payload에는 `session_id`, Claude Code 2.1.196+의 optional `prompt_id`, `cwd`,
`hook_event_name` 등이 있다. Roamling은 session/event name과 notification 분류만 decode해
다음처럼 normalize한다.

```text
PreToolUse              -> activityStarted, low/medium intensity
PermissionRequest       -> attentionRequired
PostToolUse             -> positive, low intensity
PostToolUseFailure      -> setback
Stop                    -> achievement
StopFailure             -> negative
SessionEnd              -> activityEnded
```

MVP 1 구현은 explicit opt-in으로 설치되는 idempotent hook + token으로 보호된
`127.0.0.1` HTTP receiver를 선택했다. prompt, tool input/output, source, transcript path는
decode model, metadata, disk, log에 넣지 않는다. request는 memory에서 lifecycle field만
읽은 뒤 폐기한다. hook installer는 기존 설정을 덮어쓰지 않고 one-time backup과 자기
handler만 제거하는 경로를 제공한다. user-level hook은 `~/.claude/settings.json`에 있으며
`~/.claude.json`은 hook 설정 위치가 아니다.

## Codex activity/events

조사 기준은 현재 설치된 `codex-cli 0.147.0`과 정확히 대응하는 OpenAI Codex
[`rust-v0.147.0`](https://github.com/openai/codex/tree/rust-v0.147.0) source다. 로컬
`codex features list`에서 `hooks`가 `stable: true`임을 확인했다.

### Stable hook surface

[`hooks/src/lib.rs`](https://github.com/openai/codex/blob/rust-v0.147.0/codex-rs/hooks/src/lib.rs)는
이 버전이 다음 11개 event를 registry에 제공한다고 선언한다.

- `PreToolUse`, `PermissionRequest`, `PostToolUse`
- `PreCompact`, `PostCompact`
- `SessionStart`, `SessionEnd`, `UserPromptSubmit`
- `SubagentStart`, `SubagentStop`, `Stop`

user-level JSON은 `~/.codex/hooks.json`이며 shape는 다음과 같다. exact config type은
[`config/src/hook_config.rs`](https://github.com/openai/codex/blob/rust-v0.147.0/codex-rs/config/src/hook_config.rs)의
`HooksFile`, `MatcherGroup`, tagged `HookHandlerConfig`에서 확인했다.

```json
{
  "hooks": {
    "SessionStart": [{
      "hooks": [{
        "type": "command",
        "command": "...",
        "timeout": 2,
        "statusMessage": "Notifying Roamling"
      }]
    }]
  }
}
```

command handler는 event JSON을 stdin으로 받는다. OpenAI의
[`core hook integration tests`](https://github.com/openai/codex/blob/rust-v0.147.0/codex-rs/core/tests/suite/hooks.rs)도
user home의 `hooks.json`을 만들고 Python command가 `json.load(sys.stdin)`으로 읽는 경로를
검증한다. payload schema/source에서 확인한 공통 관심 field는 `session_id`, optional
`turn_id`, `hook_event_name`이다. event에 따라 cwd, transcript path, prompt, model,
tool input/output, last assistant message도 포함되지만 Roamling은 이를 decode하지 않는다.

MVP 2는 11개를 전부 등록하지 않고 companion state가 달라지는 7개만 쓴다.

```text
SessionStart / UserPromptSubmit -> activityStarted
PreToolUse                     -> activityStarted (work intensity)
PostToolUse                    -> positive (routine, no visible reaction)
PermissionRequest              -> attentionRequired
Stop                           -> achievement
SessionEnd                     -> activityEnded
```

Codex hook은 새롭거나 변경되면 startup trust review를 요구한다. Roamling은 이 승인을
자동화하거나 `--dangerously-bypass-hook-trust`를 사용하지 않는다. installer success 안내에서
Codex를 restart하고 trust prompt를 승인하도록 명시한다.

### Rejected primary transports

공식 [`codex app-server` README](https://github.com/openai/codex/blob/rust-v0.147.0/codex-rs/app-server/README.md)는
JSON-RPC transport와 rich turn/item/approval event를 제공한다. 그러나 Roamling이 시작하지
않은 기존 CLI/Desktop session에 안정적으로 observer로 attach하는 public contract는 아니다.
Roamling이 agent runtime을 소유하게 되므로 기본 desktop companion transport로 선택하지
않았다.

legacy `notify`는 turn completion 위주이고 사용자 `~/.codex/config.toml`에 이미 다른
command가 설정되어 있을 수 있다. 실제 개발 환경에도 기존 `notify` key가 있었으므로
Roamling은 이를 읽어 복제하거나 덮어쓰지 않는다. 별도 `hooks.json` 병합이 기존 설정과
안전하게 공존한다.

### Architectural implications

- Codex hook schema는 version과 함께 변할 수 있어 adapter에만 event name을 둔다.
- unknown/unused lifecycle event는 무시한다.
- source-specific payload는 receiver 직후 폐기하고 core에는 `CompanionEvent`만 전달한다.
- hook install/repair/remove는 idempotent하며 Roamling marker가 있는 handler만 수정한다.
- transport 실패는 shell에서 성공으로 바꿔 agent work를 절대 block하지 않는다.

## macOS platform APIs

### overlay와 click-through

Apple의 [`NSWindow.ignoresMouseEvents`](https://developer.apple.com/documentation/appkit/nswindow/ignoresmouseevents)는
window를 mouse event에 투명하게 만든다. Roamling MVP는 다음 조합을 사용한다.

- `NSPanel`, `.borderless`, `.nonactivatingPanel`
- clear background, `isOpaque = false`, no shadow
- `.canJoinAllSpaces`, `.fullScreenAuxiliary`, `.stationary`, `.ignoresCycle`
- sprite 크기의 window 한 개
- 평상시 `ignoresMouseEvents = true`
- 빠른 접근으로 catch가 armed되고 pointer가 pet hit region 안에 있을 때만 `false`
- drag 중에는 상태를 고정하고 mouse-up 후 즉시 click-through 복원

`NSView.hitTest`만으로는 window 아래 application까지 event를 전달할 수 없다. window가
hit된 뒤 nil을 반환하면 같은 앱 내부의 다른 view를 찾을 뿐이다. 그래서 per-pixel에
가까운 view hit region과 window-level `ignoresMouseEvents` gating이 둘 다 필요하다.

### pointer

[`NSEvent.mouseLocation`](https://developer.apple.com/documentation/appkit/nsevent/mouselocation)은
pending event와 무관한 global screen coordinate를 제공하고,
`NSEvent.pressedMouseButtons`로 drag 상태를 확인할 수 있다. MVP는 Accessibility나
Input Monitoring permission이 필요한 global event tap을 만들지 않는다. 움직이는 동안
30 Hz, 평상시 낮은 cadence로 위치를 sample한다.

### multiple displays와 coordinates

[`NSScreen`](https://developer.apple.com/documentation/appkit/nsscreen)은 `frame`,
`visibleFrame`, scale과 display id를 제공한다. `visibleFrame`은 Dock/menu bar/camera
housing을 제외한 현재 안전 영역이며 cache하면 안 된다. display 구성 변경은
[`NSApplication.didChangeScreenParametersNotification`](https://developer.apple.com/documentation/appkit/nsapplication/didchangescreenparametersnotification)으로
받는다.

AppKit screen 좌표는 bottom-left/y-up이고 배치에 따라 음수 origin이 가능하다.
Roamling core는 top-left/y-down world 좌표를 사용한다. 모든 screen frame의
`maxY`를 `worldTop`으로 잡고 다음 affine transform 하나만 platform 경계에서 적용한다.

```text
core.x = appKit.x
core.y = worldTop - appKit.y

coreRect.y = worldTop - appKitRect.maxY
```

Retina backing pixel과 logical point를 섞지 않는다. layout/movement는 point 단위이고
atlas crop만 image pixel이다. Apple도 일반 layout에는
[`backingScaleFactor`](https://developer.apple.com/documentation/appkit/nsscreen/backingscalefactor)보다
coordinate conversion API 사용을 권한다.

### Accessibility, focus, caret

Accessibility는 MVP 0/0.5 필수 권한이 아니다. 허용된 경우 system-wide element에서
focused application/window/UI element를 찾고, selected text range가 있으면
[`kAXBoundsForRangeParameterizedAttribute`](https://developer.apple.com/documentation/applicationservices/kaxboundsforrangeparameterizedattribute)로
화면 bounds를 얻을 수 있다. 모든 control이 이 attribute를 구현하는 것은 아니며
secure field/브라우저/terminal별 차이가 있으므로 optional hint다.

신뢰 여부는
[`AXIsProcessTrustedWithOptions`](https://developer.apple.com/documentation/applicationservices/1459186-axisprocesstrustedwithoptions)로
검사하되 앱 시작마다 prompt하지 않는다. focused/caret lookup은 event 또는 coarse
debounce에 반응해서 수행하고 frame마다 AX tree를 순회하지 않는다.

### ScreenCaptureKit

[ScreenCaptureKit](https://developer.apple.com/documentation/screencapturekit)은 display,
application, window 단위 filter를 제공하고
[`SCScreenshotManager`](https://developer.apple.com/documentation/screencapturekit/scscreenshotmanager)로
single frame을 캡처할 수 있다. Screen Recording permission과 usage description이
필요하다. VisualSafeZoneProvider는 continuous stream이 아니라 placement가 정말 필요할
때 downsampled single-frame만 처리한다. image는 메모리에서 폐기하고 disk/network/
telemetry로 보내지 않는다. permission이 없으면 BasicSafeZoneProvider와 corner
fallback을 쓴다.

## 무엇을 재사용하고, 무엇을 직접 구현하는가

### 재사용 가능한 public contract

- Codex/Petdex manifest field와 v1/v2 atlas layout
- canonical state timing과 animation alias/fallback 개념
- Petdex가 이미 설치한 local package directory
- Claude Code 공식 hook event와 Codex stable hook/app-server protocol
- AppKit/ApplicationServices/ScreenCaptureKit public API

### Roamling이 직접 구현할 것

- global desktop world와 display topology/path planning
- wander/evade/catch/drag FSM과 movement tuning
- `PetCapabilities`와 graceful animation resolver
- `ActivitySource -> CompanionEvent -> AttentionModel -> ReactionPolicy`
- safe-zone scoring 및 permission-degraded placement
- native small overlay/input gating

### 피할 것

- Petdex gallery/backend fork 또는 undocumented database dependency
- v1의 8×9 상수를 전체 ecosystem 규격으로 간주
- gallery asset을 source license만 보고 재배포
- Claude/Codex enum과 payload를 BehaviorController에 전달
- transcript/prompt/source content 분석
- full-display interactive overlay, continuous capture/OCR/AX traversal
- 기존 hook/notify config를 무조건 덮어쓰기
- AGPL code 복사 또는 license가 불명확한 CodePet code/asset 재사용

## 불안정한 dependency와 대응

| surface | 위험 | 대응 |
| --- | --- | --- |
| Codex v1/v2 runtime 차이 | 공개 client 사이 지원 시점 차이 | manifest version을 명시적으로 해석하고 fixture test |
| custom `animations` | 아직 널리 쓰이지 않는 선택 surface | unknown key 보존 불필요, invalid track만 격리/fallback |
| Petdex API redirect/snapshot | cache/CDN schema 변경 가능 | API는 installer에만 사용, runtime은 local package 중심 |
| Codex app-server | generated schema가 버전별 변경 | capability negotiation, unknown event 무시 |
| Codex command hooks | event/config/trust contract가 버전별 변경 가능 | 설치된 CLI tag source 확인, 최소 event만 등록, opt-in repair/remove |
| Claude hooks | event 추가/field 변경 | event name + 최소 식별자만 consume |
| AX caret | 앱별 지원 편차 | confidence가 낮으면 focused window/corner fallback |
| ScreenCaptureKit | permission/OS version | opt-in provider, basic mode가 항상 완전 동작 |

## Architecture implications

1. `RoamlingCore`는 AppKit, Accessibility, Claude, Codex를 import하지 않는다.
2. pet loader는 format parser이고 renderer/behavior를 모른다.
3. standard capability를 먼저 resolve하고 missing animation은 pet 전체 실패가 아니라
   deterministic fallback으로 처리한다.
4. display refresh 시 platform snapshot을 통째로 교체하고 core path를 re-plan한다.
5. input mode의 writer는 overlay controller 하나뿐이다.
6. future Windows port는 같은 domain value를 Win32/UIAutomation adapter로 채우면 된다.
   지금 Rust/FFI를 만들 이유는 없다.
