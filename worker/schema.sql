-- 리더보드 한 줄 = entry 한 행.
--
-- line 은 web/src/store.rs 의 `encode` 형식(탭 구분)과 정확히 같아야 합니다.
--   run    : 이름 \t 성공(0|1) \t 정확도 \t 밀리초 \t 등록시각
--   battle : 이긴쪽 \t 진쪽 \t 무승부(0|1) \t 이긴셀 \t 진셀 \t 세대 \t 등록시각
--
-- dedupe 는 "질의문자열@10분버킷" — 같은 QR 을 연달아 두 번 등록하는 사고만 막고,
-- 한참 뒤에 우연히 값이 똑같은 경기가 나오는 것은 막지 않습니다.
create table if not exists entry (
  id     integer primary key autoincrement,
  kind   text    not null,
  round  integer not null,
  dedupe text    not null unique,
  line   text    not null,
  at     integer not null
);

create index if not exists entry_lookup on entry (kind, round, id);
