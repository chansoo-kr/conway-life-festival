-- 축제 부스 리더보드.
--
-- GitHub Pages 는 정적 호스팅이라 서버가 없습니다. 기록은 Supabase 에 모으고,
-- 브라우저는 PostgREST 의 RPC 세 개만 부릅니다:
--
--   board_lines(p_kind, p_round)                 순위표를 줄 단위 텍스트로
--   submit_result(p_q, p_k, p_name, p_foe)       결과 QR 의 질의문자열 + 서명으로 등록
--   wipe_board(p_kind, p_round, p_token)         초기화 (관리자 토큰 필요)
--
-- 표에는 직접 쓰지 못합니다(RLS 에 insert 정책이 없음). 등록은 반드시 submit_result 를
-- 거치고, 거기서 서명을 다시 계산해 맞는 값만 받습니다. 그래서 주소창에 숫자를 아무렇게나
-- 적어 넣은 결과는 들어오지 않습니다. 다만 서명 비밀값은 게임 바이너리 안에 들어 있으니
-- "장난을 조금 귀찮게" 만드는 수준이지 강한 보안은 아닙니다.

-- ------------------------------------------------------------------ 설정 보관

-- 노출 스키마가 아니라서 publishable 키로는 읽을 수 없습니다.
create schema if not exists private;

create table if not exists private.config (
  key   text primary key,
  value text not null
);

-- conway-core/src/qr.rs 의 SUBMIT_SECRET 과 같아야 합니다.
insert into private.config (key, value)
values ('submit_secret', 'conway-life-festival')
on conflict (key) do nothing;

-- 부스 진행 요원이 '기록 지우기' 에서 입력할 값. 배포 뒤 대시보드에서 바꾸세요:
--   update private.config set value = '<정한 값>' where key = 'admin_token';
insert into private.config (key, value)
values ('admin_token', 'change-me')
on conflict (key) do nothing;

-- ---------------------------------------------------------------------- 표

-- line 은 web/src/store.rs 의 `encode` 형식(탭 구분)과 정확히 같아야 합니다.
--   run    : 이름 \t 성공(0|1) \t 정확도 \t 밀리초 \t 등록시각
--   battle : 이긴쪽 \t 진쪽 \t 무승부(0|1) \t 이긴셀 \t 진셀 \t 세대 \t 등록시각
--
-- dedupe 는 '질의문자열@10분버킷' — 같은 QR 을 연달아 두 번 등록하는 사고만 막고,
-- 한참 뒤에 우연히 값이 똑같은 경기가 나오는 것까지 막지는 않습니다.
create table if not exists public.entry (
  id     bigint generated always as identity primary key,
  kind   text   not null check (kind in ('run', 'battle')),
  round  bigint not null,
  dedupe text   not null unique,
  line   text   not null,
  at     bigint not null
);

create index if not exists entry_lookup on public.entry (kind, round, id desc);

alter table public.entry enable row level security;

-- 읽기만 열어 둡니다. insert/update/delete 정책은 일부러 없습니다.
drop policy if exists entry_read on public.entry;
create policy entry_read on public.entry for select using (true);

-- -------------------------------------------------------------------- 서명

-- conway-core/src/qr.rs 의 `sign_query`, web/src/store.rs 의 `signature` 와 같은 함수.
-- FNV-1a 64 해시의 아래 30비트를 36진수 6자리로. Postgres 의 bigint 는 곱셈이 넘치면
-- 오류가 나므로, 곱셈은 numeric 에서 mod 2^64 로 하고 XOR 만 bigint 로 내려서 합니다.
create or replace function private.sign_query(p_query text, p_secret text)
returns text
language plpgsql
immutable
as $$
declare
  two64  constant numeric := 18446744073709551616;
  prime  constant numeric := 1099511628211;         -- 0x100000001b3
  digits constant text    := '0123456789abcdefghijklmnopqrstuvwxyz';
  data   bytea   := convert_to(p_secret || '|' || p_query, 'UTF8');
  h      numeric := 14695981039346656037;           -- 0xcbf29ce484222325
  hb     bigint;
  n      bigint;
  i      int;
  out    text := '';
begin
  for i in 0 .. octet_length(data) - 1 loop
    hb := case when h >= 9223372036854775808 then (h - two64)::bigint else h::bigint end;
    hb := hb # get_byte(data, i)::bigint;
    h  := case when hb < 0 then hb::numeric + two64 else hb::numeric end;
    h  := (h * prime) % two64;
  end loop;

  hb := case when h >= 9223372036854775808 then (h - two64)::bigint else h::bigint end;
  n  := hb & 1073741823;                            -- 0x3fffffff
  for i in 1 .. 6 loop
    out := substr(digits, (n % 36)::int + 1, 1) || out;
    n := n / 36;
  end loop;
  return out;
end;
$$;

-- web/src/store.rs 의 `clean_name` 과 같은 규칙 (제어문자 제거 · 12자 · 빈 값이면 익명).
create or replace function private.clean_name(p_raw text)
returns text
language sql
immutable
as $$
  select case
    when cleaned = '' then '익명'
    else left(cleaned, 12)
  end
  from (select btrim(regexp_replace(coalesce(p_raw, ''), '[[:cntrl:]]', '', 'g')) as cleaned) t;
$$;

-- 질의문자열의 숫자 한 칸. 이상한 값은 0, 범위를 넘으면 잘라 냅니다.
create or replace function private.num(p_raw text, p_lo bigint, p_hi bigint)
returns bigint
language sql
immutable
as $$
  select least(greatest(coalesce(nullif(regexp_replace(coalesce(p_raw, ''), '\D', '', 'g'), '')::numeric, 0), p_lo::numeric), p_hi::numeric)::bigint;
$$;

-- ---------------------------------------------------------------------- RPC

create or replace function public.board_lines(p_kind text, p_round bigint)
returns text
language sql
stable
security definer
set search_path = public, private
as $$
  select coalesce(string_agg(line, e'\n' order by id desc), '')
  from (
    select id, line from public.entry
    where kind = p_kind and round = p_round
    order by id desc
    limit 200
  ) t;
$$;

/*
 * 결과 QR 의 질의문자열(p_q)과 그 서명(p_k)으로 기록 한 줄을 만듭니다.
 * 숫자는 전부 여기서 다시 읽으므로, 클라이언트가 보내는 것은 이름뿐입니다.
 *
 *   스피드런: r=<라운드>&c=<0|1>&a=<정확도x10000>&t=<밀리초>
 *   대전    : w=<0 무승부|1 P1|2 P2>&x=<P1 셀>&y=<P2 셀>&g=<세대>
 */
create or replace function public.submit_result(
  p_q    text,
  p_k    text,
  p_name text,
  p_foe  text default null
)
returns text
language plpgsql
security definer
set search_path = public, private
as $$
declare
  u32    constant bigint := 4294967295;
  secret text;
  parts  text[];
  q      jsonb := '{}'::jsonb;
  part   text;
  now_s  bigint := floor(extract(epoch from now()))::bigint;
  v_kind text;
  v_round bigint;
  v_line text;
  w bigint;
  x bigint;
  y bigint;
  wc bigint;
  lc bigint;
  inserted int;
begin
  select value into secret from private.config where key = 'submit_secret';
  if private.sign_query(p_q, secret) is distinct from p_k then
    raise exception 'bad signature' using errcode = '42501';
  end if;

  -- 질의문자열을 key/value 로. 값은 전부 숫자라서 URL 디코딩은 필요 없습니다.
  foreach part in array string_to_array(p_q, '&') loop
    if position('=' in part) > 0 then
      q := q || jsonb_build_object(split_part(part, '=', 1), split_part(part, '=', 2));
    end if;
  end loop;

  if q ? 'r' then
    v_kind  := 'run';
    v_round := private.num(q ->> 'r', 0, 9223372036854775807);
    v_line  := concat_ws(
      e'\t',
      private.clean_name(p_name),
      case when private.num(q ->> 'c', 0, 1) = 1 then 1 else 0 end,
      private.num(q ->> 'a', 0, 10000),
      private.num(q ->> 't', 0, u32),
      now_s
    );
  elsif q ? 'w' then
    w := private.num(q ->> 'w', 0, 3);
    if w > 2 then
      raise exception 'bad winner' using errcode = '22023';
    end if;
    x := private.num(q ->> 'x', 0, u32);
    y := private.num(q ->> 'y', 0, u32);
    if w = 2 then wc := y; lc := x; else wc := x; lc := y; end if;
    v_kind  := 'battle';
    v_round := 0;
    v_line  := concat_ws(
      e'\t',
      private.clean_name(p_name),
      private.clean_name(p_foe),
      case when w = 0 then 1 else 0 end,
      wc,
      lc,
      private.num(q ->> 'g', 0, u32),
      now_s
    );
  else
    raise exception 'bad query' using errcode = '22023';
  end if;

  insert into public.entry (kind, round, dedupe, line, at)
  values (v_kind, v_round, p_q || '@' || (now_s / 600)::text, v_line, now_s)
  on conflict (dedupe) do nothing;

  get diagnostics inserted = row_count;
  if inserted = 0 then
    raise exception 'duplicate' using errcode = '23505';
  end if;
  return 'ok';
end;
$$;

create or replace function public.wipe_board(p_kind text, p_round bigint, p_token text)
returns text
language plpgsql
security definer
set search_path = public, private
as $$
declare
  token text;
begin
  select value into token from private.config where key = 'admin_token';
  if p_token is null or p_token is distinct from token then
    raise exception 'bad admin token' using errcode = '42501';
  end if;
  delete from public.entry where kind = p_kind and round = p_round;
  return 'ok';
end;
$$;

-- ------------------------------------------------------------------ 권한

revoke all on function public.board_lines(text, bigint) from public;
revoke all on function public.submit_result(text, text, text, text) from public;
revoke all on function public.wipe_board(text, bigint, text) from public;

grant execute on function public.board_lines(text, bigint) to anon, authenticated;
grant execute on function public.submit_result(text, text, text, text) to anon, authenticated;
grant execute on function public.wipe_board(text, bigint, text) to anon, authenticated;
