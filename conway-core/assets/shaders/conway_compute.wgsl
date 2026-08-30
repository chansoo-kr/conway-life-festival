const WORDS_X : u32 = u32(#{WORDS_X});
const GRID_H  : u32 = u32(#{GRID_H});

const WG   : u32 = 8u;
const TILE : u32 = WG + 2u;

@group(0) @binding(0) var<storage, read>       src : array<u32>;
@group(0) @binding(1) var<storage, read_write> dst : array<u32>;

fn wrap_word_x(x: i32) -> u32 {
    var v = x;
    if (v < 0)             { v = v + i32(WORDS_X); }
    if (v >= i32(WORDS_X)) { v = v - i32(WORDS_X); }
    return u32(v);
}

fn wrap_y(y: i32) -> u32 {
    var v = y;
    if (v < 0)            { v = v + i32(GRID_H); }
    if (v >= i32(GRID_H)) { v = v - i32(GRID_H); }
    return u32(v);
}

fn bs_add(
    m: u32,
    ones:   ptr<function, u32>,
    twos:   ptr<function, u32>,
    fours:  ptr<function, u32>,
    eights: ptr<function, u32>,
) {
    let c1 = *ones  & m;   *ones   = *ones   ^ m;
    let c2 = *twos  & c1;  *twos   = *twos   ^ c1;
    let c4 = *fours & c2;  *fours  = *fours  ^ c2;
    *eights = *eights | c4;
}

var<workgroup> tile: array<u32, 100>;

@compute @workgroup_size(8, 8, 1)
fn update(
    @builtin(workgroup_id)           wg_id : vec3<u32>,
    @builtin(local_invocation_id)    lid   : vec3<u32>,
    @builtin(local_invocation_index) li    : u32,
) {
    let origin = vec2<i32>(wg_id.xy * WG) - vec2<i32>(1, 1);

    for (var i = li; i < TILE * TILE; i = i + WG * WG) {
        let tx = i % TILE;
        let ty = i / TILE;
        let gx = wrap_word_x(origin.x + i32(tx));
        let gy = wrap_y(origin.y + i32(ty));
        tile[i] = src[gy * WORDS_X + gx];
    }
    workgroupBarrier();

    let word_x = wg_id.x * WG + lid.x;
    let y      = wg_id.y * WG + lid.y;
    if (word_x >= WORDS_X || y >= GRID_H) {
        return;
    }

    let tx = lid.x + 1u;
    let ty = lid.y + 1u;

    let t_l = tile[(ty - 1u) * TILE + tx - 1u];
    let t_c = tile[(ty - 1u) * TILE + tx];
    let t_r = tile[(ty - 1u) * TILE + tx + 1u];
    let m_l = tile[ ty       * TILE + tx - 1u];
    let m_c = tile[ ty       * TILE + tx];
    let m_r = tile[ ty       * TILE + tx + 1u];
    let b_l = tile[(ty + 1u) * TILE + tx - 1u];
    let b_c = tile[(ty + 1u) * TILE + tx];
    let b_r = tile[(ty + 1u) * TILE + tx + 1u];

    let top_l = (t_c << 1u) | (t_l >> 31u);
    let top_r = (t_c >> 1u) | (t_r << 31u);
    let mid_l = (m_c << 1u) | (m_l >> 31u);
    let mid_r = (m_c >> 1u) | (m_r << 31u);
    let bot_l = (b_c << 1u) | (b_l >> 31u);
    let bot_r = (b_c >> 1u) | (b_r << 31u);

    var ones   = 0u;
    var twos   = 0u;
    var fours  = 0u;
    var eights = 0u;
    bs_add(top_l, &ones, &twos, &fours, &eights);
    bs_add(t_c,   &ones, &twos, &fours, &eights);
    bs_add(top_r, &ones, &twos, &fours, &eights);
    bs_add(mid_l, &ones, &twos, &fours, &eights);
    bs_add(mid_r, &ones, &twos, &fours, &eights);
    bs_add(bot_l, &ones, &twos, &fours, &eights);
    bs_add(b_c,   &ones, &twos, &fours, &eights);
    bs_add(bot_r, &ones, &twos, &fours, &eights);

    let alive = ~eights & ~fours & twos & (ones | m_c);
    dst[y * WORDS_X + word_x] = alive;
}
