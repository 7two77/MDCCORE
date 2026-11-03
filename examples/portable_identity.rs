#[path = "support/portable_identity.rs"]
mod fixture;
fn main() {
    print!("{}", fixture::run());
}
