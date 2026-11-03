fn main() {
    println!(
        "{{\"arch\":\"{}\",\"pointer_width\":{},\"endian\":\"{}\"}}",
        std::env::consts::ARCH,
        usize::BITS,
        if cfg!(target_endian = "little") {
            "little"
        } else {
            "big"
        }
    );
}
