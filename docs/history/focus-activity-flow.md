# 일하는 앱 흐름: source · director · 펫 런타임이 어떻게 맞물리는가

> **이 문서는 2단계 이전 구조를 설명한다.** 2026-09-12에 일하는 앱이 상태형 source로 옮겨가면서
> 여기 적힌 우회 코드는 전부 사라졌다 — `HEARTBEAT` 재발신, `BLOCKED_RESEND`, `news_owed`,
> `Greeting` 박자, `Goodbye` 붙잡기, 셸이 넘기던 펫의 사실 넷. 무엇이 왜 없어졌는지는
> `docs/state-sources.md` §8.1에 있고, 지금 도는 구조는 같은 문서의 §3~§7이다. 이 문서는
> **그 우회 코드가 왜 생겼는지의 기록**으로 남긴다 — 새 구조를 읽는 근거로 쓰지 않는다.
> (§6의 리팩터 후보 R1~R6은 2단계가 대부분 흡수했다.) 다시 쓰는 것은 실사용 확인 뒤다.

기준 커밋 `e64584e`. 줄 번호는 전부 이 커밋의 것이고, 줄은 썩으므로 함수·상수 이름을 먼저 쓴다.
이 문서는 **코드를 읽고 확인한 것만** 적는다. 계획서·진행 노트·`CLAUDE.md`·다른 docs의 문장은 근거로
쓰지 않았다. 코드와 다른 문서 문장은 §7에 모았고, 확인하지 못한 것은 그 자리에 "확인 못 함"이라고 적었다.
리팩터링 후보(§6)는 결정이 아니다.

경로 약칭:

| 약칭 | 경로 |
|---|---|
| `fa` | `rust/roamling-core/src/focus_activity.rs` |
| `ad` | `rust/roamling-core/src/activity_director.rs` |
| `att` | `rust/roamling-core/src/attention.rs` |
| `act` | `rust/roamling-core/src/activity.rs` |
| `beh` | `rust/roamling-core/src/behavior.rs` |
| `pr` | `rust/roamling-core/src/pet_runtime.rs` |
| `pl` | `rust/roamling-core/src/placement.rs` |
| `RR` | `Sources/RoamlingEngine/RoamlingRuntime.swift` |
| `FT` | `Tests/RoamlingLogicTests/FocusActivityLogicTests.swift` (Swift 하네스) |

## 1. 파이프라인 한 장

```text
[macOS 셸, 매 틱]  RR tick() 408
 │
 ├─① RR sampleWorkingApplication 540 — 앞 샘플 뒤 0.5초 이상 지난 첫 틱에서만 (541, focusSampleInterval 109)
 │     FocusActivity::observe(fa 324)에 입력 일곱
 │       app              MacWindowProvider.frontmostApplicationIdentifier — 자기 프로세스면 nil
 │       watched          workApps.contains(app) — 설정 roamling.workApps
 │       seconds_since_key MacUserIdleProvider.keyboardIdleDuration — keyDown만, 0.5초 캐시
 │       dispatched_event PetRuntime::last_dispatched_activity_id (pr 293)
 │       arrival_pending  PetRuntime::has_arrival_reaction (pr 286)
 │       pet_resting      PetRuntime::is_resting (pr 301)
 │       agent_on_duty    PetRuntime::agent_on_duty (pr 310)
 │     → CompanionEvent 목록: id "focus:<app>:<n>", source_type System, context Working, 창 없음 (fa event 793)
 │
 ├─② RR handleActivityEvent 571 — 창이 없고 wants_window_hint(kind)면 앞 창을 채움 (576-579)
 │     → PetRuntime::handle_activity_event (pr 757) → ActivityDirector::handle_event (ad 121)
 │         ActivityEnded·Idle → recent 제거 · attention clear · 좌석 주인이면 좌석 비움+Calm · 다음 후보 큐 (136-150)
 │         그 밖 → recent에 넣고 → candidates(출처 거름) → attention.select → 마지막 dispatch id면 끝 (153-163)
 │           잡힌 펫 → pending (165-168) / 쉬는 펫 → 깨우는 kind만 CancelRest+pending, 나머지는 버림 (169-176)
 │           → dispatch (252): kind별 begin_watching(지속 반응, 도착 반응) 또는 좌석 비움+반응
 │
 ├─③ PetRuntime::begin_tick (pr 476, RR 415)
 │     behavior Tick(순간 상태 만료, 481) → expire_silent(482) → resume_pending_if_ready(상태가 Idle일 때만, 488-495)
 │
 ├─④ PetRuntime::finish_tick (pr 501, RR 422)
 │     user_idle < 0.8이고 쉬는 중이면 깨움 (509-511)
 │     placement.decide (552) → 분기(잡힘·회피·휴식·착지·커서·그 밖) → apply_intent (979)
 │       Travel → travel_to_seat (1008)
 │       Hold · SleepInPlace · None + 지켜보는 창 있음 → hold_seat (1032):
 │         도착 반응이 있으면 deliver_arrival_reaction, 없으면 상태에 따라 sustain_on_seat
 │     이 틱에 걷기가 끝났고 hold_seat가 안 입혔으면 deliver_arrival_reaction (639-661)
 │
 └─⑤ BehaviorController::handle(Reaction) (beh 229-245) → BehaviorState
       → capability_for (capability.rs 57-99) → TickOutput.capability (pr 671)
       → RR updateAnimation 639-642 → animationPlayer → asset.resolver (Swift AnimationResolver) → 행
```

- **샘플 간격.** 샘플은 틱 안에서만 돈다(RR `tick` 410). 틱 간격은 `PetRuntime::preferred_tick_interval`(pr 324-342)이
  정한다 — 잠 0.5초, `wake`·`stretch` 등 1/30초, 걷기 1/60초, 그 밖 1/12초. 그래서 샘플 간격은 0.5초에서 한 틱까지
  늘어난다.
- **키 감지 지연.** `MacUserIdleProvider.keyboardIdleDuration`(`Sources/RoamlingMac/MacUserIdleProvider.swift` 41-53)은
  마지막 OS 조회 뒤 0.5초 동안 "캐시 값 + 경과"를 돌려준다. 캐시를 읽은 뒤에 눌린 키는 캐시가 만료될 때까지 안 보인다.
  실기에서 몇 초 늦는지는 확인 못 함.
- **셸이 채우는 창.** `MacWindowProvider.currentActivityLocationHint`(`Sources/RoamlingMac/MacWindowProvider.swift` 73-84)는
  앞 앱의 가장 큰 창을 신뢰도 0.55로 준다. `wants_window_hint`는 `ActivityStarted` · `HighIntensity` ·
  `AttentionRequired` · `Present`에만 참이다(ad 50-58).
- **agent 이벤트에도 창이 없다.** `Sources/RoamlingSources`와 `rust/roamling-agent/src` 어디에도 `locationHint`/
  `location_hint`를 채우는 코드가 없다(grep 0건). 그래서 agent 이벤트도 ②에서 **그 순간 앞에 있는 창**을 받는다.
  사용자가 편집기에서 치는 동안 온 Claude 이벤트는 편집기 창을 받는다는 뜻이다. 하네스는 agent에 자기 창을 따로 줘서
  이 경로를 피한다(FT `emitAgent` 614-630). 실기에서 펫이 이 때문에 편집기 쪽으로 옮기는지는 확인 못 함.
- **행 해석.** macOS 셸은 capability를 `animationPlayer`에 넘기고(RR 639-642) 행은 `asset.resolver`
  (`Sources/RoamlingPet/PetAsset.swift` 59, 타입은 `PetAnimation.swift` 191의 Swift `AnimationResolver`)가 고른다.
  `PetAnimation.swift`는 `Foundation`만 import한다. 같은 매핑의 Rust 판은 `animation.rs` `petdex_state`(108-121)다.
  둘이 같은지 대조하는 `rust/roamling-core/tests/animation_differential.rs`의 내용은 확인 못 함.
- **Windows.** `rust/roamling-win/src/main.rs` `tick`(444-470)은 agent 이벤트를 모아 같은 `wants_window_hint` 규칙으로
  창을 채우고(465-467) `app.pet.handle_activity_event`를 부른다(469) — ②부터는 같은 코어다. ①에 해당하는 코드는
  roamling-win에 없다(`FocusActivity` 참조 grep 0건). W8은 미구현이다.

### 이벤트 하나가 그림이 되기까지

| kind · 보내는 곳 | intensity | 셸이 창을 채우나 | attention 기본 점수 | 반응 (작업 중 배율 0.8) | director가 거는 것 (지속 / 도착) | 상태 | 행 |
|---|---|---|---|---|---|---|---|
| `Present` — `arrive` 437, 하트비트 468, 따라잡기·재발신 501 | 0.5 | 예 | 30 | 없음 (att 300-302) | Calm / Calm (ad 322-329) | `idle` | idle |
| `ActivityStarted` — `send_greeting` 635 | 0.5 | 예 | 50 | `Spark` 항상 (att 289) | Observe / Spark (ad 284-291) | `spark` 0.84초 → idle | jumping |
| `HighIntensity` — `work` 645 | 0.7 | 예 | 75 | 0.56 ≥ 0.5 → `Work` (att 291-295) | Work / Work (ad 300-307) | `work`, 타이머 없음 | running |
| `AttentionRequired` — `ask` 651 | 0.5 | 예 | 100, 긴급 | `Paw` 항상, 간격 무시 (att 234-244, 259) | Paw / Paw (ad 310-317) | `waitingForUser`, 타이머 없음 | waiting |
| `Achievement` — `leave` 595 | 0.8 | 아니오 | 80 (≥ 0.75) | 0.64 < 0.75 → `SmallCelebrate`, 1.5초 간격 안이면 없음 → 기본값 `Glance` (att 240-244, 267-275; ad 338) | 좌석 비움 + 즉시 반응 (ad 335-344) | `celebrate` 0.70초, `Glance`면 `observe` 1.03초 | waving / review |
| `ActivityEnded` — `release` 659, `leave` 604, `arrive` 416, `say_goodbye` 677 | 0.5 | 아니오 | select에서 제외 (att 79) | — | 좌석 주인이면 좌석 비움 + Calm (ad 145-148) | `idle` | idle |

근거: intensity는 fa 143-146 · `intensity` 782-788, 점수는 att `score` 148-173, 배율과 반응 간격은 att
`ReactionConfiguration::default` 206-210(1.5초, 작업 중 0.8), 반응→상태는 beh 229-245, 상태 길이는 beh `timing`
105-119, `waitingForUser`·`work`에 타이머가 없는 것은 beh `settle_transient_state` 288-301, 상태→capability는
`capability.rs` 57-99, capability→Petdex 행은 `animation.rs` 108-121.

## 2. 일하는 앱 상태 기계

### 2.1 상태와 보조 상태

```text
                 지정 앱이 앞에 옴 (arrive)                    60초마다 Present
  ┌──────┐ ───────────────────────────────────────────▶ ┌─────────┐ ◀─┐
  │ Away │                                              │ Present │ ──┘
  └──────┘ ◀── leave: 지정 안 된 앱이 3초 (어느 상태든) └─────────┘
      ▲        지정된 다른 앱이면 유예 없이 arrive              │ 셀 수 있는 키
      │                                                        ▼
      │    ┌──────────┐   5초    ┌─────────┐  셀 키 뒤 10초  ┌────────┐ ◀─┐ 60초마다
      └─── │ Released │ ◀─────── │ Waiting │ ◀────────────── │ Typing │ ──┘ HighIntensity
           └──────────┘          └─────────┘                 └────────┘
                │ 셀 수 있는 키         │ 셀 수 있는 키           ▲
                └──────────────────────┴─────────────────────────┘
      키가 세션 첫 키면(session_typed 거짓) ActivityStarted + 인사, 아니면 HighIntensity
```

`Phase`(fa 156-170)는 다섯이다. 그 위에 네 가지 보조 상태가 얹힌다.

| 보조 상태 | 필드 | 뜻 | 코드 |
|---|---|---|---|
| 인사 | `greeting: Option<Greeting>` | 네 단계 — (가) `event_id` 없음: 쉬는 펫이라 아직 안 보냄 (나) `event_id` 있음, `dispatched_at` 없음: 보냈지만 director가 dispatch 안 함 (다) `dispatched_at` 있음, `arrived_at` 없음: 펫이 받았지만 아직 안 입음 (라) `arrived_at` 있음: 0.84초 박자 대기 | fa 173-189, 243 |
| 보류된 자리 소식 | `news_owed` | 쉬는 동안 자리 kind를 말하려다 못 함 | fa 250, `tell` 737-744 |
| 마지막 자리 소식 | `seat_news: Option<SeatNews>` | 마지막으로 보낸 자리 kind의 id와, 펫이 그것을 처리 중이던 마지막 샘플 — 재발신 시계 | fa 191-197, 253, 340-344 |
| 손 흔들기 뒤 끝 | `goodbye: Option<Goodbye>` | 손 흔들기를 보냈고 `ActivityEnded`를 보류 중 | fa 199-205, 256 |

인사가 있는 동안 `sustain`은 `sustain_greeting`만 돌고 돌아간다(fa 450-453) — 하트비트·따라잡기·일반 재발신이 없다.

세션 관련 필드: `session_typed`(230, 이 세션의 점프가 **펫에게 dispatch됐는가**), `typed_seconds`(238), `last_app`(215),
`left_at`(220), `away_since`(223), `in_front_since`(227), `last_counted_key_at`(234), `waiting_since`(236),
`last_emitted_at`(240, `event`가 갱신 803).

### 2.2 상수

| 상수 | 값 | 쓰이는 곳 | 코드 |
|---|---|---|---|
| `TYPING_WINDOW` | 10.0초 | `keys_stopped`(759-761), `typed` 판정(383) | fa 62 |
| `FOCUS_GRACE` | 3.0초 | `leave` 유예(571), `left_at = now - 3`(613) | fa 66 |
| `BREAK` | 120.0초 | `arrive`의 새 세션 판정(422) | fa 70 |
| `WAVE_AFTER_TYPING` | 180.0초 | `leave`의 손 흔들기(574) | fa 75 |
| `GREETING_DELAY` | 0.840초 | 인사 박자(717) | fa 85 |
| `GREETING_TIMEOUT` | 20.0초 | dispatch 뒤 인사 상한(704) | fa 93 |
| `BLOCKED_RESEND` | 15.0초 | `resend_due`(764-766) | fa 103 |
| `HEARTBEAT` | 60.0초 | `heartbeat_due`(455) | fa 110 |
| `WAITING_BEFORE_RELEASE` | 5.0초 | `Waiting` → `release`(472-478) | fa 116 |
| `WAVE_HOLD_TIMEOUT` | 5.0초 | `goodbye_is_due`(667-672) | fa 133 |
| `MAX_RECENT` | 6 | 메뉴의 최근 앱(`remember` 817) | fa 137 |
| `TYPING_INTENSITY` · `PRESENT_INTENSITY` · `WAVE_INTENSITY` | 0.7 · 0.5 · 0.8 | `intensity`, `leave` | fa 143-146 |
| 샘플 간격 | 0.5초 이상 | 셸 | RR 109, 541 |

### 2.3 전이 표

한 샘플의 순서는 `observe`(fa 324-392)가 정한다: 경과 시간 계산(335) → 펫이 `seat_news`를 처리 중이면 `heard_at` 갱신
(340-344) → `on_seat` 계산, 아니면 `in_front_since` 지움(346-352) → 보류된 끝이 기한이면 보냄(359-361) → `app`이 nil이면
끝(368) → 최근 앱 기억(369) → `on_seat`이고 `Typing`이면 `typed_seconds += 경과`(374-376) → 아래 표.

| 지금 | 조건 | 보내는 이벤트 | 다음 | 코드 |
|---|---|---|---|---|
| 아무거나 | `app` nil | 보류된 끝(`ActivityEnded`)만, 기한이면 | 그대로 | fa 359-368 |
| `Away` | 지정 안 된 앱 | 없음 (최근 앱에만 기억) | `Away` | fa 378-379, `leave` 569 |
| `Present`·`Typing`·`Waiting`·`Released` | 지정 안 된 앱, `away_since`부터 3초 미만 | 없음 | 그대로 | `leave` 570-573 |
| 같음 | 3초 이상, `typed_seconds ≥ 180`, `agent_on_duty` 거짓 | `Achievement` 0.8. 끝은 `goodbye`로 보류 | `Away` | `leave` 574-600, 606-613 |
| 같음 | 3초 이상, `typed_seconds ≥ 180`, `agent_on_duty` 참 | `ActivityEnded`만 (누적은 0) | `Away` | `leave` 593-605 |
| 같음 | 3초 이상, `typed_seconds < 180` | `ActivityEnded` (누적은 유지) | `Away` | `leave` 603-613 |
| `Away` | 지정 앱 | [보류된 끝] `Present`. `left_at`이 없거나 120초 이상 전, 또는 다른 앱이면 세션 초기화 | `Present` | `arrive` 408-438 |
| `Present`·`Typing`·`Waiting`·`Released` (앱 A) | 지정된 다른 앱 B | [보류된 끝] `ActivityEnded`(A) + `Present`(B). `left_at`이 없어 **항상 새 세션** | `Present` | `arrive` 411-426 |
| `Present`·`Waiting`·`Released` | 셀 수 있는 키, `session_typed` 거짓 | `ActivityStarted` 0.5, 인사 시작 | `Typing` | `sustain` 459-464, `greet` 619-627 |
| 같음 | 셀 수 있는 키, `session_typed` 참 | `HighIntensity` 0.7 | `Typing` | `sustain` 459-461, `work` 644-648 |
| `Present` | 마지막 발신 뒤 60초 | `Present` | `Present` | `sustain` 467-469 |
| `Typing` (인사 없음) | 셀 수 있는 마지막 키 뒤 10초 | `AttentionRequired` 0.5 | `Waiting` | `sustain` 470, `ask` 650-654 |
| `Typing` (인사 없음) | 마지막 발신 뒤 60초 | `HighIntensity` | `Typing` | `sustain` 471 |
| `Waiting` | `waiting_since`부터 5초 | `ActivityEnded` | `Released` | `sustain` 472-478, `release` 658-662 |
| `Released` | 키 없음 | 없음 — 하트비트도 재발신도 없다 | `Released` | `sustain` 479, `say_where_things_stand` 504 |
| 인사 (가) | 매 샘플 | 쉬지 않으면 `ActivityStarted` | 인사 (나) | `sustain_greeting` 526-531, `send_greeting` 632-642 |
| 인사 (나) | `dispatched_event == event_id` | 없음. `dispatched_at = now`, `session_typed = true` | 인사 (다), 같은 샘플에 계속 | `sustain_greeting` 533-539 |
| 인사 (나) | 셀 키 뒤 10초 | `AttentionRequired`. 인사 버림, `session_typed`는 거짓 그대로 | `Waiting` | `sustain_greeting` 540-546 |
| 인사 (나) | 자리 소식이 15초 동안 처리 안 됨 | `ActivityStarted` 새 id | 인사 (나) | `sustain_greeting` 547-551 |
| 인사 (다) | `dispatched_event == event_id`이고 `arrival_pending` 거짓 | 없음. `arrived_at = now` | 인사 (라) | `greeting_is_over` 707-715 |
| 인사 (다)·(라) | `dispatched_at`부터 20초 | 키가 멈췄으면 `AttentionRequired`, 아니면 `HighIntensity` | `Waiting`/`Typing` | `greeting_is_over` 704-706, `sustain_greeting` 557-564 |
| 인사 (라) | `arrived_at`부터 0.84초 | 같음 | `Waiting`/`Typing` | `greeting_is_over` 717, `sustain_greeting` 557-564 |
| 아무거나 | 자리 kind를 말하는데 `pet_resting` | 안 보냄. `news_owed = true` | 그대로 | `tell` 737-742, `is_seat_news` 772-780 |
| `on_seat` 샘플, 인사 없음 | `news_owed`이고 쉬지 않음 | 지금 phase의 kind (`Released`·`Away`면 없음) | 그대로 | `sustain` 484-487, `say_where_things_stand` 499-506 |
| `on_seat` 샘플, 인사 없음 | `seat_news`가 15초 동안 처리 안 됨 | 지금 phase의 kind, 새 id | 그대로 | `sustain` 492-494, `resend_due` 764-766 |
| `goodbye` 있음 | `dispatched_event == wave_id` 또는 손 흔들기 뒤 5초 | `ActivityEnded`(떠난 앱) | `goodbye` 없음 | `observe` 359-361, `goodbye_is_due` 667-672, `say_goodbye` 675-678 |

모든 이벤트는 `tell`(729-754) 한 곳으로 나간다. `ActivityEnded`는 `seat_news`를 지우고(750-752), `Achievement`는 건드리지
않는다. 모든 이벤트는 새 id를 받는다(`event` 800-802).

### 2.4 nil 입력

`app`이 nil이면 보류된 끝만 처리하고 돌아간다(fa 359-368). 그래서 그 샘플에는 `remember`, 누적, `leave`, `sustain`이 모두
안 돈다. `in_front_since`는 지워진다(350-352). 시계는 멈추지 않는다 — `keys_stopped`(759-761)·`waiting_since`(474-475)·
`heartbeat_due`(455)·`resend_due`(764-766)가 전부 `now`와 저장된 시각을 비교하므로 앱이 다시 보인 첫 샘플에 그사이 기한이
된 것이 나간다. `away_since`는 nil 동안 지워지지 않는다 — 지우는 곳은 `on_seat`(381)·`arrive`(429)·`leave`(608)뿐이다.
macOS에서 nil이 오는 경우는 펫 자신이 앞에 있을 때다(`MacWindowProvider.frontmostApplicationIdentifier` 60-65).
고정 테스트: `an_unknown_front_app_mid_sitting_says_nothing_and_loses_nothing`(fa 1582),
`the_pets_own_app_in_front_is_not_the_user_leaving`(fa 1688), `a_wave_the_pet_never_gets_holds_the_end_for_the_cap_at_most`
(fa 2132, nil 동안 끝이 나감). nil 동안 유예가 이어지는 것을 고정하는 테스트는 없다.

### 2.5 Cmd-Tab 규칙

키 입력 API는 "마지막 keyDown 뒤 몇 초"만 준다(MacUserIdleProvider.swift 46-49). 그래서 source는 **앱이 끊김 없이 앞에 있기
시작한 첫 샘플**(`in_front_since`, `get_or_insert` 382)보다 나중에 눌린 키만 센다(`typed` 383). 다른 앱이든 nil이든
`on_seat`이 아닌 샘플이 하나라도 끼면 `in_front_since`가 지워진다(350-352). 센 키의 시각은 `last_counted_key_at`
(384-386)에 두고, 10초 판정은 원시 `seconds_since_key`가 아니라 이 값으로 한다(`keys_stopped` 759-761). 이 값을 쓰는 곳은
`Typing` → `ask`(470), 인사 끝의 갸웃/일 판단(560), 막힌 인사를 버리는 판단(540)이다.
고정 테스트: `the_key_that_brought_the_app_forward_is_not_typing`(fa 1038), `a_key_at_the_moment_of_arrival_is_not_typing`
(1051), `a_glance_away_and_back_by_keyboard_is_not_typing`(1062), `a_keyboard_glance_away_does_not_restart_the_typing_window`
(1543).

### 2.6 세션

`arrive`는 `left_at`이 없거나 120초 이상 전이면 쉰 것으로 보고, 앱이 `last_app`과 다르면 역시 새 세션으로 본다
(fa 419-426). `leave`는 `left_at = now - FOCUS_GRACE`로 적는다(613). **지정 앱에서 지정 앱으로 곧장 가면** `leave`를
거치지 않아 `left_at`이 없고, 그래서 항상 새 세션이다(419-422 주석과 코드). A → B → A로 곧장 돌아와도 A는 새 세션이다.
고정 테스트: `a_long_break_starts_a_new_session`(1390), `a_short_break_keeps_the_session`(1405),
`a_short_break_before_any_typing_still_owes_the_hop`(1420), `switching_work_apps_ends_one_and_seats_the_other`(1432),
`a_different_app_after_a_short_break_is_a_new_session`(1449). A → B → A 즉시 복귀를 고정하는 테스트는 없다.

## 3. director에서 발목 잡는 동작

각 항목: 무엇을 하는가 · 코드 위치 · 일하는 앱에 준 영향.

### 3.1 attention 점수와 자리 바뀜

- **점수**는 `kind 기본 + intensity×10 + 신선도 + 창 신뢰도×3`이다. 신선도는 30초에 걸쳐 5에서 0으로 줄어든다
  (att `score` 148-173). 기본값: `AttentionRequired` 100 · `Negative`/`Setback` 90 · `Achievement` 80(intensity ≥ 0.75)
  또는 65 · `HighIntensity` 75 · `Positive` 65 · `Inspecting` 60 · `ActivityStarted` 50 · `Present` 30 · `Calm` 30 ·
  `ActivityEnded`/`Idle` 0 (149-168).
- **설정 기본값**은 dwell 3초 · 이력 여유 12 · 재방문 쿨다운 2초 · 이벤트 수명 30초다(att 40-44).
- **`select`**(att 73-138): 30초 안의 미래가 아닌 이벤트 중 `ActivityEnded`·`Idle`을 뺀 것만 본다(74-82). 최고점이 없으면
  `clear`하고 None(90-93). 현재 source가 없으면 최고점을 잡는다(95-98). 같은 source면 이벤트만 바꾼다(100-103). 다른
  source면 `(긴급 아님 && dwell < 3초) || 쿨다운 중 || 후보 < 현재 + 12`일 때 현재를 유지한다(128-133). 긴급은
  `AttentionRequired`·`Negative`·`Setback`이다(106-111).
- **"현재"는 넘겨받은 목록에서 다시 찾는다**(att 112-121). director가 거른 목록을 넘기면(3.7) 현재 source의 이벤트가 목록에
  없어서 `current`가 None이 되고, 유지 조건에 걸리면 `select`가 None을 돌려준다 — dispatch도 pending도 없다.
- **선택은 이벤트가 들어올 때만 돈다.** `select`를 부르는 곳은 `handle_event`(ad 155)와 `queue_next_candidate`(ad 435)
  둘이다. `begin_tick`·`finish_tick`은 부르지 않는다(pr 476-683). 유지 조건에 막힌 이벤트는 dwell·쿨다운이 지나도 저절로
  dispatch되지 않고, 다음에 누군가 이벤트를 보낼 때 다시 판정된다.
- **dispatch 없이도 attention 상태는 바뀐다.** `select`는 dispatch 여부를 모른 채 `acquire`한다(att 96, 136). 쉬는 펫에게
  버려진 이벤트(3.3)나 잡힌 펫의 pending도 attention의 현재 source가 된다. 이것이 실사용에서 무엇을 바꾸는지는 확인 못 함.

일하는 앱의 최고점(창 신뢰도 0.55, `MacWindowProvider.swift` 82; `Achievement`는 창을 안 받는다, ad 50-58):

| kind | 계산 | 최고점 |
|---|---|---|
| `Present` 0.5 | 30 + 5 + 5 + 1.65 | 41.65 |
| `ActivityStarted` 0.5 | 50 + 5 + 5 + 1.65 | 61.65 |
| `HighIntensity` 0.7 | 75 + 7 + 5 + 1.65 | 88.65 |
| `AttentionRequired` 0.5 | 100 + 5 + 5 + 1.65 | 111.65, 긴급 |
| `Achievement` 0.8 | 80 + 8 + 5 + 0 | 93 |

영향: 점수만으로는 agent와 겨루지 못해 출처 거름(3.7)이 생겼다. 선택이 이벤트 때만 도는 것은 15초 재발신(4절)의 원인 중
하나다.

### 3.2 dispatch마다 도착 반응을 다시 건다

- `begin_watching`(ad 394-425)은 이벤트에 창이 있으면 그 창을 기록하고 **도착 반응을 새로 덮어쓴다**(403-407, 424). 창이
  없고 다른 source면 창을 지우고 반응을 그 자리에서 입힌다(408-423). 같은 source가 창 없이 오면 옛 창이 남는다(408).
- 도착 반응은 두 곳에서 입혀진다. `hold_seat`(pr 1032-1042)는 앉아 있는 펫에게 **같은 틱에** 입힌다. 걷기가 끝난 틱에는
  `did_arrive` 블록(pr 639-661)이 입히되, `hold_seat`가 이미 입혔으면 건너뛴다(644-652).
- `deliver_arrival_reaction`(ad 225-238)은 도착 반응이 없으면 지속 반응을, 그것도 없으면 `Observe`를 입힌다(231-234).
- 도착 반응이 없을 때 `hold_seat`는 상태가 `observe`·`work`·`waitingForUser`·`celebrate`·`sad`·`spark`·`wake`·`stretch`·
  `caught`·`dragged`가 **아닐 때만** `sustain_on_seat`를 부르고(pr 1043-1061), 그것은 `Work`·`Paw`만 다시 입힌다
  (ad 242-250, `is_ongoing` act 172-174). `ActivityStarted`의 지속 반응 `Observe`와 `Present`의 `Calm`은 다시 입혀지지
  않는다.
- 커서가 인식 거리 안이면(`Watching`) `finish_tick`은 `apply_intent`를 부르지 않는다(pr 616-630) — 도착 반응은 커서가 떠날
  때까지 남는다.

영향: 인사 뒤 `HighIntensity`를 일찍 보내면 아직 안 입은 `Spark`가 `Work`로 덮인다 → `GREETING_DELAY`와 `arrived_at`, 입력
`dispatched_event`·`arrival_pending`(fa 77-85, 696-718). 60초마다 다시 보내는 kind는 재생해도 안 보이는 반응만 입을 수
있다 → `Present`는 `Calm`(ad 318-321 주석). 고정 테스트: `present_walks_the_pet_over_and_sits_it_down_wearing_nothing`
(ad 575, 재발신에도 도착 반응이 다시 걸림).

### 3.3 쉬는 펫에게 온 안 깨우는 kind를 버린다

- `handle_event`는 쉬는 펫(`Sit`·`FindSleepSpot`·`Sleep`, beh 64-66)에게 선택된 이벤트가 `wakes_resting_pet`가 아니면
  **pending에도 안 넣고 돌아간다**(ad 169-172). 깨우는 kind는 `AttentionRequired`·`Achievement`·`Negative`·`Setback`
  뿐이다(act 51-56). 이벤트는 `recent`에 남는다(ad 153).
- macOS 셸의 틱에서 샘플은 맨 앞(RR 410)이고, 키 입력으로 펫을 깨우는 판정은 `finish_tick` 안(pr 509-511, RR 422)이다.
  세션 첫 키가 보인 샘플에서 펫은 아직 쉬는 중이다.

영향: source가 쉬는 동안 자리 kind 넷을 보내지 않고 `news_owed`로 들고 있다가 깬 첫 샘플에 보낸다(fa `tell` 737-744,
`sustain` 484-487). 인사는 `event_id` 없는 단계로 기다린다(fa 526-531). 입력 `pet_resting`이 이 때문에 있다.
고정 테스트: `present_lets_a_sleeping_pet_sleep`(ad 653).

### 3.4 pending · `resume_pending_if_ready` · `queue_next_candidate` · `last_dispatched_id`

- `pending`은 **한 칸**이다(ad 68). 채우는 곳: 잡힌 펫(166), 쉬는 펫 + 깨우는 kind(174), `queue_next_candidate`(443-447).
  뒤에 온 것이 앞의 것을 덮는다.
- `resume_pending_if_ready`는 `begin_tick`이 **상태가 `Idle`일 때만** 부른다(pr 488-494, ad 206-208). `work`·
  `waitingForUser`·`lookAtPointer`·`wake`·`stretch`에서는 기다린다. 꺼낼 때 나이를 다시 보지 않는다(ad 209-219).
  `System`이고 agent가 자리를 지키면 버린다(216-218).
- `queue_next_candidate`(ad 433-448)는 거른 후보로 다시 고르고, 고른 것이 마지막 dispatch id와 같거나 없으면 pending을
  **비운다**(436, 440, 443-444). 부르는 곳은 종료 kind 처리(149)와 `finish_transient`(430)다.
- `last_dispatched_id`는 dispatch에서 쓰고(261), **지우는 곳은 한 군데**다 — 종료 kind가 왔고 그 source가 attention의 현재
  source일 때(141-144). `Achievement`·`Negative`(`finish_transient` 427-431), `expire_silent`(185-195), resume의 버림
  (216-218)은 지우지 않는다.
- `handle_event`는 선택된 이벤트 id가 마지막 dispatch id와 같으면 아무것도 안 한다(161-163).

영향: 매 이벤트가 새 id를 받는다(fa `event` 790-802). `seat_news.heard_at`은 `dispatched_event`로 갱신한다(fa 340-344).
끝난 agent의 id가 남아 있으므로 손 흔들기는 `dispatched_event`를 보고 판단하지 않는다(fa 586-591, 테스트
`the_source_waves_whatever_the_pet_last_acted_on` 2091).

### 3.5 `Achievement`와 `finish_transient`가 자리를 비우는 방식

- `dispatch`의 `Achievement` 갈래(ad 335-344)는 **어느 source가 좌석을 쥐었는지 보지 않고** `clear_active`한다. 그다음
  반응(기본값 `Glance`)을 입히고 `finish_transient`(427-431)로 그 source를 `recent`에서 지우고, attention을 `clear`하고,
  다음 후보를 큐에 넣는다. `Negative`도 같다(345-354).
- 그래서 dispatch된 `Achievement` 뒤의 같은 source `ActivityEnded`는 할 일이 없다 — `recent`·좌석·attention이 이미 비어
  있다(136-150). 고정 테스트: `the_end_after_the_wave_does_not_cancel_it`(fa 1654).
- 쉬는 펫에게 온 `Achievement`는 `CancelRest` + pending이 되고(ad 169-175), 펫은 `wake` 0.7초 + `stretch` 1.0초 뒤
  `Idle`이 돼야 그것을 dispatch한다(beh 117-118, 292-293; pr 488-494). 그 사이에 같은 source의 `ActivityEnded`가 오면,
  좌석 주인이면 `Calm`을 입히고(147) `queue_next_candidate`가 pending을 비운다(149, 436).

영향: 떠날 때 손 흔들기가 펫이 있는 자리에서 좌석을 비운다(fa 575-579). `ActivityEnded`를 손 흔들기가 dispatch될 때까지
보류하는 `goodbye`·`WAVE_HOLD_TIMEOUT`(fa 118-133, 667-678)이 생겼다. 고정 테스트:
`a_wave_through_the_director_is_worn_before_the_seat_is_given_back`(fa 2183).

### 3.6 5분 침묵 만료는 다음 후보를 큐에 넣지 않는다

- `expire_silent`(ad 185-195)는 `begin_tick`마다 불리고(pr 482), 좌석 주인의 `heard_at`에서 300초가 지나면
  (`SILENCE_BEFORE_EXPIRY`, act 146-150) `clear_active` + `Calm`만 한다. `queue_next_candidate`도 attention `clear`도 없다.
- `heard_at`은 dispatch에서 좌석이 비었거나 같은 source일 때(ad 262-266)와 `begin_watching`(416)에서만 갱신된다.

영향: 가만히 앉아 읽는 사용자 옆 좌석이 300초에 끝나지 않도록 `HEARTBEAT` 60초가 있다(fa 105-110). agent 좌석이 만료된
순간 일하는 앱은 큐에 없으므로 다음 발신(재발신 15초 또는 하트비트)에야 이어받는다. 고정 테스트:
`heartbeats_keep_the_seat_from_expiring`(fa 1628), `a_quiet_agent_keeps_the_seat_until_its_watch_expires`(ad 734 — 만료 뒤
**새 이벤트**로 이어받는 쪽만 본다). 만료 순간 큐에 안 넣는다는 것 자체를 고정하는 테스트는 없다.

### 3.7 출처 거름과 pending 재거름

- `candidates`(ad 463-470)는 `agent_on_duty`가 참이면 `System` 이벤트를 뺀다. 쓰는 곳은 `handle_event`(154)와
  `queue_next_candidate`(434)다. 거른 이벤트도 `recent`에는 들어간다(153).
- `agent_on_duty`(ad 479-489)는 둘 중 하나다. (1) `recent`에 남은 `Agent` 이벤트가 30초 안이다. `recent`는 source당
  마지막 하나만 들고, dispatch된 `Achievement`·`Negative`는 `finish_transient`가(428), 종료 kind는 140이 지운다. (2) 좌석
  주인이 `Agent`이고 `heard_at`에서 300초가 안 됐다.
- pending은 `resume_pending_if_ready`에서 한 번 더 거른다(216-218). 버린 이벤트는 `recent`에 남는다.

영향: 막혀 있는 동안 source가 15초마다 다시 말한다(`BLOCKED_RESEND`, fa 95-103). 인사는 dispatch돼야 소모된다
(`session_typed`, fa 228-230, 533-539). 떠날 때 손 흔들기 여부를 source가 입력 `agent_on_duty`로 판단한다(fa 581-600).
고정 테스트: ad 695 `the_desk_never_draws_the_pet_off_a_working_agent`, 716 `the_desks_question_does_not_interrupt_a_working_agent`,
757 `the_desk_takes_the_seat_the_moment_the_agent_lets_go`, 787 `an_agent_starting_a_turn_takes_the_seat_from_the_desk`,
818 `a_desk_word_queued_before_the_agent_came_back_does_not_take_its_seat`; Swift ↔ Rust 대조
`the Rust activity director agrees with the Swift one`(`Tests/RoamlingLogicTests/CoreLogicTests.swift` 408, 출처 종류와 버림
검사 443-601).

### 3.8 `Present` kind가 있는 이유

- 창을 받는 kind는 도착하면 무언가를 입는다: `ActivityStarted` → `Spark`, `HighIntensity` → `Work`, `AttentionRequired` →
  `Paw`(ad 284-317). `Inspecting`은 `Observe`(1.03초 review 행)를 입고, `wants_window_hint`가 거짓이라(ad 50-58) 새 source로
  오면 창이 없어 걷지도 않는다(408-423).
- `Present`는 지속·도착 반응이 모두 `Calm`(= `idle`, beh 241)이고(ad 322-329), 반응 정책이 None을 돌려줘서 1.5초 간격
  시계를 건드리지 않는다(att 297-302, 305-307). 점수 30이라 최고점 48(창 신뢰도 1.0일 때)이 `ActivityStarted`의 최저점 50보다
  낮다(att 161-165). 자는 펫을 깨우지 않는다(act 51-56). 문서 주석: act 30-40.
- 고정 테스트: `present_is_worth_asking_where_the_window_is`(ad 567), `present_walks_the_pet_over_and_sits_it_down_wearing_nothing`
  (575), `present_never_takes_a_seat_from_an_agent`(613), `present_lets_a_sleeping_pet_sleep`(653).

## 4. 우회 코드 ↔ 원인 표

| 일하는 앱 쪽 장치 | 코드 위치 | 막는 것 | 원인이 된 동작 | 원인 코드 |
|---|---|---|---|---|
| 인사 대기 — `Greeting.dispatched_at`·`arrived_at`, `GREETING_DELAY`, 입력 `dispatched_event`·`arrival_pending` | fa 173-189, 512-565, 696-718; RR 554-555; pr 286-295 | 박자(`HighIntensity`)가 안 입은 `Spark`를 덮음 | 도착 반응 덮어쓰기(3.2). 보낸 것 ≠ 받은 것 — 잡힘·쉼·거름으로 dispatch가 미뤄지거나 없음(3.3, 3.4, 3.7) | ad 424, 165-176, 154; pr 1036-1041, 651-657 |
| `GREETING_TIMEOUT` 20초 (dispatch부터) | fa 87-93, 704-706 | 받은 인사를 끝내 못 입으면 source가 영영 침묵 | 커서가 가까우면 도착 반응을 안 입힘, 걷기 상한 `8 + 남은 거리/속도×2`, 걷는 중 다른 이벤트가 도착 반응을 덮음 | pr 616-630; pl 694-697; ad 424 |
| 쉬는 중 보류 — `news_owed`, `is_seat_news`, 인사 (가) 단계, 입력 `pet_resting` | fa 737-744, 772-780, 484-487, 526-531; RR 556-560; pr 297-303 | 자는 펫에게 보낸 자리 소식이 버려짐 | 쉬는 펫에게 온 안 깨우는 kind를 버림 + 틱 안에서 샘플이 깨움보다 앞 | ad 169-172; act 51-56; RR 410, 422; pr 509-511 |
| 막힘 재발신 — `BLOCKED_RESEND`, `SeatNews`, `resend_due` | fa 95-103, 191-197, 340-344, 489-494, 547-551, 764-766 | agent가 자리를 비울 때 넘겨받을 후보가 없음 | 거름(3.7), attention 수명 30초, 선택이 이벤트 때만 돎(3.1), 만료가 큐에 안 넣음(3.6) | ad 463-470, 155, 435, 185-195; att 77 |
| 60초 `HEARTBEAT` | fa 105-110, 455, 467-471 | 키 없이 300초가 지나면 좌석 만료 | 5분 침묵 만료, `heard_at`은 dispatch 때만 갱신 | ad 185-195, 262-266; act 146; pr 482 |
| 매 이벤트 새 id | fa 790-802 | 같은 id 재발신이 무시됨 | 마지막 dispatch id와 같으면 아무것도 안 함 | ad 161-163 |
| `Present` kind | act 30-40; ad 318-329; att 165, 300-302 | 걸어와 앉기만 하는 kind가 없음, 재발신이 반응을 재생 | 도착 반응 재무장(3.2), 다른 좌석 kind는 보이는 반응을 입음(3.8) | ad 284-317, 424; pr 1036-1041 |
| 손 흔들기 뒤 보류 — `Goodbye`, `WAVE_HOLD_TIMEOUT` | fa 118-133, 199-205, 359-361, 409-411, 597-605, 667-678 | 쉬는 펫에게 대기 중인 손 흔들기를 `ActivityEnded`가 지움 | 쉬는 펫의 `Achievement`는 pending, `Idle`에서만 dispatch. 종료 kind가 `Calm` + pending 비움(3.5) | ad 169-175, 147, 149, 436; pr 488-494; beh 292-293 |
| 입력 `agent_on_duty` (떠날 때 손 흔들기 생략) | fa 313-316, 581-600; RR 561-565; pr 305-312; ffi.rs 1392 | 걸러진 손 흔들기가 `recent`에 남았다가 agent가 끝난 뒤 늦게 재생 | 거른 이벤트도 `recent`에 들어감, agent가 비우면 `queue_next_candidate`가 다시 고름, 반응 간격 안이면 `Glance` | ad 153, 430, 149, 338; att 240-244 |
| 막힌 인사는 소모 안 함 (`session_typed`는 dispatch에서) | fa 228-230, 533-546 | 사용자가 못 본 점프를 세션이 써 버림 | 거름으로 dispatch가 안 됨 | ad 463-470 |
| Cmd-Tab 규칙 — `in_front_since`, `last_counted_key_at` | fa 221-234, 346-352, 380-386, 759-761 | 앱 전환 키를 타이핑으로 셈 | **director 원인 아님** — 키 입력 API가 마지막 keyDown 뒤 초만 줌 | MacUserIdleProvider.swift 41-53 |
| nil 규칙 | fa 363-368 | 펫 메뉴를 연 것을 떠남으로 읽음 | **director 원인 아님** — 셸이 자기 프로세스에 nil을 줌 | MacWindowProvider.swift 60-65 |

## 5. 시나리오별 시간표 (초 단위)

공통 가정. 샘플은 0.5초 격자에 맞춰 돈다(실제로는 0.5초 이상, §1). 키는 샘플 0.1초 전에 눌렸고 키 캐시 지연은 없다고
둔다. 펫은 따로 적지 않으면 깨어 있고, 커서는 인식 거리 밖이며, 커서 회피와 배회는 기본값인 켜짐이다(RR 154-157).
`W`는 걷는 초다 — 기본 속도는 160pt/s(`rust/roamling-core/src/tuning.rs` 257, `RuntimeTuning::default` 첫 인자)이고,
틱마다 `출발 뒤 경과 > 8 + 남은 거리 ÷ 160 × 2`이면 도착으로 처리한다(pl 386, 694-697). 도착 판정 거리는 4pt다(pl 211,
382-383). `≈`는 틱 간격만큼 어긋날 수 있다는 뜻이다.

### 5a. agent 없음 — 앱이 앞에 옴부터 떠나며 손 흔들기까지

| 시각 | 입력 | 일하는 앱이 보냄 | director · 런타임 | 펫이 입는 그림 | 코드 |
|---|---|---|---|---|---|
| 0.0 | 편집기(지정 앱)가 앞에 옴 | `Present` 0.5 | 선택·dispatch, 반응 없음 → 도착 반응 `Calm`. 같은 틱 placement가 새 source라 `NewActivity`를 판정하고, 목적지를 받아들이면 걷는다 | 걷기 행 | fa 437; ad 153-177, 322-329; pl 393-399, 585-586, 660-671; pr 1008-1027 |
| W | 좌석 도착 | 없음 | `did_arrive` → `Calm` | idle | pr 558-560, 651-657; pl 381-390 |
| 10.0 | 첫 키 (9.9) | `ActivityStarted` 0.5, 인사 시작 | dispatch → 도착 반응 `Spark`. 앉은 펫이라 같은 틱에 `hold_seat`가 입힘 | jumping | fa 459-464, 619-627; ad 284-291; pr 1036-1041; beh 107 |
| 10.5 | 계속 침 | 없음. `dispatched_at = arrived_at = 10.5`, `session_typed` | — | jumping | fa 533-539, 707-715 |
| ≈10.84 | — | — | `spark` 만료. 지속 반응 `Observe`는 다시 입히지 않음 | **idle** | beh 296; pr 1054-1061; ad 244-247 |
| 11.5 | 계속 침 | `HighIntensity` 0.7 | dispatch → `Work`. 반응 간격 1.5초가 딱 지남 | running | fa 717, 557-564, 644-648; att 240-244, 291-295; ad 300-307 |
| 71.5 · 131.5 | 계속 침 | `HighIntensity` 하트비트 | 재dispatch. `work` → `work`는 전이 없음 | running | fa 471; beh 303-306 |
| 190.0 | 마지막 키 179.9 뒤 10초 | `AttentionRequired` 0.5 | 긴급 → `Paw` | waiting | fa 470, 650-654, 759-761; att 259; beh 237 |
| 195.0 | 갸웃 5초 | `ActivityEnded` | `recent` 제거, attention clear, `last_dispatched_id` 지움, 좌석 비움 + `Calm`, 다음 배회 197.0 | idle | fa 472-478, 658-662; ad 136-150, 493-501, 519-523 |
| ≥197.0 | — | 없음 (`Released`는 말하지 않음) | 배회 판정 | 걷기 행 | pl 702-; fa 479 |
| 300.0 | 크롬(지정 안 됨)이 앞에 옴 | 없음. `away_since = 300.0` | — | 그대로 | fa 570-573 |
| 303.0 | 3초 | `Achievement` 0.8 (누적 180.0). 끝은 보류 | attention에 현재 source가 없어 곧바로 dispatch → 좌석 비움 + `SmallCelebrate` + `finish_transient` | waving 0.70초 | fa 574-600; att 95-98, 267-275; ad 335-344 |
| 303.5 | — | `ActivityEnded` (`dispatched_event == 손 흔들기`) | 할 일 없음 | ≈303.7부터 idle | fa 359-361, 667-678; ad 136-150 |

- **누적 180.0**은 10.5부터 190.0까지 샘플마다 0.5를 더한 값이다. 190.0 샘플도 `sustain`이 phase를 바꾸기 전에 더해진다
  (fa 374-376). 마지막 키가 179.9보다 이르면 180에 못 미쳐 303.0에는 `ActivityEnded`만 나간다(fa 603-605).
- **10.84 ~ 11.5의 약 0.66초 idle**은 코드에서 계산한 것이다 — 박자 0.84초를 "입은 것을 본 샘플"(10.5)부터 세기 때문이다
  (fa 707-717). 이 틈을 고정하는 테스트는 없다.
- 0.0에 placement가 목적지를 받아들이지 않으면(지금 선 자리가 창을 보고, 새 좌석이 15점 넘게 낫지 않음, pl 660-671, 207-208)
  걷지 않고 그 자리를 좌석으로 적은 뒤(pl 468-477) 같은 틱 `hold_seat`가 `Calm`을 입힌다.
- 고정 테스트: 입는 순서 `Calm, Spark, Work, Paw, Calm, Work`는 `a_sitting_through_the_director_wears_what_it_promises`
  (fa 1729, 시각은 안 봄). 박자 = 입은 것을 본 샘플 + 0.84는 `the_first_keystroke_hops_and_the_beat_waits_for_the_hop`(fa 1077).
  10초·5초는 `ten_quiet_seconds_ask_and_five_more_give_the_seat_back`(fa 1348). 60초는 `a_typing_desk_says_so_every_minute_too`
  (fa 1618). 누적 규칙은 `asking_and_released_time_is_not_counted_toward_the_wave`(fa 1520)와
  `a_stretch_just_short_of_the_wave_leaves_without_one`(fa 2070). 자리를 비운 뒤의 손 흔들기와 다음 샘플의 끝은
  `leaving_a_released_seat_after_a_long_stretch_waves`(fa 2046)와 `a_wave_through_the_director_is_worn_before_the_seat_is_given_back`
  (fa 2183, 끝이 축하 0.5초 뒤). 실제 런타임은 FT 18 `a named work app brings the pet over to sit, and nothing more`, FT 54
  `the first keystroke hops, ten quiet seconds ask, and five more give the seat back`, FT 306
  `a long sitting waves on the way out, even after the seat went back`(순서만 봄). `W`의 값: 테스트 없음.

### 5b. Claude가 일하는 중 — 막힌 타이핑을 Claude `Stop` 뒤에 이어받기

가정: Claude가 **자기 창** 옆 좌석을 쥐고 일한다. 실제 Claude 이벤트에는 창이 없어 그 순간 앞 창(편집기)을 받으므로(§1),
실기에서 좌석 위치가 이 표와 같은지는 확인 못 함.

| 시각 | 입력 | 일하는 앱이 보냄 | director · 런타임 | 펫이 입는 그림 | 코드 |
|---|---|---|---|---|---|
| 0.0 | 편집기가 앞에 옴 | `Present` | `System`이라 후보 아님 → attention이 agent 이벤트를 다시 고르고 마지막 dispatch id와 같아 끝. agent 마지막 이벤트가 30초를 넘었으면 후보가 비어 `clear`만 | agent 옆 running | fa 437; ad 153-163, 463-470; att 90-93 |
| 2.0 | 첫 키 | `ActivityStarted` h1, 인사 (나) | 후보 아님 | 그대로 | fa 459-464; ad 154 |
| 17.0 · 32.0 | 계속 침 | `ActivityStarted` h2 · h3 (15초마다 새 id) | 후보 아님 | 그대로 | fa 547-551, 764-766 |
| 40.0 | Claude `Stop` | — | `Achievement` 0.55 dispatch → 좌석 비움 + 반응 → `finish_transient`가 agent를 `recent`에서 지움 → `agent_on_duty` 거짓 → h3가 pending | waving 0.70초. 1.5초 안에 다른 반응이 있었으면 `Glance` → review 1.03초 | `Sources/RoamlingSources/ClaudeCode/ClaudeCodeEventNormalizer.swift` 100-101; ad 335-344, 427-448, 479-489; att 240-244, 267-275 |
| 40.5 | 계속 침 | 없음 | — | waving | fa 547-551 |
| ≈40.7 | — | — | `celebrate` 만료 → Idle → resume가 h3 dispatch. 반응 간격 안이라 정책은 None → 기본값 `Spark`. 좌석 주인 = 편집기, 같은 틱 `NewActivity` 걷기 | 걷기 행 | pr 481, 488-495; ad 197-221, 284-291; beh 294 |
| 41.0 | 계속 침 | 없음. `dispatched_at = 41.0`, `session_typed`. 걷는 중이라 `arrival_pending` 참 | — | 걷기 행 | fa 533-539, 707-715 |
| 40.7 + W | 도착 | — | `did_arrive` → `Spark` | jumping | pr 651-657 |
| 그 뒤 첫 샘플 A | 계속 침 | 없음. `arrived_at = A` | — | jumping | fa 707-715 |
| A + 0.84 뒤 첫 샘플 | 계속 침 | `HighIntensity` | dispatch → `Work` | running | fa 717, 557-564 |

- A가 61.0(= 41.0 + 20) 이후면 61.0에 `HighIntensity`가 먼저 나가 걷는 동안 `Spark` 도착 반응을 덮는다 — 점프 없이 도착해
  running(fa 704-706; ad 424).
- 40.0 전에 사용자가 10초 멈췄으면 인사를 버리고 `AttentionRequired`(후보 아님)를 보내고, 5초 뒤 `ActivityEnded`가 편집기를
  `recent`에서 지운다(fa 540-546, 472-478; ad 140). 그러면 40.0에 넘겨받을 것이 없고, 다음 키가 `ActivityStarted`를
  보낸다(`session_typed`가 거짓, fa 459-464).
- 고정 테스트: 15초 간격과 새 id는 `a_hop_kept_from_the_pet_is_said_again_and_never_given_up`(fa 1144). 자리가 빌 때 치는 중이면
  최신 인사가 점프하는 것은 `a_hop_kept_from_the_pet_plays_when_the_seat_frees_mid_typing`(fa 1284, `Desk`가 dispatch를
  흉내 냄). agent가 끝나면 펫이 한가해진 뒤 dispatch하는 것은 `the_desk_takes_the_seat_the_moment_the_agent_lets_go`(ad 757,
  `is_idle`을 인자로 줌 — 0.7초 값은 테스트 없음). dispatch부터 20초는 `the_cap_on_a_hop_counts_from_when_the_pet_was_handed_it`
  (fa 1183). 실제 런타임은 FT 135 `beside a working agent the work app neither hops, runs nor asks`, FT 185
  `when the agent finishes, the work app takes the seat: walk, hop, then work`(순서만 봄). `W`의 값: 테스트 없음.

### 5c. 펫이 앱 옆에서 잠 — 첫 키가 깨우고 보류된 점프가 나감

가정: 편집기 옆에 앉아 있고(`Present`를 입음), 이 세션에 아직 친 적이 없고, 키보드·포인터 입력이 모두 없다.

| 시각 | 입력 | 일하는 앱이 보냄 | director · 런타임 | 펫이 입는 그림 | 코드 |
|---|---|---|---|---|---|
| 0.0 | 마지막 입력 | — | — | idle | — |
| ≈75.0 | 전체 입력 idle ≥ 75 | — | 검토 박자(0.5초)에 좌석이 유지 가능하고 idle ≥ 75 → `SleepInPlace` → `BeginRest` | sit (idle 행을 빌림) | pl 207, 482-490; pr 603-611, 1128-1142; tuning.rs 48; animation.rs 158 |
| ≈77.4 | — | 이때부터 `pet_resting`. 하트비트 기한이 되면 샘플마다 보류(`news_owed`) | `sit` 2.4초 → `SeekSleepSpot` → 제자리 잠 | sleep | pr 45, 1081-1088, 1145-1152, 1197-1202; fa 467-469, 737-742 |
| 200.0 | 첫 키 (199.9). 샘플은 틱 맨 앞 | 쉬는 중 → 인사 (가): `ActivityStarted` 보류, phase `Typing` | 같은 틱 `finish_tick`에서 idle < 0.8 → 깨움 | wake (stretch 그림) | RR 410, 422; fa 459-464, 619-627, 737-742; pr 509-511, 1204-1212; beh 246-249; capability.rs 97 |
| 200.5 | 계속 침 | `ActivityStarted` 전송 (보류됐던 `Present`는 안 나감 — 인사 중에는 따라잡기가 없음) | 쉬지 않음 → dispatch → 도착 반응 `Spark` → 같은 틱 `hold_seat`가 입힘(`wake`를 끊음) | jumping | fa 450-453, 526-531, 632-642, 743; ad 169, 177, 284-291; pr 1036-1041; beh 229-245 |
| 201.0 | 계속 침 | 없음. `dispatched_at = arrived_at = 201.0` | — | jumping | fa 533-539, 707-715 |
| ≈201.34 | — | — | `spark` 만료 | idle | beh 296 |
| 202.0 | 계속 침 | `HighIntensity` | dispatch → `Work` | running | fa 717, 557-564 |

- 샘플은 `keyboardIdleDuration`(MacUserIdleProvider.swift 41-53)을, 깨움은 `idleDuration`(23-35)을 읽고 둘은 캐시가 따로다.
  위 표는 두 캐시가 같은 틱에 키를 본 경우다. 깨움이 한 틱 늦으면 200.5 샘플도 보류되고 전송이 한 샘플 밀린다
  (fa 526-531). 실기에서 두 캐시의 위상은 확인 못 함.
- 고정 테스트: `a_hop_for_a_resting_pet_goes_out_when_it_wakes`(fa 1807 — 쉬는 샘플에는 없음, 깬 첫 샘플에 `ActivityStarted`,
  다음 샘플에 입음, 그 뒤 0.84초에 `HighIntensity`), `the_cap_on_a_held_back_hop_counts_from_when_the_pet_was_handed_it`(1835),
  `a_heartbeat_held_back_by_a_nap_goes_out_on_waking`(1888), FT 228 `a pet asleep at the seat still hops for the first keystroke`
  (순서만 봄). 휴식 진입의 75초·2.4초를 이 시나리오로 고정하는 테스트는 찾지 못했다.

### 5d. Claude가 자리를 지키는 중 3분 타이핑 뒤 떠남 — 손 흔들기 없음, 1초 뒤 `Stop`

가정: Claude가 자기 창 옆 좌석을 쥐고 10초마다 도구 호출을 보내며, 마지막 호출은 180.0이다. 사용자는 0.0부터 185.0까지
편집기에서 계속 친다(인사는 막힌 채).

| 시각 | 입력 | 일하는 앱이 보냄 | director · 런타임 | 펫이 입는 그림 | 코드 |
|---|---|---|---|---|---|
| 0.0 | 첫 키 | `ActivityStarted` | 후보 아님 | agent 옆 running | fa 459-464; ad 463-470 |
| 15.0 · 30.0 · … | 계속 침 | `ActivityStarted` 15초마다 | 후보 아님 | 그대로 | fa 547-551 |
| 0.5 ~ 185.0 | — | `typed_seconds` 샘플마다 +0.5 → 185.0 | — | — | fa 374-376 |
| 185.5 | 크롬이 앞에 옴 | 없음. `away_since = 185.5` | — | 그대로 | fa 570-573 |
| 188.5 | 3초. `agent_on_duty` 참 (180.0 이벤트가 30초 안) | `ActivityEnded`만, 누적 0 | `recent`에서 편집기 제거. attention·좌석은 agent 것이라 그대로. 다음 후보는 마지막 dispatch id와 같아 pending 없음 | 그대로 | fa 593-605; ad 136-150, 433-448, 479-485 |
| 189.5 | Claude `Stop` | — | `Achievement` dispatch → 좌석 비움 + 반응 → `finish_transient`: 후보가 비어 pending 없음 | waving 0.70초 (간격 안이면 `Glance`) | ad 335-344, 427-448; att 90-93 |
| ≈190.2 이후 | — | 없음 (`Away`) | resume: pending 없음 | idle. 편집기에 대한 반응 없음 | pr 488-495; ad 209 |

- **`Stop`이 떠남 유예(185.5 ~ 188.5) 안에 오면 결과가 다르다.** 코드상 경로: `Stop`의 `finish_transient`가 agent를
  `recent`에서 지우고 좌석을 비워 `agent_on_duty`가 거짓이 되고, `queue_next_candidate`가 아직 `recent`에 있는 편집기의 마지막
  `ActivityStarted`를 pending에 넣는다(ad 427-448, 479-489). 펫이 `Idle`이 되면 그것이 dispatch돼 편집기로 걷기 시작하고
  (ad 197-221), 유예가 끝난 샘플에서 `leave`는 `agent_on_duty` 거짓을 보고 손 흔들기를 보낸다(fa 574-600). 유예 동안 source는
  `leave`의 유예 검사에서 돌아가므로 이것을 막지 않는다(fa 570-573). 사용자가 떠난 뒤 편집기로 걷고 손을 흔드는 셈이다.
  이 경우를 고정하는 테스트는 없다.
- 고정 테스트: `beside_a_working_agent_a_long_sitting_ends_at_once_without_a_wave`(fa 2271 — 끝이 첫 이탈 샘플 + 3초, agent는
  2초 뒤 끝, 편집기 이벤트는 한 번도 dispatch 안 됨), `an_agent_finishing_just_after_the_user_leaves_brings_nothing_of_the_desk_late`
  (fa 2365 — agent 끝이 끝 발신 뒤 1.0초 이내, 이후 10초 반응 없음), `a_long_stretch_left_beside_an_agent_on_duty_ends_without_a_wave`
  (fa 2112 — 누적 0), FT 356 `beside a working agent a long sitting ends without a wave`, FT 389
  `an agent finishing just after a long sitting ends plays nothing of it late`(떠난 뒤 4.5초에 agent 끝).

## 6. 리팩터링 후보 (결정 아님)

각 후보는 "무엇이 지워지고 무엇이 바뀌는가"만 코드 근거로 적는다. 좋다·나쁘다는 판단은 하지 않는다.

**영향 범위를 볼 때 공통으로 닿는 곳.**

- **Swift 대조군.** `Sources/RoamlingEngine/ActivityDirector.swift` `SwiftActivityDirector`(73-)는 앱이 부르지 않는
  대조군이다(68-71 주석). director를 바꾸면 이것도 같이 바꾸고, `CoreLogicTests.swift` 408
  `the Rust activity director agrees with the Swift one`이 둘을 비교한다. attention을 바꾸면
  `Sources/RoamlingCore/AttentionModel.swift`와 `CoreLogicTests.swift` 165 `the Rust attention and reactions agree with the Swift ones`.
- **differential 픽스처.** `rust/roamling-core/tests/activity_differential.rs`는 기록된 `handle`·`expire`·`resume`·`arrive`·
  `sustain` 호출을 잡힘·쉼 입력째로 재생한다(154-181). 이벤트는 `CompanionEvent::new`라 전부 `Agent`이고(154-162, act 103-121),
  `Present`는 픽스처에 없다(19-20). 어느 케이스가 바뀌는 경로를 지나는지는 확인 못 함. `attention_differential.rs` 내용도
  확인 못 함.
- **녹화 세션.** `Tests/RoamlingLogicTests/RuntimeTraceTests.swift` 196-232는 agent 하나의 `activityStarted → highIntensity →
  inspecting → attentionRequired → setback → achievement`를 재생하고 휴식으로 간다. 트레이스는 `frontmost`를 설정하지 않고
  가짜 창 제공자의 기본값은 nil이라(`RuntimeLogicTests.swift` 346, 351) 일하는 앱은 매 샘플 nil 분기에서 돌아간다(fa 368).
  232줄 뒤 스크립트는 확인 못 함.
- **Windows.** `roamling-win`은 같은 `PetRuntime`을 쓴다(`main.rs` 469). director 변경은 그대로 Windows에 간다. 일하는 앱은
  아직 배선 전이라(grep 0건) source 쪽 삭제는 Windows 코드에 닿지 않는다.

### R1. 같은 source 재dispatch에서 도착 반응 다루기

두 갈래가 있고, 지우는 것이 다르다.

**R1-가. 같은 source · 같은 지속 반응이면 도착 반응을 다시 걸지 않는다** (`begin_watching` ad 394-425).

- 지워지는 것: `Present`가 `Calm`이어야 하는 이유(act 33-36, ad 318-321 주석). kind 자체는 남는다 — 반응 없이 걸어와 앉는
  kind는 여전히 필요하다(3.8). 코드 삭제는 거의 없다.
- 바뀌는 것: 같은 agent의 다음 `UserPromptSubmit`(`ActivityStarted`, 지속 `Observe`)에 점프가 다시 걸리지 않는다
  (ClaudeCodeEventNormalizer.swift 81-82; ad 284-291). 막으려면 순간 반응(`Spark`·`Observe`)을 예외로 둬야 한다. 같은 kind
  `HighIntensity` 재발신은 지금도 `work` → `work` 전이가 없어 보이는 변화가 없다(beh 303-306).

**R1-나. 아직 안 입은 도착 반응을 뒤 이벤트가 덮지 않는다 — 뒤 것은 지속 반응으로만 둔다** (ad 424).

- 지워지는 것: `GREETING_DELAY`(fa 77-85), `Greeting.arrived_at`(186-188)과 `greeting_is_over`의 뒤 절반(707-717), 입력
  `arrival_pending`(fa 307-308, 330; ffi.rs 1390; RustCore.swift 969-975, 989; RR 555). `has_arrival_reaction`은 `hold_seat`가
  계속 쓰므로 남는다(pr 1036).
- 남는 것: dispatch 추적(fa 533-551). `recent`가 source당 하나라(ad 153) 막힌 인사를 넘겨받으려면 여전히 `ActivityStarted`가
  최신이어야 한다.
- 바뀌는 것: 점프가 끝나 `idle`이 되면 `hold_seat`가 `sustain_on_seat`로 `Work`·`Paw`를 입힌다(pr 1054-1061; ad 242-250) —
  source가 박자를 기다리지 않아도 running이 온다. `Observe`·`Calm`은 다시 입지 않는다. agent가 걷는 중 `ActivityStarted` 뒤
  `HighIntensity`를 보내면 지금은 도착해서 `Work`만 입고, 바뀌면 점프 뒤 running — agent 쪽에 보이는 변화다.
- 영향: Swift `beginWatching`(ActivityDirector.swift 282-308) 같은 변경. 픽스처에 "안 입은 도착 반응 위에 다음 이벤트"
  케이스가 있으면 출력이 바뀐다 — 있는지 확인 못 함. 녹화 세션은 `activityStarted` 뒤 10초를 걷고 `highIntensity`를
  보낸다(RuntimeTraceTests.swift 214-219) — 그 사이 도착했는지 확인 못 함. Windows: 코어 공유.
- 위험: `did_arrive`의 두 번 입힘 방지(pr 644-652)와 `deliver_arrival_reaction`의 대체값(ad 231-234)이 지금 규칙에 맞춰져 있다.

### R2. 쉬는 펫에게 온 안 깨우는 kind를 버리지 않고 pending으로

- 흡수: `handle_event`의 버림(ad 169-172)을 `CancelRest` 없는 `pending = selected`로.
- 지워지는 것: `tell`의 쉬는 중 관문(fa 737-744), `news_owed`(247-250, 482-487), `is_seat_news`(768-780), 인사 (가) 단계
  (176-180 주석, 526-531), 입력 `pet_resting`(fa 309-312, 331, 337; ffi.rs 1391; RustCore.swift 976-979, 990; RR 556-560).
  `PetRuntime::is_resting`(pr 297-303)은 FFI(ffi.rs 1783) 말고 다른 호출자를 찾지 못했다.
- 바뀌는 것: pending은 `Idle`에서만 꺼내므로(pr 488-494) 키로 깬 펫의 점프는 `wake` 0.7 + `stretch` 1.0 = 1.7초 뒤에 나간다
  (beh 117-118, 292-293) — 지금은 깬 뒤 첫 샘플이다(§5c). pending은 한 칸이라 뒤 이벤트가 덮고(ad 68), 꺼낼 때 나이를 보지
  않는다(197-221). 잠든 펫에게 온 agent 도구 호출이 깨는 순간 오래된 채 dispatch된다 — `wakes_resting_pet` 주석이 말하는
  "일상 진행에는 계속 잔다"(act 44-50)와 달라지는 agent 동작이다.
- 영향: Swift `handle`(ActivityDirector.swift 144-149). 픽스처는 쉼 입력을 재생하므로(activity_differential.rs 163-169) 쉼 +
  안 깨우는 kind 케이스가 있으면 바뀐다 — 확인 못 함. 녹화 세션 휴식 구간(RuntimeTraceTests.swift 230-232)에 agent 이벤트가
  오는지 확인 못 함. Windows: 코어 공유.
- 위험: 오래된 이벤트 재생, 점프가 약 1.2초 늦어짐.

### R3. 거름 대신 출처별 우선순위를 attention에

- 흡수: `candidates`의 거름(ad 463-470)과 resume 버림(216-218)을 `AttentionModel::select`의 출처 계층으로. 지금 `select`는
  `source_type`을 읽지 않는다(att 73-138).
- 지워지는 것 (source 쪽): 거의 없다. `BLOCKED_RESEND`는 남는다 — attention 수명 30초(att 77)와 이벤트 때만 도는 선택(3.1)은
  우선순위와 무관하다. 입력 `agent_on_duty`도 남는다 — 늦은 손 흔들기의 원인은 거른 이벤트가 `recent`에 남는 것
  (ad 153, 430)이지 점수가 아니다.
- 바뀌는 것: `agent_on_duty`의 뒤 절반(좌석 주인 `Agent` + 300초, ad 487-488)은 director 필드라 attention이 볼 수 없다 —
  director가 무언가를 계속 넘겨야 한다. 긴급 kind의 dwell 건너뛰기(att 106-111, 128)가 계층을 넘지 않게 정해야 한다.
- 영향: Swift `AttentionModel`과 `CoreLogicTests.swift` 165. 계층이 `System`만 가르면 agent만 있는 픽스처·녹화 세션은 이론상
  그대로다 — 실행으로 확인 못 함. Windows: 코어 공유.
- 위험: 규칙이 자리를 옮길 뿐 source 쪽 우회는 줄지 않는다.

### R4. 재발신 대신 source가 "자리 유지"를 알리는 API

- 흡수: 예를 들어 `ActivityDirector::renew(source_id, now)` — `recent`에 있는 그 source 이벤트의 시각을 갱신해 attention 수명
  (att 77)을 잇고, 좌석 주인이면 `heard_at`(ad 74)을 갱신한다. `expire_silent`가 `queue_next_candidate`를 부르게 하면
  (ad 185-195) 만료 순간 넘겨받는다.
- 지워지는 것: `HEARTBEAT`(fa 105-110, 455, 467-471), `BLOCKED_RESEND`(95-103), `SeatNews`와 `seat_news`(191-197, 253, 340-344,
  748-752), `resend_due`(764-766), 재발신 호출(492-494, 547-551), "매 이벤트 새 id"(790-792)의 이유 중 재발신 몫.
  `say_where_things_stand`(499-506)는 깬 뒤 따라잡기에 남는다.
- 바뀌는 것: 신선도 점수가 "마지막으로 말한 때" 대신 "마지막으로 갱신한 때"를 본다(att 169-170). `expire_silent`에 큐를
  더하면 agent 좌석이 만료된 뒤 다른 source가 곧바로 큐에 드는 agent 쪽 변화가 생긴다.
- 영향: 두 director + FFI `PetLoop` + `RustPetLoop` + RR 호출에 메서드 하나씩. 픽스처에 `expire` 연산이 있어
  (activity_differential.rs 171) 큐를 더하면 뒤따르는 `resume` 출력이 바뀔 수 있다 — 확인 못 함. 녹화 세션이 300초 침묵에
  닿는지 확인 못 함. Windows: W8 배선 때 이 API를 부르면 된다.
- 위험: 갱신이 끊기면 오늘의 만료와 같다. 갱신되는 이벤트는 attention에서 계속 "신선"하다.

### R5. 떠날 때 손 흔들기 + 자리 비움을 director의 한 동작으로

- 흡수: 예를 들어 director 연산 `farewell(event)` — agent가 자리를 지키면 끝만, 아니면 `Achievement`처럼 반응하되(쉬는 펫이면
  깨우고 pending) 끝을 그 반응이 dispatch된 뒤에 적용한다. dispatch된 `Achievement`는 이미 좌석·`recent`·attention을
  비우므로(ad 335-344, 427-431) 비는 곳은 dispatch되지 못한 손 흔들기뿐이다 — 끝이 pending을 지우는 것(149, 436)과 거른 손
  흔들기가 `recent`에 남는 것(153).
- 지워지는 것: `Goodbye`와 `goodbye`(fa 199-205, 254-256), `WAVE_HOLD_TIMEOUT`(118-133), `goodbye_is_due`·`say_goodbye`
  (667-678), `observe`의 보류 처리(355-361), `arrive`의 `say_goodbye`(409-411), `leave`의 `agent_on_duty` 갈래와 주석(581-605),
  입력 `agent_on_duty`(fa 313-316, 332; ffi.rs 1392; RustCore.swift 980-983, 991; RR 561-565). `PetRuntime::agent_on_duty`
  (pr 305-312)는 FFI(ffi.rs 1789) 말고 다른 호출자를 찾지 못했다.
- 바뀌는 것: 새 kind로 하면 kind가 FFI를 순서로 건너므로 표 끝에만 붙는다(act 38-39; ffi.rs `KINDS`; activity_differential.rs
  21-34; Swift `Activity.swift`). §5d의 "유예 안 `Stop`" 경로는 source가 유예 중일 때 director가 편집기 이벤트를 큐에 넣는
  문제라 이 후보만으로는 닫히지 않는다(ad 430, 433-448).
- 영향: agent는 이 연산을 쓰지 않으므로 agent 동작은 그대로다. Swift 대조군에 같은 연산. 픽스처는 새 kind를 부르지 않으면
  그대로. 녹화 세션은 일하는 앱이 조용하므로 그대로. Windows: W8 배선 전이라 지금 바꾸는 편이 닿는 곳이 적다.
- 위험: pending이 한 칸이라, 쉬는 펫에게 걸린 작별이 `Idle` 전에 다른 source 이벤트에 덮일 수 있다(ad 68, 165-174).

### R6. (추가로 본 것) 이벤트가 아니라 틱에서도 다시 고르기

- 흡수: `begin_tick`에서 펫이 한가할 때 재선택. 지금 선택은 ad 155와 435뿐이다.
- 지워지는 것: 만료 뒤·dwell에 막힌 뒤를 메우던 재발신의 몫. 30초 수명(att 77) 때문에 `BLOCKED_RESEND` 자체는 남는다.
- 바뀌는 것: 매 틱 `select`가 attention 상태를 바꾼다(att 90-93, 96, 136) — agent만 있는 흐름에서도 `clear`·`acquire` 시점이
  달라진다. `begin_tick`의 난수 소비 순서가 녹화 세션과 묶여 있다(pr 484-487 주석) — 영향 확인 못 함.
- 위험: 넓다. 지워지는 source 코드는 적다.

## 7. 기존 문서와 다른 점

대조 범위: `CLAUDE.md` G(350-452), `docs/behavior-flow.md` §5b(273-453)와 §6의 일하는 앱 행(471, 476-486),
`docs/windows.md` W8(1630-1704), `docs/architecture.md`의 일하는 앱 문단(70-79, 130-148). 과거 경위를 말하는 문장은 대조하지
않았다. `docs/architecture.md`에서는 코드와 어긋나는 문장을 찾지 못했다. 아무것도 고치지 않았다.

1. **인사 뒤 박자.** `docs/behavior-flow.md:284`와 `:483`은 "펫이 받아 입은 뒤 0.84초", `CLAUDE.md:358`은 "점프, 이어서
   running"이라고 한다. 코드는 0.84초를 source가 "dispatch됐고 갚을 반응이 없다"를 **본 샘플**부터 센다(fa `greeting_is_over`
   707-717). 앉은 펫이면 그 샘플이 점프 0.5초 뒤라, running은 점프 시작 1.5초 뒤 샘플에 나가고 사이에 약 0.66초 idle이 있다
   (§5a). 그때 키가 멈춰 있으면 running 대신 `attentionRequired`다(fa 560-564).
2. **인사 시계의 기준.** `docs/behavior-flow.md:355-356`은 "인사 박자와 20초 상한은 그 발신 시각부터 센다"고 한다. 코드는 20초를
   `dispatched_at`(dispatch를 본 첫 샘플)부터(fa 533-539, 704-706), 박자를 `arrived_at`부터(707-717) 센다. 같은 절 `:338`
   ("dispatch된 뒤부터")과도 다르다.
3. **셸이 넘기는 것.** `docs/behavior-flow.md:276-277`은 "셸이 0.5초마다 두 가지만 읽는다"고 한다. 코드의 입력은 일곱이다 —
   기계에서 둘, 설정에서 하나, 펫에게서 넷(RR 543-566). 간격은 "앞 샘플 뒤 0.5초 이상 지난 첫 틱"이다(RR 541).
4. **"3분 쳤으면"의 뜻.** `CLAUDE.md:364`·`:424`, `docs/behavior-flow.md:293`·`:372`·`:480`, `docs/windows.md:1645`. 코드의 누적은
   키를 친 시간이 아니라 **`Typing` 단계에 있던 시간**이다(fa 374-376). 마지막 키 뒤 10초 창과 막힌 인사를 기다리는 시간이
   들어간다. 테스트 주석이 "키 4.5초 + 창 10초 = 누적 14.5초"를 적어 둔다(fa 1513-1518).
5. **지정 앱끼리의 전환.** `docs/behavior-flow.md:292`("다른 앱이 3초 미만 앞에 옴 → 없음"), `:305-307`과 `CLAUDE.md:372-373`
   ("2분 미만 자리 비움도 같은 세션"). 코드에서 3초 유예는 지정 **안 된** 앱에만 있다(fa 378-379 → `leave` 570-573). 지정된 다른
   앱은 곧바로 `ActivityEnded` + `Present`이고(388-389, `arrive` 415-437) `left_at`이 없어 새 세션이다(419-426). A → B → A로 몇 초
   만에 돌아와도 A는 새 세션이고 점프가 다시 걸린다.
6. **"agent 신호가 30초 안에 왔거나".** `CLAUDE.md:407-408`, `docs/behavior-flow.md:413-414`. 코드의 `agent_on_duty` 앞 절반은
   **`recent`에 남아 있는** `Agent` 이벤트만 본다(ad 481-485). `recent`는 source당 마지막 하나이고, dispatch된 `Achievement`(Stop)는
   `finish_transient`가(428), `ActivityEnded`는 140이 지운다. 방금 dispatch된 `Stop`은 세지 않는다. §5d의 "유예 안 `Stop`" 경로가
   여기서 나온다.
7. **dwell 안에 온 agent.** `docs/behavior-flow.md:417-418`은 "agent 이벤트가 오면 … agent가 가져간다(attention의 dwell 3초는
   그대로)"라고 한다. 코드에서 일하는 앱이 attention을 잡은 뒤 3초 안에 온 긴급 아닌 agent 이벤트는 `select`가 None을 돌려줘
   dispatch되지 않는다(att 112-133 — 현재 source의 이벤트를 거른 목록에서 찾는다). 틱에서 다시 고르지 않으므로(3.1) 3초가 지나도
   저절로 가져가지 않고, 다음에 누군가 이벤트를 보낼 때 가져간다. 이 경우를 고정하는 테스트는 없다(ad 787-807은 8초 뒤를 본다).
8. **"창 옆으로 걸어와 앉음".** `docs/behavior-flow.md:283`, `CLAUDE.md:357`. 코드는 placement가 `NewActivity` 목적지를 받아들일 때만
   걷는다(pl 660-671). 지금 선 자리가 창을 보고 새 좌석이 15점 넘게 낫지 않으면 그 자리에 앉는다(pl 444-455, 468-477). 쉬는
   펫에게는 깰 때까지 보내지 않는다(fa 737-742).
9. **"agent 옆에 그대로".** `docs/behavior-flow.md:296`, `docs/windows.md:1649`, `CLAUDE.md:409-410`("펫은 Claude 옆에 남고").
   agent 이벤트에는 창이 없어서(`Sources/RoamlingSources`·`rust/roamling-agent/src`에서 hint를 채우는 코드 grep 0건) 셸이 그 순간
   앞 창을 채운다(RR 576-579; `main.rs` 465-467). 사용자가 편집기에서 치는 동안 온 agent 이벤트는 편집기 창을 좌석 창으로
   받는다(ad 403-407). 좌석 주인은 agent 그대로지만 펫이 편집기 쪽으로 옮기는지는 placement 판정(pl 578-609) 몫이고, 실기에서
   확인 못 함.

## 부록: 이 문서에서 확인 못 한 것

- 실기에서 샘플 간격과 두 입력 캐시(`idleDuration`·`keyboardIdleDuration`)가 만드는 지연의 크기(§1, §5c).
- agent 이벤트가 앞 창을 받을 때 실기 좌석 위치(§1, §5b, §7-9).
- Swift `AnimationResolver`와 Rust `petdex_state`가 같은지 — `animation_differential.rs` 내용을 읽지 않았다(§1).
- §6 각 후보가 activity·attention 픽스처와 녹화 세션을 실제로 바꾸는지 — 실행하지 않았다. `attention_differential.rs` 내용과
  `RuntimeTraceTests.swift` 232줄 뒤 스크립트도 읽지 않았다.
- dispatch 없이 attention 상태가 바뀌는 것(3.1)의 실사용 영향.
- 걷는 시간 `W` — 거리와 placement 목적지에 달렸다.

