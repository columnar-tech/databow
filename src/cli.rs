use clap::{Arg, ArgAction, Command};
use std::process::exit;

pub struct DatabaseConfig {
    pub driver_name: String,
    pub uri: Option<String>,
    pub username: Option<String>,
    pub password: Option<String>,
    pub options: Vec<(String, String)>,
}

pub fn parse_args() -> DatabaseConfig {
    let arguments = [
        Arg::new("driver")
            .long("driver")
            .help("Driver name")
            .required(true),
        Arg::new("uri")
            .long("uri")
            .help("Database uniform resource identifier"),
        Arg::new("username")
            .long("username")
            .help("Database user username"),
        Arg::new("password")
            .long("password")
            .help("Database user password"),
        Arg::new("option")
            .long("option")
            .help("Driver-specific database option")
            .action(ArgAction::Append),
    ];
    let command = Command::new("adbcli")
        .version(env!("CARGO_PKG_VERSION"))
        .about("Query databases via ADBC")
        .arg_required_else_help(true)
        .args(arguments);
    let matches = command.get_matches();

    let driver_name = match matches.get_one::<String>("driver") {
        Some(name) => name.clone(),
        None => {
            eprintln!("Driver name is required");
            exit(1);
        }
    };

    let uri = matches.get_one::<String>("uri").cloned();
    let username = matches.get_one::<String>("username").cloned();
    let password = matches.get_one::<String>("password").cloned();

    let mut options = Vec::new();
    if let Some(option_values) = matches.get_many::<String>("option") {
        for option in option_values {
            let parts: Vec<&str> = option.splitn(2, '=').collect();
            if parts.len() != 2 {
                eprintln!("Invalid option format (expected key=value): {option}");
                exit(1);
            }
            let (key, value) = (parts[0].to_string(), parts[1].to_string());
            options.push((key, value));
        }
    }

    DatabaseConfig {
        driver_name,
        uri,
        username,
        password,
        options,
    }
}
