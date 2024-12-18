mod run;
mod setbest;
mod settings;
mod show;

use anyhow::Result;
use clap::{Parser, Subcommand};

fn main() {
    let args = Cli::parse();

    if let Err(error) = exec_command(args) {
        eprintln!("Error: {}", error);
        std::process::exit(1);
    }
}

fn exec_command(args: Cli) -> Result<()> {
    match args.command {
        Command::Run(args) => {
            run::run(args)?;
        }
        Command::Init => {
            settings::init()?;
        }
        Command::Best(args) => {
            setbest::set_best(args)?;
        }
        Command::Show(args) => {
            show::show(args)?;
        }
    }

    Ok(())
}

#[derive(Parser)]
struct Cli {
    #[clap(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    #[clap(name = "run", alias = "r")]
    Run(run::RunArgs),
    #[clap(name = "init", alias = "i")]
    Init,
    #[clap(name = "best", alias = "b")]
    Best(setbest::SetBestArgs),
    #[clap(name = "show", alias = "s")]
    Show(show::ShowArgs),
}
