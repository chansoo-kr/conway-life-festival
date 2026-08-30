struct Edit {
    index      : u32,
    set_mask   : u32,
    clear_mask : u32,
}

@group(0) @binding(0) var<storage, read_write> cells : array<u32>;
@group(0) @binding(1) var<storage, read>       edits : array<Edit>;

@compute @workgroup_size(64, 1, 1)
fn apply_edits(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= arrayLength(&edits)) {
        return;
    }
    let e = edits[id.x];
    cells[e.index] = (cells[e.index] & ~e.clear_mask) | e.set_mask;
}
