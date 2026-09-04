//! 리더보드 화면에서 참가자 결과 QR을 카메라로 읽습니다.
//!
//! 카메라 → `<canvas>` → 흑백 → `rqrr` 순으로 프레임마다 훑고,
//! 읽히는 순간 콜백에 QR 내용(주소 문자열)을 넘기고 스스로 멈춥니다.

use std::cell::RefCell;
use std::rc::Rc;

use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;
use web_sys::{HtmlCanvasElement, HtmlVideoElement, MediaStream, MediaStreamConstraints};

/// 프레임 검사 간격(ms). 너무 자주 돌리면 태블릿이 뜨거워집니다.
const TICK_MS: i32 = 160;
/// 검사용으로 줄이는 가로 크기.
const WORK_W: u32 = 480;

struct Running {
    stream: Option<MediaStream>,
    interval: Option<i32>,
    _tick: Closure<dyn FnMut()>,
}

thread_local! {
    static RUNNING: RefCell<Option<Running>> = const { RefCell::new(None) };
}

/// 이미 돌고 있으면 먼저 정리하고 다시 시작합니다.
pub fn start(video: HtmlVideoElement, on_text: Rc<dyn Fn(String)>, on_error: Rc<dyn Fn(String)>) {
    stop();
    let Some(window) = web_sys::window() else {
        return;
    };
    let media = window.navigator().media_devices();
    let Ok(media) = media else {
        on_error("이 브라우저에서는 카메라를 쓸 수 없습니다.".into());
        return;
    };

    let constraints = MediaStreamConstraints::new();
    let video_opts = js_sys::Object::new();
    let _ = js_sys::Reflect::set(
        &video_opts,
        &JsValue::from_str("facingMode"),
        &JsValue::from_str("environment"),
    );
    constraints.set_video(&video_opts);

    let Ok(promise) = media.get_user_media_with_constraints(&constraints) else {
        on_error("카메라를 여는 데 실패했습니다.".into());
        return;
    };

    wasm_bindgen_futures::spawn_local(async move {
        let stream = match JsFuture::from(promise).await {
            Ok(v) => v.unchecked_into::<MediaStream>(),
            Err(_) => {
                on_error("카메라 권한이 없습니다. 브라우저 주소창의 카메라 권한을 허용해 주세요.".into());
                return;
            }
        };
        video.set_src_object(Some(&stream));
        video.set_muted(true);
        let _ = video.set_attribute("playsinline", "");
        let _ = video.play();

        let canvas = match make_canvas() {
            Some(c) => c,
            None => return,
        };
        let tick = Closure::<dyn FnMut()>::new(move || {
            if let Some(text) = read_frame(&video, &canvas) {
                stop();
                on_text(text);
            }
        });
        let interval = web_sys::window().and_then(|w| {
            w.set_interval_with_callback_and_timeout_and_arguments_0(
                tick.as_ref().unchecked_ref(),
                TICK_MS,
            )
            .ok()
        });
        RUNNING.with(|slot| {
            *slot.borrow_mut() = Some(Running {
                stream: Some(stream),
                interval,
                _tick: tick,
            });
        });
    });
}

pub fn stop() {
    RUNNING.with(|slot| {
        let Some(running) = slot.borrow_mut().take() else {
            return;
        };
        if let (Some(window), Some(id)) = (web_sys::window(), running.interval) {
            window.clear_interval_with_handle(id);
        }
        if let Some(stream) = running.stream {
            for track in stream.get_tracks().iter() {
                if let Ok(track) = track.dyn_into::<web_sys::MediaStreamTrack>() {
                    track.stop();
                }
            }
        }
    });
}

fn make_canvas() -> Option<HtmlCanvasElement> {
    web_sys::window()?
        .document()?
        .create_element("canvas")
        .ok()?
        .dyn_into::<HtmlCanvasElement>()
        .ok()
}

fn read_frame(video: &HtmlVideoElement, canvas: &HtmlCanvasElement) -> Option<String> {
    let (vw, vh) = (video.video_width(), video.video_height());
    if vw == 0 || vh == 0 {
        return None;
    }
    let w = vw.min(WORK_W);
    let h = (vh as f64 * w as f64 / vw as f64).round().max(1.0) as u32;
    canvas.set_width(w);
    canvas.set_height(h);

    let ctx = canvas
        .get_context("2d")
        .ok()??
        .dyn_into::<web_sys::CanvasRenderingContext2d>()
        .ok()?;
    ctx.draw_image_with_html_video_element_and_dw_and_dh(video, 0.0, 0.0, w as f64, h as f64)
        .ok()?;
    let data = ctx.get_image_data(0.0, 0.0, w as f64, h as f64).ok()?;
    let rgba = data.data();

    let mut grey = vec![0u8; (w * h) as usize];
    for (i, px) in grey.iter_mut().enumerate() {
        let o = i * 4;
        let (r, g, b) = (rgba[o] as u32, rgba[o + 1] as u32, rgba[o + 2] as u32);
        *px = ((r * 299 + g * 587 + b * 114) / 1000) as u8;
    }

    let mut img = rqrr::PreparedImage::prepare_from_greyscale(w as usize, h as usize, |x, y| {
        grey[y * w as usize + x]
    });
    for grid in img.detect_grids() {
        if let Ok((_, text)) = grid.decode() {
            return Some(text);
        }
    }
    None
}
