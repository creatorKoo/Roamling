# 문서 지도

**질문에서 시작한다. 문서 이름에서 시작하지 않는다.**

규칙과 함정은 `CLAUDE.md`에 있다 — 매 세션 자동으로 읽히는 유일한 파일이고, 여기 있는
문서들은 그 규칙의 근거를 담는다.

## 지금 도는 것

| 묻는 것 | 문서 | 줄 |
|---|---|---|
| 사용자가 원한다고 말했는데 아직 없는 것 | [`requests.md`](requests.md) | ~100 |
| 잠깐 숨기기와 끄기 발견성 (R2·R3) — 설계 | [`hiding.md`](hiding.md) | ~80 |
| 모듈이 어떻게 갈리고 의존이 어디로 흐르나 | [`architecture.md`](architecture.md) | ~630 |
| 어떤 상황에 펫이 어떤 그림을 입나, 얼마나 오래 | [`behavior-flow.md`](behavior-flow.md) | ~560 |
| 펫이 **어디에 설지**를 어떻게 정하나 | [`placement.md`](placement.md) | ~510 |
| 활동 source가 두 종류인 이유, 상태 낱말 | [`state-sources.md`](state-sources.md) | ~160 |
| Petdex 어휘를 어떻게 아래에 두나 | [`state-contract.md`](state-contract.md) | ~290 |
| capability 16종 ↔ Petdex 9종, 무엇을 빌리나 | [`pets.md`](pets.md) | ~380 |
| 지금 시트에 무엇이 그려져 있나 | [`art/mochi-sheet.md`](art/mochi-sheet.md) | ~70 |
| 프레임을 새로 그릴 때의 불변식 | [`art/mochi-animation-handoff.md`](art/mochi-animation-handoff.md) | ~140 |
| 도구 없이 손으로 프레임을 만들 때의 프롬프트 | [`art/mochi-animation-prompts-ko.md`](art/mochi-animation-prompts-ko.md) | ~150 |
| Windows에서 지금 유효한 것, 남은 게이트(W8) | [`windows.md`](windows.md) | ~300 |
| 서명·dmg·배포를 어떻게 하나 | [`release.md`](release.md) | ~90 |
| 무엇이 실제로 배터리를 먹나 | [`battery.md`](battery.md) | ~90 |

## 닫힌 기록 — [`history/`](history/)

**지우지 않는다.** `CLAUDE.md`가 여러 곳에서 "이 실측을 다시 재지 마라"며 이것들을 가리킨다.
평소에 읽을 것은 아니고, **결정을 뒤집으려 할 때** 읽는다.

| 무엇 | 문서 | 줄 |
|---|---|---|
| Windows 게이트 W0~W7, 언어 선택, 스파이크 실측 | [`history/windows.md`](history/windows.md) | ~2410 |
| MVP 0~4 — 전부 닫힘 (2026-09-07) | [`history/mvp.md`](history/mvp.md) | ~430 |
| 일하는 앱이 사건형이던 때의 구조와 우회 코드 | [`history/focus-activity-flow.md`](history/focus-activity-flow.md) | ~670 |
| Mochi v3를 만들기 전의 진단과 계획 | [`history/mochi-v3-plan.md`](history/mochi-v3-plan.md) | ~500 |
| upstream 조사 (Petdex · Codex · macOS API) | [`history/research.md`](history/research.md) | ~470 |
| 맥 수정 A~F — 전부 닫힘 | [`history/mac-fixes.md`](history/mac-fixes.md) | ~80 |
| v2 시트의 기록 | [`history/mochi-v2-animation-spec.md`](history/mochi-v2-animation-spec.md) | ~280 |

## 이 지도의 규칙

- **문서 하나는 "지금 이렇다"이거나 "이렇게 됐다"이지, 둘 다일 수 없다.** 섞이면 읽는 사람이
  매번 어느 문단이 아직 참인지 판정해야 하고, 그 판정은 틀린다. 2026-09-12에 흐름의 정본이던
  `behavior-flow.md` §5b가 통째로 거짓이 돼 있었고 우연히 잡혔다.
- 기능을 다시 지으면 **그 기능을 설명하는 문서를 같은 작업 안에서** 고친다. `scripts/test.sh`는
  문서를 안 본다.
- 닫힌 게이트는 `history/`로 내리되 **그 안에 있던 살아 있는 사실은 위로 끌어올린다.**
  `windows.md`가 그랬다 — 설치 경로·설정 파일·업데이트 운영이 닫힌 게이트 본문에 묻혀 있었다.
- **`CLAUDE.md`에는 규칙과 함정만 둔다.** 절차는 필요할 때 와서 보면 되고, 매 세션 자동으로
  읽히는 파일에 미리 실어 둘 값이 없다. dmg 내부 구조가 그래서 `release.md`로 내려갔다.
