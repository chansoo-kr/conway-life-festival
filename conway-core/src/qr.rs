//! 결과 QR 코드.
//!
//! 챌린지·대전이 끝나면 결과 값을 주소에 담아 QR 로 띄웁니다.
//! 방문자가(또는 부스 화면이) 그 QR 을 찍으면 웹에서 이름을 적고 리더보드에 올립니다.
//!
//! 주소 형식과 서명 규칙은 [web/README.md](../../web/README.md) 와 같아야 합니다.
//! 웹 쪽 구현은 `web/src/store.rs` 의 `signature`, `web/src/main.rs` 의 `parse_*` 입니다.

use bevy::{
    asset::RenderAssetUsages,
    image::ImageSampler,
    prelude::*,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat},
};

/// 웹이 올라가 있는 주소. `CONWAY_WEB_URL` 로 덮어쓸 수 있습니다(로컬 확인용).
pub const WEB_BASE_URL: &str = "https://chansoo-kr.github.io/conway-life-festival/";

/// 결과 값이 손대지 않은 값인지 보는 용도. 웹의 `SUBMIT_SECRET` 과 같아야 합니다.
pub const SUBMIT_SECRET: &str = "conway-life-festival";

pub fn web_base_url() -> String {
    let raw = std::env::args()
        .collect::<Vec<_>>()
        .windows(2)
        .find(|w| w[0] == "--web-url")
        .map(|w| w[1].clone())
        .or_else(|| std::env::var("CONWAY_WEB_URL").ok())
        .unwrap_or_else(|| WEB_BASE_URL.to_string());
    let raw = raw.trim().to_string();
    if raw.ends_with('/') {
        raw
    } else {
        format!("{raw}/")
    }
}

/// `SUBMIT_SECRET|<질의문자열>` 의 FNV-1a 64 해시 아래 30비트를 36진수 6자리로.
pub fn sign_query(query: &str) -> String {
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

fn result_url(path: &str, query: String) -> String {
    let sig = sign_query(&query);
    format!("{}#/{path}?{query}&k={sig}", web_base_url())
}

/// 챌린지 결과 주소. `accuracy` 는 0.0~1.0, `seconds` 는 성공이면 걸린 시간·실패면 제한 시간.
pub fn challenge_url(round: u64, cleared: bool, accuracy: f32, seconds: f32) -> String {
    let acc = (accuracy.clamp(0.0, 1.0) * 10_000.0).round() as u32;
    let ms = (seconds.max(0.0) * 1000.0).round() as u32;
    result_url(
        "r",
        format!("r={round}&c={}&a={acc}&t={ms}", u8::from(cleared)),
    )
}

/// 대전 결과 주소. `winner` 는 무승부 `None`, 플레이어 1 `Some(0)`, 플레이어 2 `Some(1)`.
pub fn battle_url(winner: Option<u32>, counts: [u32; 2], generations: u64) -> String {
    let w = winner.map_or(0, |w| w + 1);
    result_url(
        "b",
        format!("w={w}&x={}&y={}&g={generations}", counts[0], counts[1]),
    )
}

/// QR 코드를 정사각형 흑백 텍스처로 굽습니다. `module_px` 는 모듈(점) 한 칸의 픽셀 수.
pub fn qr_image(data: &str, module_px: u32, quiet: u32) -> Option<Image> {
    let code = qrcode::QrCode::new(data.as_bytes()).ok()?;
    let modules = code.width() as u32;
    let colors = code.to_colors();
    let side = (modules + quiet * 2) * module_px;

    // QR 은 밝은 바탕 위 어두운 점이어야 읽힙니다(어두운 UI 위에 그대로 얹으면 안 됨).
    let mut pixels = vec![255u8; (side * side * 4) as usize];
    for y in 0..modules {
        for x in 0..modules {
            if colors[(y * modules + x) as usize] != qrcode::Color::Dark {
                continue;
            }
            for py in 0..module_px {
                for px in 0..module_px {
                    let gx = (x + quiet) * module_px + px;
                    let gy = (y + quiet) * module_px + py;
                    let o = ((gy * side + gx) * 4) as usize;
                    pixels[o] = 0;
                    pixels[o + 1] = 0;
                    pixels[o + 2] = 0;
                }
            }
        }
    }

    let mut image = Image::new(
        Extent3d {
            width: side,
            height: side,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        pixels,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    );
    image.sampler = ImageSampler::nearest();
    Some(image)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn urls_carry_a_matching_signature() {
        let url = challenge_url(485_612, true, 1.0, 12.34);
        let (body, sig) = url.split_once("&k=").expect("서명이 붙어야 함");
        let query = body.split_once("?").expect("질의문자열").1;
        assert_eq!(sign_query(query), sig);
        assert!(url.contains("a=10000") && url.contains("t=12340"));
    }

    #[test]
    fn battle_url_marks_the_winner() {
        assert!(battle_url(Some(1), [12, 30], 600).contains("w=2&x=12&y=30&g=600"));
        assert!(battle_url(None, [7, 7], 600).contains("w=0"));
    }

    #[test]
    fn qr_bakes_a_square_texture() {
        let image = qr_image("https://example.com/#/r?r=1", 4, 2).expect("QR");
        assert_eq!(image.width(), image.height());
    }
}
