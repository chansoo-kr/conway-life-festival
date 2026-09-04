/**
 * 축제 부스 리더보드 API (Cloudflare Workers + D1).
 *
 *   GET    /board?kind=run&round=123   기록을 줄 단위 텍스트로
 *   POST   /board  {q, k, name, foe}   결과 QR 의 질의문자열 + 서명으로 등록
 *   DELETE /board?kind=run&round=123   초기화 (x-admin 헤더 필요)
 *
 * 서버가 서명을 다시 계산해서 맞는 값만 받으므로, 브라우저 주소창에 숫자를 아무렇게나
 * 적어 넣은 결과는 들어오지 않습니다. 다만 서명 비밀값은 게임 바이너리 안에 들어 있으니
 * "장난을 조금 귀찮게" 만드는 수준이지 강한 보안은 아닙니다.
 */

/** conway-core/src/qr.rs 의 SUBMIT_SECRET 과 같아야 합니다. 시크릿으로 덮어쓸 수 있습니다. */
const DEFAULT_SECRET = "conway-life-festival";

/** 한 라운드에서 내려 주는 최대 줄 수. web/src/store.rs 의 MAX_ENTRIES 와 맞춥니다. */
const MAX_ENTRIES = 200;

/** 같은 QR 재등록을 막는 창(초). */
const DEDUPE_WINDOW_SECS = 600;

export default {
  async fetch(request, env) {
    const cors = corsHeaders(request, env);
    if (request.method === "OPTIONS") return new Response(null, { status: 204, headers: cors });

    const url = new URL(request.url);
    if (url.pathname !== "/board") return text("not found", 404, cors);

    try {
      if (request.method === "GET") return await list(url, env, cors);
      if (request.method === "POST") return await submit(request, env, cors);
      if (request.method === "DELETE") return await wipe(request, url, env, cors);
      return text("method not allowed", 405, cors);
    } catch (err) {
      return text(`error: ${err.message}`, 500, cors);
    }
  },
};

// ---------------------------------------------------------------- 처리기

async function list(url, env, cors) {
  const kind = kindOf(url.searchParams.get("kind"));
  if (!kind) return text("bad kind", 400, cors);

  const { results } = await env.DB.prepare(
    "select line from entry where kind = ?1 and round = ?2 order by id desc limit ?3",
  )
    .bind(kind, int(url.searchParams.get("round")), MAX_ENTRIES)
    .all();

  return text(results.map((row) => row.line).join("\n"), 200, cors);
}

async function submit(request, env, cors) {
  const body = await request.json().catch(() => null);
  if (!body || typeof body.q !== "string" || typeof body.k !== "string") {
    return text("bad request", 400, cors);
  }
  if (sign(body.q, env.SUBMIT_SECRET ?? DEFAULT_SECRET) !== body.k) {
    return text("bad signature", 403, cors);
  }

  const at = Math.floor(Date.now() / 1000);
  const record = build(body, at);
  if (!record) return text("bad request", 400, cors);

  const dedupe = `${body.q}@${Math.floor(at / DEDUPE_WINDOW_SECS)}`;
  const done = await env.DB.prepare(
    "insert or ignore into entry (kind, round, dedupe, line, at) values (?1, ?2, ?3, ?4, ?5)",
  )
    .bind(record.kind, record.round, dedupe, record.line, at)
    .run();

  if (!done.meta.changes) return text("duplicate", 409, cors);
  return text("ok", 200, cors);
}

async function wipe(request, url, env, cors) {
  if (!env.ADMIN_TOKEN || request.headers.get("x-admin") !== env.ADMIN_TOKEN) {
    return text("forbidden", 403, cors);
  }
  const kind = kindOf(url.searchParams.get("kind"));
  if (!kind) return text("bad kind", 400, cors);

  await env.DB.prepare("delete from entry where kind = ?1 and round = ?2")
    .bind(kind, int(url.searchParams.get("round")))
    .run();
  return text("ok", 200, cors);
}

// ------------------------------------------------------------ 기록 만들기

/**
 * 결과 QR 의 질의문자열을 web/src/store.rs 가 읽는 한 줄로 바꿉니다.
 * 숫자는 전부 서버가 다시 읽으므로, 클라이언트가 보내는 것은 이름뿐입니다.
 */
function build(body, at) {
  const p = new URLSearchParams(body.q);

  // 스피드런: r=<라운드>&c=<0|1>&a=<정확도x10000>&t=<밀리초>
  if (p.has("r")) {
    return {
      kind: "run",
      round: int(p.get("r")),
      line: [
        name(body.name),
        int(p.get("c")) === 1 ? 1 : 0,
        clamp(int(p.get("a")), 0, 10000),
        clamp(int(p.get("t")), 0, 4294967295),
        at,
      ].join("\t"),
    };
  }

  // 대전: w=<0 무승부|1 P1|2 P2>&x=<P1 셀>&y=<P2 셀>&g=<세대>
  if (p.has("w")) {
    const w = int(p.get("w"));
    if (w > 2) return null;
    const x = clamp(int(p.get("x")), 0, 4294967295);
    const y = clamp(int(p.get("y")), 0, 4294967295);
    const [winnerCells, loserCells] = w === 2 ? [y, x] : [x, y];
    return {
      kind: "battle",
      round: 0,
      line: [
        name(body.name),
        name(body.foe),
        w === 0 ? 1 : 0,
        winnerCells,
        loserCells,
        clamp(int(p.get("g")), 0, 4294967295),
        at,
      ].join("\t"),
    };
  }

  return null;
}

// -------------------------------------------------------------------- 도구

/** web/src/store.rs 의 `signature`, conway-core/src/qr.rs 의 `sign_query` 와 같은 함수. */
function sign(query, secret) {
  const MASK = (1n << 64n) - 1n;
  let h = 0xcbf29ce484222325n;
  for (const b of new TextEncoder().encode(`${secret}|${query}`)) {
    h ^= BigInt(b);
    h = (h * 0x100000001b3n) & MASK;
  }
  let n = h & 0x3fffffffn;
  const digits = "0123456789abcdefghijklmnopqrstuvwxyz";
  let out = "";
  for (let i = 0; i < 6; i += 1) {
    out = digits[Number(n % 36n)] + out;
    n /= 36n;
  }
  return out;
}

/** web/src/store.rs 의 `clean_name` 과 같은 규칙 (제어문자 제거 · 12자 · 빈 값이면 익명). */
function name(raw) {
  const cleaned = [...String(raw ?? "")]
    .filter((c) => !/\p{Cc}/u.test(c))
    .join("")
    .trim();
  return cleaned ? [...cleaned].slice(0, 12).join("") : "익명";
}

function kindOf(raw) {
  return raw === "run" || raw === "battle" ? raw : null;
}

function int(raw) {
  const n = Number.parseInt(raw ?? "0", 10);
  return Number.isSafeInteger(n) && n >= 0 ? n : 0;
}

function clamp(n, lo, hi) {
  return Math.min(Math.max(n, lo), hi);
}

function corsHeaders(request, env) {
  const allowed = (env.ALLOW_ORIGIN ?? "*").split(",").map((s) => s.trim());
  const origin = request.headers.get("origin") ?? "";
  const allow = allowed.includes("*") ? "*" : allowed.includes(origin) ? origin : allowed[0];
  return {
    "access-control-allow-origin": allow,
    "access-control-allow-methods": "GET,POST,DELETE,OPTIONS",
    "access-control-allow-headers": "content-type,x-admin",
    "access-control-max-age": "86400",
    vary: "origin",
  };
}

function text(body, status, cors) {
  return new Response(body, {
    status,
    headers: { ...cors, "content-type": "text/plain; charset=utf-8", "cache-control": "no-store" },
  });
}
