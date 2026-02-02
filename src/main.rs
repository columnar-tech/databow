mod cli;
mod database;
mod highlighter;
mod repl;
mod table;

fn main() {
    let config = cli::parse_args();
    let connection = database::initialize_connection(config);
    repl::run_repl(connection);
}
