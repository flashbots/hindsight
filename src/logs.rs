use tracing_appender::rolling::RollingFileAppender;

fn get_file_appender() -> RollingFileAppender {
    // tracing_appender::rolling::hourly("./logs", "trace.log")
    tracing_appender::rolling::minutely("./logs", "trace.log")
}

pub struct FileLogger {
    _guard: Box<tracing_appender::non_blocking::WorkerGuard>,
}

impl FileLogger {
    pub fn new() -> Self {
        let (non_blocking, guard) = tracing_appender::non_blocking(get_file_appender());
        tracing_subscriber::fmt()
            .with_writer(std::io::stderr)
            .with_writer(std::io::stdout)
            .with_writer(non_blocking)
            .init();
        Self {
            _guard: Box::new(guard),
        }
    }
}
