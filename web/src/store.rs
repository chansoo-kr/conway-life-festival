//! 리더보드 저장소.
//!
//! 기록은 [`config::API_BASE`] 의 서버(Cloudflare Workers + D1, `worker/` 참고)에 모입니다.
//! 그래서 관람객이 각자 폰으로 열어도 같은 순위표가 보입니다.
//!
//! `API_BASE` 가 비어 있으면 예전처럼 이 기기의 `localStorage` 에만 쌓습니다 —
//! 서버 없이 화면만 확인할 때 쓰는 길입니다. 서버를 쓰는 동안에도 받아 온 기록을
//! 같은 자리에 캐시해 두어서, 부스 Wi-Fi 가 끊겨도 마지막 순위표는 계속 보입니다.
//!
//! 저장 형식은 줄 단위 텍스트입니다(캐시는 첫 줄 = 라운드 id, 이후 한 줄에 기록 하나).
//! 서버가 돌려주는 본문에는 라운드 줄이 없습니다 — 질의로 이미 라운드를 지정했으니까요.
//! 이 줄 형식은 `worker/src/index.js` 의 `build` 와 정확히 같아야 합니다.

use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;

use crate::config::{API_BASE, SUBMIT_SECRET};

/// 스피드런 기록 캐시.
const RUN_KEY: &str = "clf.board.v1";
/// 대전 기록 캐시.
const BATTLE_KEY: &str = "clf.battle.v1";
/// 관리자 토큰(기록 지우기). 한 번 넣으면 그 기기에서 다시 묻지 않습니다.
const ADMIN_KEY: &str = "clf.admin";
/// 대전 기록은 라운드가 없어서 한 칸에 계속 쌓습니다.
const BATTLE_ROUND: u64 = 0;
const MAX_ENTRIES: usize = 200;

/// 결과 주소의 질의문자열(`&k=` 앞부분)에 대한 서명.
/// `conway-core/src/qr.rs` 의 `sign_query` 와 같은 함수입니다.
pub fn signature(query: &str) -> String {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in format!("{SUBMIT_SECRET}|{query}").into_bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    let mut n = h & 0x3fff_ffff;
    let digits = b"0123456789abcdefghijklmnopqrstuvwxyz";
    let mut out = [b'0'; 6];
    for slot in out.iter_mut().rev() {
        *slot = digits[(n % 36) as usize];
        n /= 36;
    }
    String::from_utf8_lossy(&out).into_owned()
}

// ------------------------------------------------------------------ 스피드런

#[derive(Clone)]
pub struct Run {
    pub name: String,
    pub cleared: bool,
    /// 0..=10000 (= 0.00% ~ 100.00%)
    pub accuracy: u32,
    /// 성공이면 걸린 시간, 실패면 제한 시간 (밀리초)
    pub millis: u32,
    pub at: u64,
}

impl Run {
    pub fn accuracy_pct(&self) -> f32 {
        self.accuracy as f32 / 100.0
    }
    pub fn seconds(&self) -> f32 {
        self.millis as f32 / 1000.0
    }

    fn encode(&self) -> String {
        format!(
            "{}\t{}\t{}\t{}\t{}",
            clean_name(&self.name),
            u8::from(self.cleared),
            self.accuracy,
            self.millis,
            self.at
        )
    }

    fn decode(line: &str) -> Option<Self> {
        let mut f = line.split('\t');
        Some(Self {
            name: f.next()?.to_string(),
            cleared: f.next()? == "1",
            accuracy: f.next()?.parse().ok()?,
            millis: f.next()?.parse().ok()?,
            at: f.next()?.parse().ok()?,
        })
    }
}

/// 성공한 기록이 먼저(빠른 순), 그 다음 미성공 기록(정확도 높은 순).
fn sort_runs(entries: &mut [Run]) {
    entries.sort_by(|a, b| {
        b.cleared
            .cmp(&a.cleared)
            .then_with(|| {
                if a.cleared {
                    a.millis.cmp(&b.millis)
                } else {
                    b.accuracy.cmp(&a.accuracy)
                }
            })
            .then_with(|| a.at.cmp(&b.at))
    });
}

/// 지금 라운드의 기록. 서버가 안 되면 이 기기의 캐시로 떨어집니다.
pub async fn load_runs(round: u64) -> Vec<Run> {
    let mut out = load("run", RUN_KEY, round, Run::decode).await;
    sort_runs(&mut out);
    out
}

/// 결과 QR 의 질의문자열(`signed`)과 서명(`k`)을 그대로 서버에 넘깁니다.
/// 숫자는 서버가 다시 읽으므로 여기서 보내는 건 이름뿐입니다.
pub async fn submit_run(round: u64, signed: &str, k: &str, entry: &Run) -> Result<(), String> {
    let name = clean_name(&entry.name);
    let sent = post(&[("q", signed), ("k", k), ("name", &name)]).await;
    finish(sent, RUN_KEY, round, || {
        let mut entries = read(RUN_KEY, round, Run::decode);
        entries.push(entry.clone());
        sort_runs(&mut entries);
        entries
            .iter()
            .take(MAX_ENTRIES)
            .map(Run::encode)
            .collect::<Vec<_>>()
    })
}

pub async fn clear_runs(round: u64) -> Result<(), String> {
    wipe("run", RUN_KEY, round).await
}

// -------------------------------------------------------------------- 대전

#[derive(Clone)]
pub struct Match {
    /// 무승부면 두 사람 중 왼쪽(플레이어 1).
    pub winner: String,
    pub loser: String,
    pub draw: bool,
    pub winner_cells: u32,
    pub loser_cells: u32,
    pub generations: u32,
    pub at: u64,
}

impl Match {
    fn encode(&self) -> String {
        format!(
            "{}\t{}\t{}\t{}\t{}\t{}\t{}",
            clean_name(&self.winner),
            clean_name(&self.loser),
            u8::from(self.draw),
            self.winner_cells,
            self.loser_cells,
            self.generations,
            self.at
        )
    }

    fn decode(line: &str) -> Option<Self> {
        let mut f = line.split('\t');
        Some(Self {
            winner: f.next()?.to_string(),
            loser: f.next()?.to_string(),
            draw: f.next()? == "1",
            winner_cells: f.next()?.parse().ok()?,
            loser_cells: f.next()?.parse().ok()?,
            generations: f.next()?.parse().ok()?,
            at: f.next()?.parse().ok()?,
        })
    }
}

/// 이긴 쪽이 남긴 셀이 많은 경기가 위로. 무승부는 뒤로.
fn sort_matches(entries: &mut [Match]) {
    entries.sort_by(|a, b| {
        a.draw
            .cmp(&b.draw)
            .then_with(|| b.winner_cells.cmp(&a.winner_cells))
            .then_with(|| a.at.cmp(&b.at))
    });
}

pub async fn load_matches() -> Vec<Match> {
    let mut out = load("battle", BATTLE_KEY, BATTLE_ROUND, Match::decode).await;
    sort_matches(&mut out);
    out
}

pub async fn submit_match(signed: &str, k: &str, entry: &Match) -> Result<(), String> {
    let winner = clean_name(&entry.winner);
    let loser = clean_name(&entry.loser);
    let sent = post(&[("q", signed), ("k", k), ("name", &winner), ("foe", &loser)]).await;
    finish(sent, BATTLE_KEY, BATTLE_ROUND, || {
        let mut entries = read(BATTLE_KEY, BATTLE_ROUND, Match::decode);
        entries.push(entry.clone());
        sort_matches(&mut entries);
        entries
            .iter()
            .take(MAX_ENTRIES)
            .map(Match::encode)
            .collect::<Vec<_>>()
    })
}

pub async fn clear_matches() -> Result<(), String> {
    wipe("battle", BATTLE_KEY, BATTLE_ROUND).await
}

// -------------------------------------------------------------- 서버 주고받기

/// 서버를 쓰는 중인지. `API_BASE` 가 비어 있으면 이 기기에만 쌓는 예전 방식입니다.
pub fn online() -> bool {
    !API_BASE.is_empty()
}

async fn load<T>(kind: &str, key: &str, round: u64, decode: fn(&str) -> Option<T>) -> Vec<T> {
    if online() {
        let url = format!("{API_BASE}/board?kind={kind}&round={round}");
        if let Ok(body) = request("GET", &url, None).await {
            let lines: Vec<&str> = body.lines().filter(|l| !l.trim().is_empty()).collect();
            write(key, round, lines.iter().map(|l| l.to_string()));
            return lines.into_iter().filter_map(decode).collect();
        }
    }
    read(key, round, decode)
}

async fn post(fields: &[(&str, &str)]) -> Option<Result<(), String>> {
    if !online() {
        return None;
    }
    let url = format!("{API_BASE}/board");
    match request("POST", &url, Some(json(fields))).await {
        Ok(_) => Some(Ok(())),
        Err(err) => Some(Err(err)),
    }
}

/// 서버에 올라갔으면 그대로 끝. 서버를 안 쓰거나 못 붙었으면 이 기기에라도 남깁니다.
fn finish(
    sent: Option<Result<(), String>>,
    key: &str,
    round: u64,
    local: impl FnOnce() -> Vec<String>,
) -> Result<(), String> {
    match sent {
        Some(Ok(())) => Ok(()),
        Some(Err(err)) => {
            write(key, round, local().into_iter());
            Err(format!("{err} 이 기기에만 저장했습니다."))
        }
        None => {
            write(key, round, local().into_iter());
            Ok(())
        }
    }
}

async fn wipe(kind: &str, key: &str, round: u64) -> Result<(), String> {
    if !online() {
        write(key, round, std::iter::empty());
        return Ok(());
    }
    let Some(token) = admin_token() else {
        return Err("관리자 토큰이 필요합니다.".into());
    };
    let url = format!("{API_BASE}/board?kind={kind}&round={round}");
    request_with_admin("DELETE", &url, None, Some(&token)).await?;
    write(key, round, std::iter::empty());
    Ok(())
}

async fn request(method: &str, url: &str, body: Option<String>) -> Result<String, String> {
    request_with_admin(method, url, body, None).await
}

async fn request_with_admin(
    method: &str,
    url: &str,
    body: Option<String>,
    admin: Option<&str>,
) -> Result<String, String> {
    let window = web_sys::window().ok_or("window 를 찾을 수 없습니다.")?;

    let init = web_sys::RequestInit::new();
    init.set_method(method);
    init.set_mode(web_sys::RequestMode::Cors);
    if let Some(body) = &body {
        init.set_body(&JsValue::from_str(body));
    }

    let request =
        web_sys::Request::new_with_str_and_init(url, &init).map_err(|_| "요청을 만들지 못했습니다.")?;
    if body.is_some() {
        // application/json 이면 프리플라이트가 한 번 더 갑니다. 서버는 본문만 보므로 단순 요청으로.
        let _ = request
            .headers()
            .set("content-type", "text/plain;charset=UTF-8");
    }
    if let Some(token) = admin {
        let _ = request.headers().set("x-admin", token);
    }

    let response = JsFuture::from(window.fetch_with_request(&request))
        .await
        .map_err(|_| "서버에 연결하지 못했습니다.".to_string())?;
    let response: web_sys::Response = response
        .dyn_into()
        .map_err(|_| "서버 응답을 읽지 못했습니다.".to_string())?;

    let text = JsFuture::from(
        response
            .text()
            .map_err(|_| "서버 응답을 읽지 못했습니다.".to_string())?,
    )
    .await
    .ok()
    .and_then(|v| v.as_string())
    .unwrap_or_default();

    if response.ok() {
        return Ok(text);
    }
    Err(match response.status() {
        403 => "서버가 이 결과를 받아 주지 않았습니다(서명 확인 실패).".into(),
        409 => "이미 등록된 결과입니다.".into(),
        code => format!("서버 오류({code})."),
    })
}

/// 이름에 따옴표가 들어와도 깨지지 않도록 JSON 은 브라우저에 맡깁니다.
fn json(fields: &[(&str, &str)]) -> String {
    let obj = js_sys::Object::new();
    for (key, value) in fields {
        let _ = js_sys::Reflect::set(
            &obj,
            &JsValue::from_str(key),
            &JsValue::from_str(value),
        );
    }
    js_sys::JSON::stringify(&obj)
        .ok()
        .and_then(|s| s.as_string())
        .unwrap_or_else(|| "{}".into())
}

// ------------------------------------------------------------- 관리자 토큰

pub fn admin_token() -> Option<String> {
    storage()?
        .get_item(ADMIN_KEY)
        .ok()
        .flatten()
        .filter(|t| !t.is_empty())
}

pub fn set_admin_token(token: &str) {
    let Some(store) = storage() else { return };
    if token.is_empty() {
        let _ = store.remove_item(ADMIN_KEY);
    } else {
        let _ = store.set_item(ADMIN_KEY, token);
    }
}

// ------------------------------------------------------------- 기기 안 캐시

fn storage() -> Option<web_sys::Storage> {
    web_sys::window()?.local_storage().ok().flatten()
}

fn read<T>(key: &str, round: u64, decode: fn(&str) -> Option<T>) -> Vec<T> {
    let Some(store) = storage() else {
        return Vec::new();
    };
    let Ok(Some(raw)) = store.get_item(key) else {
        return Vec::new();
    };
    let mut lines = raw.lines();
    if lines.next().and_then(|l| l.trim().parse::<u64>().ok()) != Some(round) {
        let _ = store.remove_item(key);
        return Vec::new();
    }
    lines.filter_map(decode).collect()
}

fn write(key: &str, round: u64, lines: impl Iterator<Item = String>) {
    let Some(store) = storage() else { return };
    let mut raw = round.to_string();
    for line in lines.take(MAX_ENTRIES) {
        raw.push('\n');
        raw.push_str(&line);
    }
    let _ = store.set_item(key, &raw);
}

/// 줄바꿈·탭을 지우고 길이를 자릅니다(저장 형식이 줄 단위라서).
/// `worker/src/index.js` 의 `name` 과 같은 규칙입니다.
pub fn clean_name(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .filter(|c| !c.is_control() && *c != '\t')
        .collect();
    let cleaned = cleaned.trim();
    if cleaned.is_empty() {
        return "익명".into();
    }
    cleaned.chars().take(12).collect()
}
