-- QR 하나로는 한 번만 등록되게.
--
-- 처음 판에서는 표(dedupe)가 '질의문자열@10분버킷' 이었습니다. 같은 QR 을 연달아 두 번
-- 누르는 사고만 막고, 10분이 지나면 같은 QR 이 다시 들어올 수 있었습니다.
--
-- 이제 결과 주소에 결과마다 다른 값(`n` = 결과가 나온 시각)이 들어갑니다
-- (conway-core/src/qr.rs 의 `result_url`). 그래서 질의문자열 자체가 QR 한 장을 가리키고,
-- 시간 버킷 없이 질의문자열만으로 "이미 등록된 QR" 을 가려낼 수 있습니다.
--
-- 값이 우연히 똑같은 두 경기(예: 같은 정확도로 시간 초과)도 `n` 이 달라서 서로 다른 QR 로 봅니다.

-- 이미 들어와 있는 줄들의 표에서 '@10분버킷' 을 떼어 냅니다.
-- 같은 QR 이 이미 두 번 들어와 있으면 먼저 들어온 것만 남깁니다.
delete from public.entry a
using public.entry b
where split_part(a.dedupe, '@', 1) = split_part(b.dedupe, '@', 1)
  and a.id > b.id;

update public.entry
set dedupe = split_part(dedupe, '@', 1)
where dedupe like '%@%';

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
  part   text;
  q      jsonb := '{}'::jsonb;
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

  -- 표가 곧 QR 한 장. 시간 버킷 없이 질의문자열 그대로 씁니다.
  insert into public.entry (kind, round, dedupe, line, at)
  values (v_kind, v_round, p_q, v_line, now_s)
  on conflict (dedupe) do nothing;

  get diagnostics inserted = row_count;
  if inserted = 0 then
    raise exception 'duplicate' using errcode = '23505';
  end if;
  return 'ok';
end;
$$;

revoke all on function public.submit_result(text, text, text, text) from public;
grant execute on function public.submit_result(text, text, text, text) to anon, authenticated;
