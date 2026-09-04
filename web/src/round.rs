//! 라운드(= 한 시간마다 바뀌는 목표 모양) 계산.
//!
//! `challenge` 앱의 `LocalSource` 와 같은 규칙입니다:
//! `id = now_unix / interval`, `pool[splitmix64(id) % pool.len()]`.
//! 목록 순서도 `challenge` 의 `TARGET_POOL` 과 같아야 같은 모양이 나옵니다.

use crate::config::ROUND_INTERVAL_SECS;

/// (이름, 가로, 행 문자열) — `o` 가 살아있는 셀.
pub const POOL: &[(&str, &[&str])] = &[
    ("블록", &["oo", "oo"]),
    ("벌집", &[".oo.", "o..o", ".oo."]),
    ("빵", &[".oo.", "o..o", ".o.o", "..o."]),
    ("보트", &["oo.", "o.o", ".o."]),
    ("튜브", &[".o.", "o.o", ".o."]),
    ("깜빡이", &["ooo"]),
    ("두꺼비", &[".ooo", "ooo."]),
    ("비컨", &["oo..", "oo..", "..oo", "..oo"]),
    ("글라이더", &[".o.", "..o", "ooo"]),
    ("이터", &["oo..", "o.o.", "..o.", "..oo"]),
];

fn splitmix64(mut x: u64) -> u64 {
    x = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
    x = (x ^ (x >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^ (x >> 31)
}

#[derive(Clone, Copy)]
pub struct Round {
    pub id: u64,
    pub name: &'static str,
    pub shape: &'static [&'static str],
    pub ends_at: u64,
}

pub fn round_at(now_unix: u64) -> Round {
    let id = now_unix / ROUND_INTERVAL_SECS;
    let (name, shape) = POOL[(splitmix64(id) % POOL.len() as u64) as usize];
    Round {
        id,
        name,
        shape,
        ends_at: (id + 1) * ROUND_INTERVAL_SECS,
    }
}

pub fn round_by_id(id: u64) -> Round {
    round_at(id.saturating_mul(ROUND_INTERVAL_SECS))
}

pub fn now_unix() -> u64 {
    (js_sys::Date::now() / 1000.0).max(0.0) as u64
}
