//! 리더보드 저장소.
//!
//! GitHub Pages 는 정적 호스팅이라 서버가 없습니다. 기록은 **리더보드를 띄운 그 기기의**
//! `localStorage` 에 쌓입니다. 부스에서는 리더보드 화면을 한 대에 띄워 두고,
//! 그 화면의 `[QR 스캔]` 으로 결과 QR을 찍는 방식을 전제로 합니다.
//!
//! 저장 형식은 줄 단위 텍스트입니다(첫 줄 = 라운드 id, 이후 한 줄에 기록 하나).
//! 스피드런은 라운드 id 가 지금과 다르면 통째로 버립니다 = 모양이 바뀌면 순위표 초기화.

use crate::config::SUBMIT_SECRET;

const RUN_KEY: &str = "clf.board.v1";
const BATTLE_KEY: &str = "clf.battle.v1";
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

/// 지금 라운드의 기록만 돌려줍니다. 라운드가 바뀌었으면 저장된 값을 지우고 빈 목록.
pub fn load_runs(round: u64) -> Vec<Run> {
    let mut out = read(RUN_KEY, round, Run::decode);
    sort_runs(&mut out);
    out
}

pub fn add_run(round: u64, entry: Run) {
    let mut entries = load_runs(round);
    entries.push(entry);
    sort_runs(&mut entries);
    entries.truncate(MAX_ENTRIES);
    write(RUN_KEY, round, entries.iter().map(Run::encode));
}

pub fn clear_runs(round: u64) {
    write(RUN_KEY, round, std::iter::empty());
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

pub fn load_matches() -> Vec<Match> {
    let mut out = read(BATTLE_KEY, BATTLE_ROUND, Match::decode);
    sort_matches(&mut out);
    out
}

pub fn add_match(entry: Match) {
    let mut entries = load_matches();
    entries.push(entry);
    sort_matches(&mut entries);
    entries.truncate(MAX_ENTRIES);
    write(BATTLE_KEY, BATTLE_ROUND, entries.iter().map(Match::encode));
}

pub fn clear_matches() {
    write(BATTLE_KEY, BATTLE_ROUND, std::iter::empty());
}

// -------------------------------------------------------------------- 공통

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
    for line in lines {
        raw.push('\n');
        raw.push_str(&line);
    }
    let _ = store.set_item(key, &raw);
}

/// 줄바꿈·탭을 지우고 길이를 자릅니다(저장 형식이 줄 단위라서).
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
