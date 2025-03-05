mod cli;
mod file_organizer;
mod metadata_handler;
mod utilities;

use cli::Conf;
use file_organizer::organize_file;
use metadata_handler::change_metadata;

use clap::Parser;

fn main() {
    let conf = Conf::parse();

    let result = if conf.change {
        change_metadata(&conf)
    } else {
        organize_file(&conf)
    };

    if let Err(e) = result {
        println!("{e}");
    }
}
