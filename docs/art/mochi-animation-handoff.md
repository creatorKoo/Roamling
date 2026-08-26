# Mochi animation art handoff

이 문서는 ChatGPT 이미지에 그대로 전달할 수 있는 제작 brief다. 현재
`mochi-runtime-atlas.png`는 승인본이 아니라 결함 참고 자료다. 새 결과를 받기 전까지
그 파일의 픽셀을 새 원화의 일부로 재사용하지 않는다.

## 첨부할 파일과 역할

1. `mochi-poses.png` — **identity source of truth**
   - 첫 번째 idle의 얼굴, 큰 눈, 체형, 목, 무늬, 색, outline을 기준으로 삼는다.
   - walking/sleeping/caught pose도 자세의 기준으로 사용한다.
2. `fat-mochi-runtime-atlas.png` — **motion taxonomy reference only**
   - idle/walk/sleep/caught/drag/stretch/landing이라는 동작 종류와 흐름만 참고한다.
   - FatMochi의 얼굴, 체형, 눈, 무늬를 Mochi에 복사하지 않는다.
3. `mochi-runtime-atlas-REJECTED.png` — **defect reference only**
   - 깨진 조각, 프레임 경계 잔여물, 부분 합성, 체형 변화가 무엇인지 보여 주는 반면교사다.
   - 이 이미지에서 어떤 픽셀도 복사하거나 수정해서 최종 원화로 사용하지 않는다.

## 반드시 지킬 제작 방식

- 최종 8×7 atlas를 한 번에 생성하지 않는다.
- animation strip을 하나씩 만들고 승인받은 뒤 다음 strip으로 진행한다.
- **각 프레임마다 머리부터 꼬리와 네 발까지 고양이 전체를 완성된 한 장으로 그린다.**
- 눈만, 발만, 얼굴만 생성한 뒤 기존 그림에 붙이지 않는다.
- body/face/limb patch, cut-and-paste, inpainting fragment, 기존 프레임 조각 재사용을 금지한다.
- 한 strip 안의 모든 프레임은 동일한 canvas, scale, ground line, body center를 사용한다.
- 프레임 사이에서 얼굴 크기, 눈 크기, 목 길이, 몸통 비율, 무늬 위치가 변하지 않는다.
- 모든 visible pixel은 해당 프레임의 고양이에 연결되어야 한다. 떨어진 수염, 털, 발,
  꼬리 조각, 이전/다음 프레임 잔여물은 없어야 한다.
- 고양이는 각 panel 안에 완전히 들어가고 panel 좌우에 충분한 빈 공간을 둔다.
- panel 경계를 다른 고양이의 수염, 꼬리, 발이 넘지 않는다.
- flat `#00FF00` 배경만 사용한다. 그림자, glow, 바닥, texture, gradient, 글자, grid,
  watermark를 넣지 않는다.
- pixel art edge는 선명하게 유지하고 blur나 painterly antialiasing을 쓰지 않는다.

## Runtime frame contract

Roamling이 최종적으로 사용하는 cell은 `192×208px`, `8 columns × 7 rows`다. 하지만
ChatGPT 이미지는 아래 source strip만 제작한다. Roamling 쪽에서는 완성된 **전체 프레임**에
대해서만 chroma 제거, 동일 비율 축소, 중앙 정렬, full-frame mirror, atlas 배치를 한다.

| Source | Frames | Required motion |
|---|---:|---|
| `mochi-idle` | 8 | open → half close → closed → half open → open, subtle breathing |
| `mochi-walk-right` | 8 | four-beat gait, all four short paws alternate |
| `mochi-sleep` | 4 | slow reversible breathing |
| `mochi-caught-drag` | 8 | 4-frame caught intro + 4-frame compact four-paw scramble loop |
| `mochi-stretch` | 6 | real feline forward stretch/play bow and return |
| `mochi-hop-landing` | 6 | low celebration hop, squash, exact idle recovery |

왼쪽 걷기는 승인된 `mochi-walk-right`의 **전체 프레임**을 local mirror해서 만든다. 부분
mirror나 얼굴/발 단위 mirror는 하지 않는다.

## ChatGPT 이미지 첫 요청 — 그대로 복사

아래 prompt와 위 세 이미지를 함께 첨부한다.

```text
Use case: identity-preserve
Asset type: final production pixel-art animation sources for the Roamling macOS desktop companion

Input image roles:
- Image 1, mochi-poses.png: the only identity source of truth. Preserve this Mochi exactly.
- Image 2, fat-mochi-runtime-atlas.png: motion-category reference only. Never copy FatMochi's body, face, eyes, expression, or markings.
- Image 3, mochi-runtime-atlas-REJECTED.png: rejected defect reference only. It shows what must not be repeated. Do not reuse or repair any pixels from it.

Production workflow:
Do not generate the final 8x7 runtime atlas. Work on one animation strip at a time, beginning with WALK RIGHT only. Wait for my approval before creating another animation.

Create exactly eight chronological full-body frames of Mochi walking to the right in one horizontal strip. Every frame must be a newly rendered, complete cat from ears to tail and all four paws. Never generate or replace only a face, eye, paw, limb, or body fragment. Never splice, composite, inpaint a fragment, or reuse a cropped piece from another frame.

Identity invariants:
Preserve Image 1's exact compact calico character: huge round brown-black eyes, small pink nose and mouth, alert ears, short visible neck, compact torso, four very short feline paws, striped upright tail, orange/brown/cream marking shapes, palette, pixel size, outline weight, and cute expression. The face, head size, neck, torso proportions, markings, and tail thickness must remain consistent in every frame.

Walk motion:
Use a readable natural four-beat feline gait. All four paws must visibly change position during the cycle. The near forepaw must plant, pass under the shoulder, lift in a compact recovery, and extend again. The far forepaw uses the opposite phase. Both hind paws alternate in opposing phases. Include clear planted-contact and lifted-recovery frames. Keep the torso and head stable with only a tiny body bob and restrained tail follow-through. Use a fixed ground line and fixed body center. The cycle must loop without sliding, floating, or moonwalking.

Frame integrity:
Each of the eight cats must be fully contained in its own equal-width panel with generous empty space on both sides. No whisker, tail, paw, fur pixel, or outline may touch or cross a panel boundary. No pixel or fragment from one cat may appear in a neighboring panel. Every visible pixel belonging to a frame must connect to that frame's cat; no detached debris or floating marks.

Backdrop and style:
Perfectly flat solid #00FF00 chroma-key background, uniform edge-to-edge. No shadows, gradients, texture, floor plane, reflections, glow, labels, panel lines, grid, extra objects, extra animals, text, or watermark. Crisp deliberate pixel-art edges; no painterly blur.

Before returning the image, inspect all eight panels for: complete cat, exactly four paws, identical identity and markings, fixed center and ground line, safe margins, no overlap, no clipped pixels, and no detached fragments. If any frame fails, redraw the entire failing cat rather than patching part of it.
```

## 승인 후 동작별 follow-up prompt

매 단계에서 공통으로 다음 문장을 먼저 붙인다.

```text
Keep every identity, full-frame rendering, safe-margin, connected-pixel, flat #00FF00 background, and no-partial-compositing constraint from the approved Mochi brief. Render every frame as a complete cat. Do not modify or patch only a body part.
```

### Idle — 8 frames

```text
Create the IDLE strip: exactly eight full-body front-facing frames in one horizontal row. Start and end on Image 1's exact approved idle appearance. Animate one soft natural blink: open, beginning to close, half closed, fully closed, half open, open, subtle breath, exact idle. The whole cat is fully rendered in every frame. Keep body, head, face shape, tail, paws, scale, center, and ground line consistent; only the natural pose state changes between complete drawings.
```

### Sleep — 4 frames

```text
Create the SLEEP strip: exactly four complete curled-sleeping Mochi frames in one horizontal row. Use Image 1's sleeping pose as the identity anchor. Animate a tiny reversible inhale and exhale while the eyes remain closed, paws tucked, tail curled, and ground contact fixed. No body sliding or wake-up pose.
```

### Caught and dragged — 8 frames

```text
Create the CAUGHT-DRAG strip: exactly eight complete front-facing frames in one horizontal row. Frames 1-4 transition from the exact approved idle cat into Image 1's surprised belly-facing caught pose. Frames 5-8 are a seamless restrained four-paw scrambling loop. All four very short paws alternate close to the belly in opposing diagonal pairs. It must read as a caught cat paddling in the air, never as human arms waving. Limb length and body proportions remain fixed.
```

### Feline stretch — 6 frames

```text
Create the STRETCH strip: exactly six complete frames in one horizontal row. Exact idle, both forepaws slide forward, chest and head lower, back lengthens, hindquarters remain raised, deepest feline play-bow, then exact idle recovery. This must be a real cat wake-up stretch. Never stand upright or lift the forepaws like human arms.
```

### Hop and landing — 6 frames

```text
Create the HOP-LANDING strip: exactly six complete front-facing frames in one horizontal row. Exact idle, compact crouch, low rise with four short paws tucked, small airborne apex, soft landing squash, exact idle recovery. Keep the horizontal center fixed. No high jump, long limbs, confetti, separate effects, or sideways travel.
```

## 결과를 Roamling에 넘길 때

- ChatGPT 화면 screenshot이 아니라 **다운로드한 원본 PNG**를 전달한다.
- 이미지가 자동 resize/compress되지 않도록 파일로 첨부한다.
- 승인된 strip 여섯 개를 모두 전달한다.
- 가능하면 각 strip을 만든 대화에서 마지막으로 사용한 prompt도 함께 전달한다.
- Roamling은 수령 후 전체-frame 단위 packing만 수행하고, 자동 검사에서 빈 frame,
  boundary contact, detached component, identity/center drift가 발견되면 임의 수선하지 않고
  해당 complete frame의 재제작을 요청한다.

## 현재 결함 기록

2026-08-27 검사에서 rejected Mochi atlas 56프레임 중 22프레임에 본체와 떨어진 작은 alpha
component가 있었다. 원인은 생성 strip이 정확한 equal-cell layout이 아닌데 균등 폭으로
분할한 뒤 각 crop을 다시 중앙 정렬한 것이다. idle에는 eye-region patch 합성도 사용했다.
두 방식 모두 새 asset workflow에서 금지한다.
