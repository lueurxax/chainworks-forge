fn main() {
    // sqlx::migrate! embeds directory contents; additions must invalidate Cargo's cache.
    println!("cargo:rerun-if-changed=migrations");
}
