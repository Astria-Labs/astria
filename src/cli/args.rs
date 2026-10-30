use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "astria")]
#[command(about = "A lightning-fast version control system", long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Initialize a new Astria repository
    Init,

    /// Record changes to the repository with AST semantic indexing
    #[command(visible_alias = "ci")]
    Commit {
        #[arg(short, long, default_value = "Auto commit from workspace scan")]
        message: String,
    },
    /// Show commit history logs
    Log,
    /// Show content of a Astria object by its hash
    Show {
        #[arg(value_name = "HASH")]
        hash: String,
    },
    /// Perform semantic AST diff between commits, workspace, or files
    Diff {
        /// Targets: can be 0, 1, or 2 arguments (commits, workspace, or file paths)
        #[arg(num_args(0..=2), value_name = "TARGET")]
        targets: Vec<String>,
    },
    /// Performs AI Semantic/Symbol search
    Query {
        /// Default search term (eg: symbol name)
        search: Option<String>,

        /// Filtrar apenas funções / métodos / subroutines
        #[arg(long, visible_alias = "fn")]
        func: bool,

        /// Filter only structs, classes or data types
        #[arg(long)]
        r#struct: bool, // 'r#struct' para escapar palavra reservada do Rust

        /// Filtrar apenas interfaces, traits ou contratos
        #[arg(long)]
        interface: bool,

        /// Filtrar apenas elementos públicos / exportados (--pub ou -p)
        #[arg(long, short = 'p')]
        pub_only: bool,

        /// Restrict the search to a specific file (e.g., src/main.rs)
        #[arg(long, short = 'f')]
        file: Option<String>,

        /// Search a specific commit instead of the current workspace (HEAD)
        #[arg(long, visible_alias = "ci")]
        commit: Option<String>,

        /// [AI Boost] Semantic search in natural language
        #[arg(long)]
        ai: Option<String>,
    },
    /// List or create branches
    #[command(visible_alias = "br")]
    Branch {
        #[arg(value_name = "BRANCH_NAME")]
        name: Option<String>,
    },
    /// Switch branch or restore context
    #[command(visible_alias = "co")]
    Checkout {
        #[arg(value_name = "BRANCH_OR_COMMIT")]
        name: String,
    },
    /// Show the current working tree status
    #[command(visible_alias = "st")]
    Status,
    /// Add file contents to the staging area
    Add {
        #[arg(required = true)]
        paths: Vec<String>,
    },
    /// Unstace changes by removing a file from the staging index
    Reset {
        #[arg(value_name = "PATH")]
        path: String,
    },
    /// Join two development histories together
    Merge {
        #[arg(value_name = "BRANCH_OR_COMMIT")]
        target: String,
    },

    /// Configure user options (eg; name and email)
    Config {
        #[arg(long)]
        global: bool,

        #[arg(num_args = 1..=2)]
        args: Vec<String>,
    },

    Worker,

    /// Pull remote changes and metadata (lazy fetch)
    Pull {
        /// Name of the remote repository (e.g., origin)
        #[arg(default_value = "origin")]
        remote: String,
    },

    /// Push local changes to a remote repository
    Push {
        #[arg(default_value = "origin")]
        remote: String,
    },

    /// Fetch changes from a remote repository without merging
    Fetch {
        #[arg(default_value = "origin")]
        remote: String,
    },

    /// Clone a repository into a new directory
    Clone {
        #[arg(long)]
        lazy: bool,
        #[arg(long)]
        ondemand: bool,
        /// URL or address of the remote repository (e.g., quic://127.0.0.1:4433)
        url: String,
        target_dir: Option<String>,
    },

    /// Start the Astria network server
    Server {
        /// Socket address to listen on
        #[arg(long, default_value = "127.0.0.1:4433")]
        addr: String,

        #[command(subcommand)]
        subcommand: Option<ServerSubCommand>,
    },

    /// Manage set of tracked repositories (remotes)
    #[command(subcommand)]
    Remote(RemoteCommands),
}

#[derive(Subcommand)]
pub enum RemoteCommands {
    Add {
        name: String,
        url: String,
    },
    SetUrl {
        name: String,
        url: String,
    },
    /// List configured remotes
    #[command(visible_alias = "ls")]
    List,
    /// Remove a tracked remote repository
    #[command(visible_alias = "rm")]
    Remove {
        name: String,
    },
}

#[derive(Subcommand, Debug)]
pub enum ServerSubCommand {
    /// Start the network server daemon
    Start,
    /// Token management for server access
    Token {
        #[command(subcommand)]
        command: TokenSubCommand,
    },
}

#[derive(Subcommand, Debug)]
pub enum TokenSubCommand {
    /// Create a new personal access token (PAT)
    Create {
        #[arg(short, long)]
        user: String,
        repo: String,
        perms: String,

        #[arg(long, default_value = "30d")]
        expires: String, // E.g "30d", "90d", "never"
    },
}
