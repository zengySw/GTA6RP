fn main() {
    if let Err(error) = slint_build::compile("ui/app.slint") {
        eprintln!("failed to compile launcher UI: {error}");
        std::process::exit(1);
    }
}
