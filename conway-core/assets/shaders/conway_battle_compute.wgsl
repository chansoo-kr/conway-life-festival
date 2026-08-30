const WORDS_X : u32 = u32(#{WORDS_X});
const GRID_H  : u32 = u32(#{GRID_H});
const PLANE   : u32 = WORDS_X * GRID_H;

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

var<workgroup> tile_a: array<u32, 100>;
var<workgroup> tile_b: array<u32, 100>;

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
        let idx = gy * WORDS_X + gx;
        tile_a[i] = src[idx];
        tile_b[i] = src[PLANE + idx];
    }
    workgroupBarrier();

    let word_x = wg_id.x * WG + lid.x;
    let y      = wg_id.y * WG + lid.y;
    if (word_x >= WORDS_X || y >= GRID_H) {
        return;
    }

    let tx = lid.x + 1u;
    let ty = lid.y + 1u;

    let i_tl = (ty - 1u) * TILE + tx - 1u;
    let i_tc = (ty - 1u) * TILE + tx;
    let i_tr = (ty - 1u) * TILE + tx + 1u;
    let i_ml =  ty       * TILE + tx - 1u;
    let i_mc =  ty       * TILE + tx;
    let i_mr =  ty       * TILE + tx + 1u;
    let i_bl = (ty + 1u) * TILE + tx - 1u;
    let i_bc = (ty + 1u) * TILE + tx;
    let i_br = (ty + 1u) * TILE + tx + 1u;

    let a_tl = tile_a[i_tl]; let a_tc = tile_a[i_tc]; let a_tr = tile_a[i_tr];
    let a_ml = tile_a[i_ml]; let a_mc = tile_a[i_mc]; let a_mr = tile_a[i_mr];
    let a_bl = tile_a[i_bl]; let a_bc = tile_a[i_bc]; let a_br = tile_a[i_br];
    let b_tl = tile_b[i_tl]; let b_tc = tile_b[i_tc]; let b_tr = tile_b[i_tr];
    let b_ml = tile_b[i_ml]; let b_mc = tile_b[i_mc]; let b_mr = tile_b[i_mr];
    let b_bl = tile_b[i_bl]; let b_bc = tile_b[i_bc]; let b_br = tile_b[i_br];
    let c_tl = a_tl | b_tl; let c_tc = a_tc | b_tc; let c_tr = a_tr | b_tr;
    let c_ml = a_ml | b_ml; let c_mc = a_mc | b_mc; let c_mr = a_mr | b_mr;
    let c_bl = a_bl | b_bl; let c_bc = a_bc | b_bc; let c_br = a_br | b_br;

    var ones = 0u; var twos = 0u; var fours = 0u; var eights = 0u;
    bs_add((c_tc << 1u) | (c_tl >> 31u), &ones, &twos, &fours, &eights);
    bs_add(c_tc,                          &ones, &twos, &fours, &eights);
    bs_add((c_tc >> 1u) | (c_tr << 31u), &ones, &twos, &fours, &eights);
    bs_add((c_mc << 1u) | (c_ml >> 31u), &ones, &twos, &fours, &eights);
    bs_add((c_mc >> 1u) | (c_mr << 31u), &ones, &twos, &fours, &eights);
    bs_add((c_bc << 1u) | (c_bl >> 31u), &ones, &twos, &fours, &eights);
    bs_add(c_bc,                          &ones, &twos, &fours, &eights);
    bs_add((c_bc >> 1u) | (c_br << 31u), &ones, &twos, &fours, &eights);

    var a1 = 0u; var a2 = 0u; var a4 = 0u; var a8 = 0u;
    bs_add((a_tc << 1u) | (a_tl >> 31u), &a1, &a2, &a4, &a8);
    bs_add(a_tc,                          &a1, &a2, &a4, &a8);
    bs_add((a_tc >> 1u) | (a_tr << 31u), &a1, &a2, &a4, &a8);
    bs_add((a_mc << 1u) | (a_ml >> 31u), &a1, &a2, &a4, &a8);
    bs_add((a_mc >> 1u) | (a_mr << 31u), &a1, &a2, &a4, &a8);
    bs_add((a_bc << 1u) | (a_bl >> 31u), &a1, &a2, &a4, &a8);
    bs_add(a_bc,                          &a1, &a2, &a4, &a8);
    bs_add((a_bc >> 1u) | (a_br << 31u), &a1, &a2, &a4, &a8);

    let self_   = c_mc;
    let next    = ~eights & ~fours & twos & (ones | self_);
    let birth   = next & ~self_;
    let survive = next &  self_;
    let a_major = a2 | a4 | a8;

    let idx = y * WORDS_X + word_x;
    dst[idx]         = (survive & a_mc) | (birth &  a_major);
    dst[PLANE + idx] = (survive & b_mc) | (birth & ~a_major);
}
