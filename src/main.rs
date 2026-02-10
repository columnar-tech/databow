mod cli;
mod database;
mod highlighter;
mod repl;
mod table;

use cli::{QuerySource, parse_args};
use std::io::Read;
use std::process::exit;

fn main() {
    let config = parse_args();
    let query_source = config.query_source.clone();

    match query_source {
        QuerySource::Query(sql) => {
            let mut connection = database::initialize_connection(config);
            if let Err(e) = database::execute_query(&mut connection, &sql) {
                eprintln!("{e}");
                exit(1);
            }
        }
        QuerySource::File(path) => {
            let sql = match std::fs::read_to_string(&path) {
                Ok(content) => content,
                Err(e) => {
                    eprintln!("Failed to read file {}: {e}", path.display());
                    exit(1);
                }
            };
            let mut connection = database::initialize_connection(config);
            if let Err(e) = database::execute_query(&mut connection, &sql) {
                eprintln!("{e}");
                exit(1);
            }
        }
        QuerySource::Stdin => {
            let mut sql = String::new();
            if let Err(e) = std::io::stdin().read_to_string(&mut sql) {
                eprintln!("Failed to read from stdin: {e}");
                exit(1);
            }
            let mut connection = database::initialize_connection(config);
            if let Err(e) = database::execute_query(&mut connection, &sql) {
                eprintln!("{e}");
                exit(1);
            }
        }
        QuerySource::Interactive => {
            let connection = database::initialize_connection(config);
            repl::run_repl(connection);
        }
    }
}
