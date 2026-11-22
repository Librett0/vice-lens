//! Write one texture's stored top mip (still tiled) to a file, and print its
//! format, size and tiling byte: material for working out a layout.
//!
//! `texraw <archive> <dictionary offset> <texture index> <out.bin>`

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let ar = rage::Archive::open(&a[0]).unwrap();
    let off = u64::from_str_radix(a[1].trim_start_matches("0x"), 16).unwrap();
    let i: usize = a[2].parse().unwrap();
    let r = ar.resource_at(off).unwrap();
    let texs = rage::txd::textures(&r.virt);
    let allocs = rage::txd::allocations(&texs, r.phys.len());
    let t = &texs[i];
    let fmt = rage::swizzle::format(t.dxgi).unwrap();
    let start = t.data.unwrap();
    // The whole allocation; the top mip is its last part when mips follow
    // smallest first.
    let bytes = &r.phys[start..start + allocs[i]];
    std::fs::write(&a[3], bytes).unwrap();
    println!(
        "{} {}x{} dxgi {} ({} bytes per element) mips {} mode {} alloc {} linear top {} tiled4k top {}",
        t.name,
        t.width,
        t.height,
        t.dxgi,
        fmt.bytes,
        t.mips,
        r.virt[t.at + 0x40],
        allocs[i],
        fmt.linear_size(t.width, t.height),
        rage::swizzle::tiled_size(fmt, t.width, t.height, rage::swizzle::Tile::K4)
    );
}
