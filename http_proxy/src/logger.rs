pub fn log_init() {
    let mut builder = env_logger::builder();

    builder.format_target(false);

    builder.parse_default_env();

    builder.try_init().unwrap();
}
