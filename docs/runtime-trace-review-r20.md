# R20 · R21 런타임 녹화 기준 검토

잠자리 선택을 `PlacementDirector`로 옮기고(R20) 기지개를 끝까지, 더 길게 켜게 한(R21) 변경은
`Tests/RoamlingLogicTests/RuntimeTrace.txt`를 어긋나게 한다. 어디가 어긋나야 하는지는 코드를 쓰기
**전에** `placement.md` 3.5.5에 적어 두었다. 이 기록은 제안 녹화가 그 예고와 맞는지 대조한 것이다.
통과시키려고 다시 만든 것이 아니다 — 예고에 없는 차이가 있었다면 갱신하지 않고 원인을 찾는다.

사용자는 2026-09-19에 "차이를 검토하고 갱신한다"는 절차를 승인했고(`requests.md` R20), 같은 날 아래
차이를 보고 "응 승인"이라고 했다. 기지개는 새 빌드로 직접 보고 "더 자연스러워서 좋음".

## 원본과 산출 근거

- 이전 기준: `Tests/Fixtures/runtime/RuntimeTrace-pre-r20.txt`. 교체 전 기준과 보존본의 Git blob이 모두
  `17a72083e324f75d70c394c4128776ba34760e52`,
  LF 기준 SHA-256 `ae56e315dd41b5e1dc1edeb9a4b18c8ad048531f89a99bbc3c7871a279222737`
  (0.6.5 검토가 적은 새 기준의 값과 같다 — 그 뒤로 바뀐 적이 없다).
- 새 기준: `Tests/RoamlingLogicTests/RuntimeTrace.txt`. 산출물 SHA-256
  `4d2e7f8e0b52d13a845a68b8c3a4101bbc144d617630df18df9b70dfddd9dc84`, 놓은 뒤의 Git blob
  `c8fb7868074f17cf7bed814d7d9765e90167d1d7` — 산출물을 LF로 해시한 값과 같다. 손대지 않았다.
- 산출 커밋: `3bbb6994b456e723e2a881d8aeb646a16342c52a` (`rest-one-layer` 브랜치).
- [검토용 macOS 녹화 실행](https://github.com/creatorKoo/Roamling/actions/runs/35439013320): `scripts/test.sh`
  전체가 산출 모드로 통과했다 — Swift 하네스(내장 `stretching` 2.60초 검사 포함), Rust 코어와
  differential. 이 실행의 녹화 검사는 비교가 아니라 산출이다.
- 두 녹화 모두 2,297틱이고 입력 시나리오·시드·단계별 틱 수는 같다.

## 전체 차이 분류

모든 틱을 같은 단계·순번으로 놓고 좌표·행동·이동 중 여부·클릭 허용·난수 카운터를 비교했다.

| 구간 | 비교 결과와 원인 |
|---|---|
| roam 300 · cursor 180 · lunge 19 · grab 1 · held 15 · drag 40 · drop 1 · land 90 | 모든 필드 동일 |
| travel 300 · busy 120 · review 90 · ask 120 · setback 90 · done 150 | 모든 필드 동일 |
| **rest 420** | **모든 필드 동일 — 좌표, 행동, 난수 카운터까지.** agent 없는 잠자리 선택(`sit → findSleepSpot → 128,779 → sleep`)은 주인만 바뀌고 같은 답을 같은 tick에 낸다는 뜻이다. R20이 이 경로에 요구한 것이 정확히 이것이다 |
| displays 1 | 동일 |
| wander 360 | 53번째 틱부터 다르다. 앞 52틱(잠 · `wake`)은 같다. `stretch`가 30틱에서 57틱으로 늘었고(1.0 → 1.9초), 그 뒤의 `idle` 23틱은 같다 — 기지개 뒤 0.8초의 숨이 그대로다. 첫 산책은 그만큼 늦게 시작하고 **목적지가 다르다**(1650,652 → 468,759): 코어는 tick마다 난수를 뽑으므로 27틱이 더 지난 뒤의 추첨은 다른 점을 낸다. 새 목적지가 가까워 71.2초에 도착해 앉고, 옛 녹화 끝의 다른 디스플레이로 건너가던 구간이 없어졌다 |

진단 로그의 차이는 아래가 전부다.

```diff
+     52.9  place rest at 128,779
      52.9  rest tucking into a safe zone, spot unvetted
      52.9  pet findSleepSpot
      58.1  pet sleep
+     58.1  place sleep in place
      64.5  pet wake
+     64.5  place hold
      64.5  rest waiting for user idle
      65.2  pet stretch
-     66.2  pet idle
-     67.0  place stroll to 1650,652
-     67.0  pet wander
-     67.0  place hold
-     75.3  place none, something else owns the pet
+     67.1  pet idle
+     67.9  place stroll to 468,759
+     67.9  pet wander
+     67.9  place hold
+     71.2  pet idle
```

- `place` 세 줄 — 잠자리가 director의 의도가 됐으므로 진단에 찍힌다. 예전에는 휴식 층이 낸 걸음이라
  47.5초의 `place hold`가 67.0초까지 이어졌다. B6을 진단할 때 없어서 아쉬웠던 줄이다.
- `66.2 → 67.1`, `67.0 → 67.9` — 기지개 0.9초. `WAKE_WANDER_DELAY`가 같은 만큼 늘어 간격은 0.8초 그대로다.
- `75.3 place none` — 옛 산책이 디스플레이 경계를 건너는 동안 찍히던 줄이다. 새 산책은 같은 화면 안에서
  끝나 건너지 않는다. 경계 통과 로직의 변화가 아니라 목적지의 차이다.

**예고(`placement.md` 3.5.5)에 없던 차이는 없다.** 예고는 "① 휴식 구간의 `place` 줄 ② 기지개가 0.9초
늦추는 `stretch → idle`과 첫 산책, 난수 순서가 tick 수에 묶여 있으면 그 뒤 전부"였다.

화면 캡처는 녹화 시나리오에서 꺼져 있으므로 agent 곁의 잠자리(B6), 격자 후보(B7)의 근거로 이 녹화를
쓰지 않는다. 그것은 `clearance_tests.rs`의 `rest_*` 넷이, 기지개는 `pet_runtime/rest_tests.rs`의 일곱이 본다.

## 재현성 확인

산출 실행은 비교를 하지 않았으므로 기준을 바꾼 뒤의 일반 검증이 첫 비교다.
[Check macOS](https://github.com/creatorKoo/Roamling/actions/runs/35439561271)가 커밋 `c0d3bd6`에서
`ROAMLING_WRITE_TRACE` 없이 통과했다 — `a recorded session replays tick for tick`이 새 기준과 바이트로
맞았고, Rust 코어 89개와 differential, 서명 빌드와 패키징된 앱의 실행까지 같은 실행에서 통과했다.
