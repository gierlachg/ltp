use std::error::Error;
use std::fs;

use clap::{App, Arg, ArgMatches};
use log::LevelFilter;
use log4rs::append::console::ConsoleAppender;
use log4rs::config::{Appender, Config, Root};

const LOGGING_CONFIGURATION_FILE_NAME: &str = "log4rs.yml";

const ENDPOINT: &str = "127.0.0.1:10322";
const PATH: &str = "path";

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error + Send + Sync>> {
    let arguments = parse_arguments();
    let path = arguments.value_of(PATH).unwrap();
    
    init_logging();

    ltp_server::run(ENDPOINT.parse().expect("This is unexpected!"), path).await?;
    Ok(())
}

fn parse_arguments() -> ArgMatches<'static> {
    App::new(env!("CARGO_PKG_DESCRIPTION"))
        .version(env!("CARGO_PKG_VERSION"))
        .arg(
            Arg::with_name(PATH)
                .required(true)
                .long("path")
                .short("p")
                .takes_value(true)
                .help("Path to the file"),
        )
        .get_matches()
}

fn init_logging() {
    match fs::metadata(LOGGING_CONFIGURATION_FILE_NAME) {
        Ok(_) => log4rs::init_file(LOGGING_CONFIGURATION_FILE_NAME, Default::default()).unwrap(),
        Err(_) => {
            let _ = log4rs::init_config(
                Config::builder()
                    .appender(Appender::builder().build("stdout", Box::new(ConsoleAppender::builder().build())))
                    .build(Root::builder().appender("stdout").build(LevelFilter::Info))
                    .unwrap(),
            );
        }
    }
}
