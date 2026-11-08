fn main() {
    println!("cargo:rerun-if-env-changed=CINDY_AUTH_REGION");
    let raw = std::env::var("CINDY_AUTH_REGION").unwrap_or_default();
    let region = match raw.trim().to_lowercase().as_str() {
        "" | "global" => "global",
        "cn" => "cn",
        "dev" => "dev",
        _ => panic!("Invalid Cindy region: {raw}; expected cn, global or dev"),
    };
    println!("cargo:rustc-env=CINDY_REGION={region}");
}
