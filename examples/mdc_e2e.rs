#[path = "support/mdc_e2e.rs"]
mod harness;
fn main() {
    let report = harness::execute();
    println!("{}", report.json());
    if !report.accepted() {
        std::process::exit(1);
    }
}
