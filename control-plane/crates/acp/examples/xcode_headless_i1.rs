#[path = "xcode_headless_i1/mod.rs"]
mod harness;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    harness::cli(std::env::args_os().skip(1).collect()).await
}
