pub mod add;
pub mod branch;
pub mod checkout;
pub mod clone;
pub mod commit;
pub mod config;
pub mod diff;
pub mod fetch;
pub mod init;
pub mod log;
pub mod merge;
pub mod pull;
pub mod push;
pub mod query;
pub mod remote;
pub mod reset;
pub mod server;
pub mod show;
pub mod status;
pub mod worker;

use crate::cli::args::{Commands, RemoteCommands};
use add::AddCommand;
use branch::BranchCommand;
use checkout::CheckoutCommand;
use clone::CloneCommand;
use commit::CommitCommand;
use config::ConfigCommand;
use diff::DiffCommand;
use fetch::FetchCommand;
use init::InitCommand;
use log::LogCommand;
use merge::MergeCommand;
use pull::PullCommand;
use push::PushCommand;
use query::QueryCommand;
use remote::RemoteCommand;
use reset::ResetCommand;
use server::ServerCommand;
use show::ShowCommand;
use status::StatusCommand;
use worker::WorkerCommand;

pub async fn execute(command: Commands) -> std::io::Result<()> {
    match command {
        Commands::Add { paths } => AddCommand::execute(paths),
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
            search,
            func,
            r#struct,
            interface,
            pub_only,
            file,
            commit,
            ai,
        } => {
            QueryCommand::execute(
                search, func, r#struct, interface, pub_only, file, commit, ai,
            )
            .await
        }

        // Dentro do `execute` principal:
        // Remote commands
        Commands::Remote(remote_cmd) => match remote_cmd {
            RemoteCommands::Add { name, url } => RemoteCommand::add(name, url),
            RemoteCommands::SetUrl { name, url } => RemoteCommand::set_url(name, url),
            RemoteCommands::List => RemoteCommand::list(),
            RemoteCommands::Remove { name } => RemoteCommand::remove(name),
        },

        Commands::Pull { remote } => PullCommand::execute(remote).await,
        Commands::Push { remote } => PushCommand::execute(remote).await,
        Commands::Clone {
            lazy,
            ondemand,
            url,
            target_dir,
        } => CloneCommand::execute(url, target_dir, lazy, ondemand).await,
        Commands::Fetch { remote } => FetchCommand::execute(remote).await,
        Commands::Server { addr, subcommand } => ServerCommand::execute(addr, subcommand).await,

        #[allow(unreachable_patterns)]
        _ => {
            println!("Command not implemented yet.");
            Ok(())
        }
    }
}
