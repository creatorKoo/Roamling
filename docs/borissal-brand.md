# 보리쌀 표시 이름과 아이콘

2026-10-04 사용자 확정: 앱 이름은 보리쌀, 아이콘은 보리와 쌀의 얼굴을 함께 쓴다.
한국어 UI는 보리쌀, 영어 UI는 BoriSsal이다. 캐릭터 이름 보리와 쌀은 유지한다.

표시 문구는 `Sources/RoamlingShell/Resources/{ko,en}.lproj/Localizable.strings`,
Windows 파일 속성은 `rust/roamling-win/roamling.rc`, macOS 앱 표시 정보는
`Support/Info.plist`에서 바뀐다. 설정 폴더와 키, 실행 파일명, 번들 ID,
업데이트 주소·제품 내부 식별자는 유지해 기존 설치의 설정과 갱신을 이어간다.

원본 아이콘은 `assets/icon/borissal.png`이며 `scripts/build-icon.py`가 ICO/ICNS와
메뉴 크기를 만든다. Windows 트레이는 실행 파일에 포함된 동일한 ICO 리소스를 쓴다.
기존 크기별 발바닥 글리프 생성 방식은 새 얼굴 아이콘으로 교체한다.
이미지는 기본 imagegen 도구로 생성했다. 프롬프트 요약: 승인된 삼색 고양이 보리와
흰 스피츠 쌀의 얼굴 둘, 단순한 픽셀 아트, 투명 배경, 글자 없이 작은 앱 아이콘으로.

macOS 빌드와 실제 메뉴바·권한 표시 확인은 macOS에서 수행해야 한다.
Android는 별도 비공개 저장소에서 표시 이름과 아이콘 리소스를 연결해야 한다.

## Windows 적용 확인

공통 release 검사·Windows 셸 67개 검사(네트워크 1개 ignored)와 release 빌드가 통과했다.
임시 설정에서 실제 앱 명령으로 내장 쌀 9색·보리 전환·재시작 복원을 확인했다.
실사용 실행 파일의 ProductName/FileDescription은 모두 보리쌀이며 버전은 기존 0.6.9다.
현재 쌀 털색을 유지하며 승인 눈색으로 맞췄고, 그 외 설정 행은 그대로임을 확인했다.
트레이의 실제 시각적 확인은 사용자의 Windows 체감 검토가 남는다.
`output/check-borissal.ps1`, `output/borissal-smoke.log`, `output/borissal-restart.log`가 검증 기록이다.
