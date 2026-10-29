pub mod add;
pub mod log;
pub mod init;
pub mod show;
pub mod diff;
pub mod pull;
pub mod query;
pub mod merge;
pub mod config;
pub mod reset;
pub mod branch;
pub mod status;
pub mod commit;
pub mod remote;
pub mod worker;
pub mod checkout;

use add::AddCommand;
use log::LogCommand;
use pull::PullCommand;
use init::InitCommand;
use show::ShowCommand;
use diff::DiffCommand;
use query::QueryCommand;
use reset::ResetCommand;
use merge::MergeCommand;
use config::ConfigCommand;
use branch::BranchCommand;
use status::StatusCommand;
use commit::CommitCommand;
use worker::WorkerCommand;
use remote::RemoteCommand;
use checkout::CheckoutCommand;
use crate::cli::args::{Commands, RemoteCommands};

pub async fn execute(command: Commands) -> std::io::Result<()> {
    match command {
        Commands::Add { paths }=> AddCommand::execute(paths),
        Commands::Log => LogCommand::execute(),
        Commands::Init => InitCommand::execute(),
        Commands::Worker => WorkerCommand::execute(),
        Commands::Status => StatusCommand::execute(),
        Commands::Show { hash } => ShowCommand::execute(hash),
        Commands::Branch { name } => BranchCommand::execute(name),
        Commands::Diff { targets } => DiffCommand::execute(targets),
        Commands::Checkout { name } => CheckoutCommand::execute(name),
        Commands::Commit { message } => CommitCommand::execute(message),
        Commands::Reset { path } => ResetCommand::execute(path),
        Commands::Merge { target } => MergeCommand::execute(target),
        Commands::Config { global, args } => ConfigCommand::execute(global, args),
        Commands::Query { 
            search ,func, r#struct, interface, pub_only, file, commit, ai
        } => QueryCommand::execute(search, func, r#struct, interface, pub_only, file, commit, ai).await,

        
        // Dentro do `execute` principal:
        // Remote commands
        Commands::Remote(remote_cmd) => match remote_cmd {
            RemoteCommands::Add { name, url } => RemoteCommand::add(name, url),
            RemoteCommands::SetUrl { name, url } => RemoteCommand::set_url(name, url),
            RemoteCommands::List => RemoteCommand::list(),
            RemoteCommands::Remove { name } => RemoteCommand::remove(name),
        },

        Commands::Pull { remote } => PullCommand::execute(remote),

        #[allow(unreachable_patterns)]
        _ => {
            println!("Command not implemented yet.");
            Ok(())
        }
    }
}