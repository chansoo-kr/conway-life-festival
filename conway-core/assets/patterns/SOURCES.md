# 패턴 출처

이 디렉터리의 파일은 시작 시 자유 모드 프리셋으로 자동 등록됩니다 (`.rle`, `.mc` 지원, 첫 줄 `#N 이름`이 표시 이름).

| 파일 | 내용 | 원본 |
|---|---|---|
| `clock_pm.rle` | 디지털 시계 (10016×6796). 저장된 시각에서 출발해 일정 세대마다 1분씩 넘어가며, 실제 시각과 동기화되지는 않음 | 기존 프로젝트 파일 |
| `computer_8bit_loizeau.mc` | 8비트 프로그래머블 컴퓨터 — ALU, RAM, 프로그램 메모리 (Nicolas Loizeau) | https://github.com/nicolasloizeau/gol-computer (`computer.mc`) |
| `computer_16bit_display_loizeau.mc` | 16비트 컴퓨터 + 픽셀 디스플레이, 변수 8개·프로그램 16줄 (Nicolas Loizeau) | https://github.com/nicolasloizeau/scalable-gol-computer (`patterns/computer_16_8_16.mc`) |

`.mc` 파일은 저장 당시 세대까지 진행된 상태라 멀리 날아간 글라이더가 포함돼 있습니다. 로더가 가장 밀집된 연결 영역(기계 본체)만 잘라내어 등록합니다.
| `turing_machine.rle` | 3상태 3기호 튜링 머신 (Paul Rendell) | Golly `Patterns/Life/Signal-Circuitry/Turing-Machine-3-state.rle` |
| `fermat_primes.rle` | 페르마 소수 계산기 (Jason Summers) | Golly `Patterns/Life/Miscellaneous/fermat-primes.rle` |
| `twin_primes.rle` | 쌍둥이 소수 계산기 (Dean Hickerson) | Golly `Patterns/Life/Miscellaneous/twinprimes.rle` |
| `memory_tape.rle` | 프로그램 가능한 구성기 + 메모리 테이프 (Paul Chapman) | Golly `Patterns/Life/Signal-Circuitry/constructor-memory-tape.rle` |
| `unit_life_cell.rle` | 512×512 단위 생명 세포 — 생명 게임 속 생명 게임 (David Bell) | Golly `Patterns/Life/Signal-Circuitry/Unit-Life-Cell-512x512.rle` |

Golly 패턴 컬렉션: https://github.com/AlephAlpha/golly (Patterns/). 각 파일의 저작권은 원저자에게 있습니다.
