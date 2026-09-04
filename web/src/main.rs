//! 축제 부스 웹 (GitHub Pages · Trunk + wasm).
//!
//!   `#/`        부스 안내
//!   `#/pay`     구매 · 계좌 이체 안내
//!   `#/board`   스피드런 리더보드 (한 시간마다 목표 모양이 바뀌고, 바뀌면 순위표 초기화)
//!   `#/battle`  1 대 1 전투 리더보드
//!   `#/r?...`   스피드런 결과 QR이 가리키는 주소 → 이름 적고 등록
//!   `#/b?...`   대전 결과 QR이 가리키는 주소 → 두 사람 이름 적고 등록
//!
//! 주소는 `conway-core/src/qr.rs` 가 만들고, 마지막 `k` 값이 서명입니다.

mod config;
mod round;
mod scan;
mod store;

use std::cell::RefCell;
use std::rc::Rc;

use store::{Match, Run};
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;
use web_sys::{Document, Element, HtmlInputElement};

thread_local! {
    /// 스피드런 리더보드가 지금 그리고 있는 라운드. 여기가 바뀌면 순위표를 새로 그립니다.
    static SHOWN_ROUND: RefCell<Option<u64>> = const { RefCell::new(None) };
}

fn main() {
    render();
    on_window_event("hashchange", || {
        scan::stop();
        render();
    });
    // 1초마다: 남은 시간 갱신 + 라운드가 넘어갔는지 확인.
    let tick = Closure::<dyn FnMut()>::new(tick);
    if let Some(window) = web_sys::window() {
        let _ = window.set_interval_with_callback_and_timeout_and_arguments_0(
            tick.as_ref().unchecked_ref(),
            1000,
        );
    }
    tick.forget();
}

// ---------------------------------------------------------------- DOM 도우미

fn doc() -> Document {
    web_sys::window()
        .expect("window")
        .document()
        .expect("document")
}

fn find(id: &str) -> Option<Element> {
    doc().get_element_by_id(id)
}

fn set_text(id: &str, text: &str) {
    if let Some(el) = find(id) {
        el.set_text_content(Some(text));
    }
}

fn on_click(id: &str, f: impl FnMut() + 'static) {
    let Some(el) = find(id) else { return };
    let cb = Closure::<dyn FnMut()>::new(f);
    let _ = el.add_event_listener_with_callback("click", cb.as_ref().unchecked_ref());
    cb.forget();
}

fn on_window_event(name: &str, f: impl FnMut() + 'static) {
    let Some(window) = web_sys::window() else {
        return;
    };
    let cb = Closure::<dyn FnMut()>::new(f);
    let _ = window.add_event_listener_with_callback(name, cb.as_ref().unchecked_ref());
    cb.forget();
}

fn input_value(id: &str) -> String {
    find(id)
        .and_then(|el| el.dyn_into::<HtmlInputElement>().ok())
        .map(|el| el.value())
        .unwrap_or_default()
}

fn go(hash: &str) {
    if let Some(window) = web_sys::window() {
        let _ = window.location().set_hash(hash);
    }
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

// ------------------------------------------------------------------- 라우팅

struct Route {
    path: String,
    /// `&k=` 앞까지의 질의문자열 — 서명은 이 문자열에 대해 계산합니다.
    signed: String,
    params: Vec<(String, String)>,
}

impl Route {
    fn get(&self, key: &str) -> Option<&str> {
        self.params
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    }

    fn num(&self, key: &str) -> Result<u64, String> {
        self.get(key)
            .and_then(|v| v.parse::<u64>().ok())
            .ok_or_else(|| format!("주소에 {key} 값이 없습니다."))
    }

    /// 마지막 `k` 값이 나머지 질의문자열과 맞는지.
    fn verified(&self) -> bool {
        self.get("k") == Some(store::signature(&self.signed).as_str())
    }
}

fn route() -> Route {
    let hash = web_sys::window()
        .and_then(|w| w.location().hash().ok())
        .unwrap_or_default();
    let hash = hash.trim_start_matches('#').trim_start_matches('/');
    let (path, query) = hash.split_once('?').unwrap_or((hash, ""));
    let signed = query.split_once("&k=").map_or(query, |(body, _)| body);
    let params = query
        .split('&')
        .filter(|p| !p.is_empty())
        .map(|pair| match pair.split_once('=') {
            Some((k, v)) => (k.to_string(), decode(v)),
            None => (pair.to_string(), String::new()),
        })
        .collect();
    Route {
        path: path.trim_end_matches('/').to_string(),
        signed: signed.to_string(),
        params,
    }
}

fn decode(s: &str) -> String {
    let s = s.replace('+', " ");
    js_sys::decode_uri_component(&s)
        .ok()
        .and_then(|v| v.as_string())
        .unwrap_or(s)
}

fn render() {
    let r = route();
    SHOWN_ROUND.with(|slot| *slot.borrow_mut() = None);

    let (nav, body) = match r.path.as_str() {
        "pay" => ("pay", view_pay()),
        "board" => ("board", view_board()),
        "battle" => ("battle", view_battle()),
        "r" => ("board", view_run_result(&r)),
        "b" => ("battle", view_match_result(&r)),
        "dev" => ("home", view_dev()),
        _ => ("home", view_home()),
    };

    if let Some(view) = find("view") {
        view.set_inner_html(&body);
    }
    for (id, key) in [
        ("nav-home", "home"),
        ("nav-pay", "pay"),
        ("nav-board", "board"),
        ("nav-battle", "battle"),
    ] {
        if let Some(el) = find(id) {
            let _ = el.set_attribute("aria-current", if key == nav { "page" } else { "false" });
        }
    }

    match r.path.as_str() {
        "pay" => bind_pay(),
        "board" => bind_board(),
        "battle" => bind_battle(),
        "r" => bind_run_result(&r),
        "b" => bind_match_result(&r),
        _ => {}
    }
    tick();
}

fn tick() {
    let now = round::now_unix();
    let current = round::round_at(now);

    if find("board-list").is_some() {
        if SHOWN_ROUND.with(|r| *r.borrow() != Some(current.id)) {
            render();
            return;
        }
        set_text("board-clock", &mmss(current.ends_at.saturating_sub(now)));
    }
    if find("result-clock").is_some() {
        set_text("result-clock", &mmss(current.ends_at.saturating_sub(now)));
    }
}

fn mmss(secs: u64) -> String {
    format!("{:02}:{:02}", secs / 60, secs % 60)
}

// ---------------------------------------------------------------- 공통 조각

/// 목표 모양 미리보기.
fn shape_html(shape: &[&str]) -> String {
    let cols = shape.iter().map(|r| r.chars().count()).max().unwrap_or(1);
    let mut cells = String::new();
    for row in shape {
        let mut n = 0;
        for c in row.chars() {
            cells.push_str(if c == 'o' {
                "<i class=\"on\"></i>"
            } else {
                "<i></i>"
            });
            n += 1;
        }
        for _ in n..cols {
            cells.push_str("<i></i>");
        }
    }
    format!("<div class=\"shape\" style=\"--cols:{cols}\">{cells}</div>")
}

fn thousands(n: u32) -> String {
    let s = n.to_string();
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

/// 스캔·초기화 버튼 줄과 카메라 자리.
fn scan_bar(wipe_label: &str) -> String {
    format!(
        r##"<div class="scan">
  <button id="scan" class="btn">QR 스캔</button>
  <button id="scan-stop" class="btn ghost" hidden>중지</button>
  <button id="wipe" class="btn ghost">{}</button>
  <span id="scan-msg" class="dim"></span>
</div>
<video id="cam" hidden playsinline muted></video>"##,
        esc(wipe_label)
    )
}

fn bind_scan() {
    on_click("scan", move || {
        let Some(video) = find("cam") else { return };
        let _ = video.remove_attribute("hidden");
        let Ok(video) = video.dyn_into::<web_sys::HtmlVideoElement>() else {
            return;
        };
        if let Some(btn) = find("scan-stop") {
            let _ = btn.remove_attribute("hidden");
        }
        set_text("scan-msg", "QR을 화면 안에 맞춰 주세요.");
        let on_text = Rc::new(|text: String| {
            hide_camera();
            match text.split_once("#/") {
                Some((_, rest)) => go(&format!("/{rest}")),
                None => set_text("scan-msg", "이 QR은 부스 결과 코드가 아닙니다."),
            }
        });
        let on_error = Rc::new(|msg: String| {
            hide_camera();
            set_text("scan-msg", &msg);
        });
        scan::start(video, on_text, on_error);
    });

    on_click("scan-stop", || {
        scan::stop();
        hide_camera();
        set_text("scan-msg", "");
    });
}

fn hide_camera() {
    for id in ["cam", "scan-stop"] {
        if let Some(el) = find(id) {
            let _ = el.set_attribute("hidden", "");
        }
    }
}

/// 서명이 맞지 않을 때 보여 줄 화면.
fn view_bad_qr(msg: &str, back: &str) -> String {
    format!(
        "<h2 class=\"cmd\">register --qr</h2><p class=\"err\">{}</p>\
         <p class=\"links\"><a href=\"#/{back}\">리더보드로</a></p>",
        esc(msg)
    )
}

// ------------------------------------------------------------------- 화면들

fn view_home() -> String {
    r##"<h2 class="cmd">cat 부스안내.txt</h2>
<p>콘웨이의 생명 게임(Conway's Game of Life) 부스입니다.
살아있는 셀 몇 개만 놓아도 규칙 네 줄만으로 시계도, 컴퓨터도, 튜링 머신도 굴러갑니다.</p>
<table class="list">
<tr><td>튜토리얼</td><td class="dim">규칙과 조작을 직접 해 보며 배우기 · 3~4분</td></tr>
<tr><td>자유 모드</td><td class="dim">초대형 프리셋 구경 + 자유롭게 그리기</td></tr>
<tr><td>스피드런</td><td class="dim">진화하는 격자 위에서 목표 모양 만들기 · 60초</td></tr>
<tr><td>1 대 1 전투</td><td class="dim">셀을 배치하고 600세대 뒤 색 다수결</td></tr>
</table>

<h2 class="cmd">ls</h2>
<p class="links"><a href="#/pay">구매 · 계좌 이체 안내</a> · <a href="#/board">스피드런 리더보드</a> · <a href="#/battle">전투 리더보드</a></p>

<h2 class="cmd">cat 규칙.txt</h2>
<pre class="rule">살아있는 셀  이웃 2~3      →  산다
살아있는 셀  그 밖         →  죽는다
죽은 셀      이웃 정확히 3 →  태어난다</pre>"##
        .to_string()
}

fn view_pay() -> String {
    let holder = if config::ACCOUNT_HOLDER.is_empty() {
        "<span class=\"todo\">부스에서 안내</span>".to_string()
    } else {
        esc(config::ACCOUNT_HOLDER)
    };
    let (number, copy_btn) = if config::ACCOUNT_NUMBER.is_empty() {
        (
            "<span class=\"todo\">확정 전 · 부스에서 안내</span>".to_string(),
            String::new(),
        )
    } else {
        (
            format!("<span id=\"acct\">{}</span>", esc(config::ACCOUNT_NUMBER)),
            "<button id=\"copy\" class=\"btn\">복사</button>".to_string(),
        )
    };
    let notes: String = config::DEPOSIT_NOTES
        .iter()
        .map(|n| format!("<li>{}</li>", esc(n)))
        .collect();

    let sections: String = config::SECTIONS
        .iter()
        .map(|section| {
            let rows: String = section
                .items
                .iter()
                .map(|item| {
                    let price = match item.price {
                        Some(won) => format!("{}원", thousands(won)),
                        None => "가격 미정".into(),
                    };
                    format!(
                        "<tr><td>{}</td><td class=\"dim\">{}</td><td class=\"num\">{price}</td></tr>",
                        esc(item.name),
                        esc(item.desc)
                    )
                })
                .collect();
            let note = if section.note.is_empty() {
                String::new()
            } else {
                format!("<p class=\"dim\">{}</p>", esc(section.note))
            };
            format!(
                "<h2 class=\"cmd\">cat {}.txt</h2><table class=\"list\">{rows}</table>{note}",
                esc(section.title)
            )
        })
        .collect();

    format!(
        r##"<h2 class="cmd">cat 계좌.txt</h2>
<table class="account">
<tr><th>은행</th><td>{bank}</td></tr>
<tr><th>예금주</th><td>{holder}</td></tr>
<tr><th>계좌번호</th><td>{number} {copy_btn}<span id="copied" class="ok" hidden>복사됨</span></td></tr>
</table>
<ul class="notes">{notes}</ul>
{sections}"##,
        bank = esc(config::BANK_NAME),
    )
}

fn bind_pay() {
    on_click("copy", || {
        let Some(text) = find("acct").and_then(|el| el.text_content()) else {
            return;
        };
        if let Some(window) = web_sys::window() {
            let _ = window.navigator().clipboard().write_text(&text);
        }
        if let Some(el) = find("copied") {
            let _ = el.remove_attribute("hidden");
        }
    });
}

fn view_board() -> String {
    let now = round::now_unix();
    let r = round::round_at(now);
    SHOWN_ROUND.with(|slot| *slot.borrow_mut() = Some(r.id));
    let entries = store::load_runs(r.id);

    let rows: String = if entries.is_empty() {
        "<tr><td colspan=\"3\" class=\"dim\">아직 기록이 없습니다. 스피드런 결과 QR을 찍어 주세요.</td></tr>".to_string()
    } else {
        entries
            .iter()
            .enumerate()
            .map(|(i, e)| {
                let result = if e.cleared {
                    format!("<span class=\"ok\">성공</span> {:.2}초", e.seconds())
                } else {
                    format!("정확도 {:.0}%", e.accuracy_pct())
                };
                format!(
                    "<tr><td class=\"rank\">{}</td><td>{}</td><td class=\"num\">{result}</td></tr>",
                    i + 1,
                    esc(&e.name)
                )
            })
            .collect()
    };

    format!(
        r##"<h2 class="cmd">watch -n1 스피드런</h2>
<div class="round">
  {shape}
  <div>
    <p class="round-name">이번 목표 · {name}</p>
    <p class="dim">round #{id} · 다음 모양까지 <span id="board-clock" class="num">--:--</span></p>
    <p class="dim">모양이 바뀌면 순위표는 자동으로 비워집니다.</p>
  </div>
</div>
{scan}
<table class="list" id="board-list">{rows}</table>
<p class="dim">성공한 기록이 걸린 시간 순으로 먼저, 그 다음 미성공 기록이 정확도 순으로 놓입니다.
기록은 이 화면을 띄운 기기에만 쌓입니다 — 부스에서는 한 대에 띄워 두고 그 화면으로 QR을 찍어 주세요.</p>"##,
        shape = shape_html(r.shape),
        name = esc(r.name),
        id = r.id,
        scan = scan_bar("기록 지우기"),
    )
}

fn bind_board() {
    bind_scan();
    let round_id = round::round_at(round::now_unix()).id;
    on_click("wipe", move || {
        store::clear_runs(round_id);
        render();
    });
}

fn view_battle() -> String {
    let entries = store::load_matches();
    let rows: String = if entries.is_empty() {
        "<tr><td colspan=\"4\" class=\"dim\">아직 기록이 없습니다. 전투 결과 QR을 찍어 주세요.</td></tr>".to_string()
    } else {
        entries
            .iter()
            .enumerate()
            .map(|(i, m)| {
                let (who, score) = if m.draw {
                    (
                        format!("{} = {}", esc(&m.winner), esc(&m.loser)),
                        format!("무승부 {}셀", m.winner_cells),
                    )
                } else {
                    (
                        format!("{} <span class=\"dim\">vs {}</span>", esc(&m.winner), esc(&m.loser)),
                        format!(
                            "<span class=\"ok\">{}</span> : {}셀",
                            m.winner_cells, m.loser_cells
                        ),
                    )
                };
                format!(
                    "<tr><td class=\"rank\">{}</td><td>{who}</td><td class=\"num\">{score}</td></tr>",
                    i + 1
                )
            })
            .collect()
    };

    format!(
        r##"<h2 class="cmd">watch -n1 전투</h2>
<p class="dim">600세대를 버틴 뒤 <b>이긴 쪽이 남긴 셀이 많은 경기</b>가 위로 올라갑니다.</p>
{scan}
<table class="list" id="battle-list">{rows}</table>
<p class="dim">기록은 이 화면을 띄운 기기에만 쌓입니다 — 부스에서는 한 대에 띄워 두고 그 화면으로 QR을 찍어 주세요.</p>"##,
        scan = scan_bar("기록 지우기"),
    )
}

fn bind_battle() {
    bind_scan();
    on_click("wipe", || {
        store::clear_matches();
        render();
    });
}

// -------------------------------------------------------- 스피드런 결과 등록

/// `#/r?r=<라운드>&c=<0|1>&a=<정확도×10000>&t=<밀리초>&k=<서명>`
fn parse_run(r: &Route) -> Result<(u64, Run), String> {
    let round = r.num("r")?;
    let cleared = r.num("c")? == 1;
    let accuracy = r.num("a")?.min(10_000) as u32;
    let millis = r.num("t")?.min(u32::MAX as u64) as u32;
    if !r.verified() {
        return Err("결과 확인에 실패했습니다. 스피드런 화면의 QR을 다시 찍어 주세요.".into());
    }
    Ok((
        round,
        Run {
            name: String::new(),
            cleared,
            accuracy,
            millis,
            at: round::now_unix(),
        },
    ))
}

fn view_run_result(r: &Route) -> String {
    let (round_id, entry) = match parse_run(r) {
        Ok(v) => v,
        Err(msg) => return view_bad_qr(&msg, "board"),
    };
    let current = round::round_at(round::now_unix());
    let shape = round::round_by_id(round_id);

    let headline = if entry.cleared {
        format!(
            "<p class=\"big ok\">성공</p><p class=\"big num\">{:.2}초</p>",
            entry.seconds()
        )
    } else {
        format!(
            "<p class=\"big\">시간 초과</p><p class=\"big num\">최고 정확도 {:.0}%</p>",
            entry.accuracy_pct()
        )
    };

    if round_id != current.id {
        return format!(
            r##"<h2 class="cmd">register --qr</h2>
<div class="round">{shape}<div><p class="round-name">round #{id} · {name}</p>{headline}</div></div>
<p class="err">이 결과는 지난 라운드(#{id}) 것입니다. 목표 모양이 이미 바뀌어서 지금 순위표에는 올릴 수 없습니다.</p>
<p class="links"><a href="#/board">이번 라운드 리더보드 보기</a></p>"##,
            shape = shape_html(shape.shape),
            id = round_id,
            name = esc(shape.name),
        );
    }

    format!(
        r##"<h2 class="cmd">register --qr</h2>
<div class="round">
  {shape}
  <div>
    <p class="round-name">round #{id} · 목표 {name}</p>
    {headline}
    <p class="dim">이 모양은 <span id="result-clock" class="num">--:--</span> 뒤에 바뀌고, 그때 순위표도 비워집니다.</p>
  </div>
</div>
<div class="form">
  <div class="field"><label for="who">이름</label><input id="who" maxlength="12" placeholder="리더보드에 표시할 이름" autocomplete="off"></div>
  <button id="submit" class="btn">리더보드에 등록</button>
</div>
<p class="links"><a href="#/board">등록하지 않고 리더보드 보기</a></p>"##,
        shape = shape_html(shape.shape),
        id = round_id,
        name = esc(shape.name),
    )
}

fn bind_run_result(r: &Route) {
    let Ok((round_id, entry)) = parse_run(r) else {
        return;
    };
    on_click("submit", move || {
        let now = round::now_unix();
        if round::round_at(now).id != round_id {
            render();
            return;
        }
        let mut entry = entry.clone();
        entry.name = store::clean_name(&input_value("who"));
        entry.at = now;
        store::add_run(round_id, entry);
        go("/board");
    });
}

// ------------------------------------------------------------ 대전 결과 등록

/// `#/b?w=<0 무승부|1 P1|2 P2>&x=<P1 셀>&y=<P2 셀>&g=<세대>&k=<서명>`
///
/// 돌려주는 `Match` 는 이름이 비어 있고, 셀 수는 이미 승자/패자 순으로 맞춰져 있습니다.
fn parse_match(r: &Route) -> Result<Match, String> {
    let w = r.num("w")?;
    let p1 = r.num("x")?.min(u32::MAX as u64) as u32;
    let p2 = r.num("y")?.min(u32::MAX as u64) as u32;
    let generations = r.num("g")?.min(u32::MAX as u64) as u32;
    if w > 2 {
        return Err("주소의 승패 값이 이상합니다.".into());
    }
    if !r.verified() {
        return Err("결과 확인에 실패했습니다. 전투 화면의 QR을 다시 찍어 주세요.".into());
    }
    let (winner_cells, loser_cells) = if w == 2 { (p2, p1) } else { (p1, p2) };
    Ok(Match {
        winner: String::new(),
        loser: String::new(),
        draw: w == 0,
        winner_cells,
        loser_cells,
        generations,
        at: round::now_unix(),
    })
}

fn view_match_result(r: &Route) -> String {
    let m = match parse_match(r) {
        Ok(v) => v,
        Err(msg) => return view_bad_qr(&msg, "battle"),
    };
    let winner_side = r.get("w").unwrap_or("0");

    let (headline, first_label, second_label) = if m.draw {
        (
            format!(
                "<p class=\"big\">무승부</p><p class=\"big num\">{} : {}셀</p>",
                m.winner_cells, m.loser_cells
            ),
            "플레이어 1 (초록)",
            "플레이어 2 (주황)",
        )
    } else {
        (
            format!(
                "<p class=\"big ok\">{} 승리</p><p class=\"big num\">{} : {}셀</p>",
                if winner_side == "1" {
                    "플레이어 1 (초록)"
                } else {
                    "플레이어 2 (주황)"
                },
                m.winner_cells,
                m.loser_cells
            ),
            "이긴 사람",
            "진 사람",
        )
    };

    format!(
        r##"<h2 class="cmd">register --qr</h2>
{headline}
<p class="dim">{gens}세대까지 살아남은 셀로 판정했습니다.</p>
<div class="form">
  <div class="field"><label for="who">{first}</label><input id="who" maxlength="12" placeholder="이름" autocomplete="off"></div>
  <div class="field"><label for="foe">{second}</label><input id="foe" maxlength="12" placeholder="이름" autocomplete="off"></div>
  <button id="submit" class="btn">리더보드에 등록</button>
</div>
<p class="links"><a href="#/battle">등록하지 않고 리더보드 보기</a></p>"##,
        gens = m.generations,
        first = esc(first_label),
        second = esc(second_label),
    )
}

fn bind_match_result(r: &Route) {
    let Ok(m) = parse_match(r) else { return };
    on_click("submit", move || {
        let mut m = m.clone();
        m.winner = store::clean_name(&input_value("who"));
        m.loser = store::clean_name(&input_value("foe"));
        m.at = round::now_unix();
        store::add_match(m);
        go("/battle");
    });
}

// --------------------------------------------------------------- 확인용 화면

/// 진행 요원 확인용. 앱 없이 등록 흐름만 확인할 때 씁니다.
fn view_dev() -> String {
    let r = round::round_at(round::now_unix());
    let mut rows = String::new();
    for (label, query) in [
        ("스피드런 성공", format!("r?r={}&c=1&a=10000&t=12340", r.id)),
        ("스피드런 시간 초과", format!("r?r={}&c=0&a=6400&t=60000", r.id)),
        ("전투 P1 승", "b?w=1&x=340&y=120&g=600".to_string()),
        ("전투 무승부", "b?w=0&x=200&y=200&g=600".to_string()),
    ] {
        let query_only = query.split_once('?').map_or("", |(_, q)| q);
        let href = format!("#/{query}&k={}", store::signature(query_only));
        rows.push_str(&format!(
            "<tr><td>{label}</td><td><a href=\"{href}\">{href}</a></td></tr>"
        ));
    }
    format!(
        "<h2 class=\"cmd\">gen --sample</h2>\
         <p class=\"dim\">round #{} · 목표 {}</p>\
         <table class=\"list\">{rows}</table>",
        r.id,
        esc(r.name)
    )
}
