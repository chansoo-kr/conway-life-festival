//! 부스 운영 정보. **가격·계좌가 바뀌면 이 파일만 고치면 됩니다.**

/// 목표 모양이 바뀌는 주기(초). 리더보드는 이 주기가 넘어가면 비워집니다.
///
/// `challenge` 앱도 같은 주기로 띄워야 같은 모양을 가리킵니다:
/// `cargo run --release -p challenge -- --interval-min 60`
pub const ROUND_INTERVAL_SECS: u64 = 60 * 60;

/// Supabase 프로젝트 주소 (끝에 `/` 없이). 리더보드는 여기 PostgREST 의 RPC 를 부릅니다.
/// 표와 함수는 `supabase/migrations/` 에 있습니다.
///
/// 비워 두면 서버 없이 **이 기기의 `localStorage`** 에만 기록이 쌓입니다(예전 방식).
/// 서버 없이 화면만 확인할 때 쓰세요.
pub const API_BASE: &str = "https://lgepcvmnqjyzdutvmnik.supabase.co";

/// Supabase publishable 키. 공개되어도 되는 값입니다 — 표는 RLS 로 읽기만 열려 있고,
/// 기록 등록은 서명을 확인하는 `submit_result` 함수를 거쳐야만 됩니다.
pub const API_KEY: &str = "sb_publishable_O22kv5GtrEQOeO3sBLmkPQ_a6NvLVPd";

/// QR 안의 결과값이 손대지 않은 값인지 확인할 때 쓰는 값.
/// `conway-core/src/qr.rs` 의 `SUBMIT_SECRET` 과 같아야 합니다.
pub const SUBMIT_SECRET: &str = "conway-life-festival";

pub const BANK_NAME: &str = "카카오뱅크";
pub const ACCOUNT_HOLDER: &str = "";
pub const ACCOUNT_NUMBER: &str = "3333266903067";

pub const DEPOSIT_NOTES: &[&str] = &[
    "입금자명은 '이름 + 휴대폰 뒷자리' 로 적어 주세요. (예: 홍길동7412)",
    "이체 확인 화면을 부스 진행 요원에게 보여 주시면 바로 전달해 드립니다.",
    "현장 재고가 없으면 입금 취소 또는 전액 환불해 드립니다.",
];

pub struct Item {
    pub name: &'static str,
    pub desc: &'static str,
    /// 확정 전이면 `None` — 화면에 '가격 미정' 으로 나옵니다.
    pub price: Option<u32>,
}

pub struct Section {
    pub title: &'static str,
    pub items: &'static [Item],
    /// 표 아래에 붙는 한 줄. 없으면 빈 문자열.
    pub note: &'static str,
}

pub const SECTIONS: &[Section] = &[
    Section {
        title: "키캡",
        items: &[
            Item { name: "1구", desc: "키캡 1개", price: Some(2000) },
            Item { name: "2구", desc: "키캡 2개", price: Some(2500) },
            Item { name: "3구", desc: "키캡 3개", price: Some(3000) },
        ],
        note: "파츠는 하나당 500원 추가입니다.",
    },
    Section {
        title: "체험",
        items: &[
            Item { name: "1대1 전투", desc: "셀을 배치하고 600세대 뒤 색 다수결", price: Some(1000) },
            Item { name: "스피드런", desc: "진화하는 격자 위에서 목표 모양 만들기 · 60초", price: Some(1500) },
        ],
        note: "튜토리얼과 자유 모드는 무료입니다.",
    },
    Section {
        title: "USB 게임팩",
        items: &[
            Item { name: "32GB", desc: "USB 하나로 부팅해서 바로 즐기기", price: Some(18000) },
            Item { name: "64GB", desc: "USB 하나로 부팅해서 바로 즐기기", price: Some(20000) },
        ],
        note: "",
    },
];
