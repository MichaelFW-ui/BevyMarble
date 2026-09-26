fn main() {
    if let Err(error) = bevymarble::territory::offline::run(std::env::args().skip(1)) {
        eprintln!("{error}");
        std::process::exit(2);
    }
}
