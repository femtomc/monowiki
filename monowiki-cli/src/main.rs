//! # monowiki CLI
//!
//! Command-line interface for the monowiki static site generator.

mod agent;
mod cache;
mod commands;

use clap::{Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "monowiki")]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// Path to configuration file
    #[arg(long, default_value = "monowiki.yml")]
    config: PathBuf,

    /// Enable verbose logging
    #[arg(short, long)]
    verbose: bool,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Initialize a new monowiki project
    Init {
        /// Target directory (defaults to current directory)
        path: Option<PathBuf>,
    },

    /// Build the static site
    Build,

    /// Start development server with live reload
    Dev {
        /// Server port
        #[arg(long, default_value = "8000")]
        port: u16,
    },

    /// Search the site content
    Search {
        /// Search query
        query: String,

        /// Maximum results to return
        #[arg(long, default_value_t = 10)]
        limit: usize,

        /// Return JSON for machine consumption
        #[arg(long)]
        json: bool,

        /// Filter by document types (comma separated)
        #[arg(long, value_delimiter = ',')]
        types: Vec<String>,

        /// Filter by tags (comma separated)
        #[arg(long, value_delimiter = ',')]
        tags: Vec<String>,

        /// Include outgoing/backlink info when using --json
        #[arg(long)]
        with_links: bool,
    },

    /// Fetch a single note in structured form
    Note {
        /// Note slug (or alias/permalink without leading slash)
        slug: String,

        /// Output format
        #[arg(long, value_enum, default_value_t = NoteFormat::Json)]
        format: NoteFormat,

        /// Include backlinks/outgoing links
        #[arg(long)]
        with_links: bool,
    },

    /// Graph queries (neighbors, paths)
    Graph {
        #[command(subcommand)]
        command: GraphCommands,
    },

    /// Export content for embeddings/agents
    Export {
        #[command(subcommand)]
        command: ExportCommands,
    },

    /// Summarize changes since a git ref
    Changes {
        /// Git ref to diff against (e.g., HEAD~1 or origin/main)
        #[arg(long, default_value = "HEAD~1")]
        since: String,

        /// Emit JSON instead of text
        #[arg(long)]
        json: bool,

        /// Include section-level details (hashes/headings)
        #[arg(long)]
        with_sections: bool,
    },

    /// Verify vault health and emit diagnostics
    Verify {
        /// Emit JSON instead of text
        #[arg(long)]
        json: bool,
    },

    /// Manage comments/annotations
    Comment {
        #[command(subcommand)]
        command: CommentCommands,
    },

    /// Combined status for agents (changes + comments)
    Status {
        /// Git ref to diff against (e.g., HEAD~1)
        #[arg(long, default_value = "HEAD~1")]
        since: String,

        /// Comment status filter (open/resolved)
        #[arg(long)]
        comment_status: Option<String>,

        /// Include section-level details
        #[arg(long)]
        with_sections: bool,

        /// Emit JSON instead of text
        #[arg(long)]
        json: bool,
    },

    /// Stream vault change events for agents
    Watch,

    /// Set up GitHub Actions for GitHub Pages deployment
    GithubPages {
        /// GitHub repository name (e.g., "username/repo")
        #[arg(long)]
        repo: Option<String>,

        /// Force overwrite existing workflow
        #[arg(long)]
        force: bool,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    // Initialize tracing
    let subscriber = tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env().add_directive(if cli.verbose {
                tracing::Level::DEBUG.into()
            } else {
                tracing::Level::INFO.into()
            }),
        )
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;

    match cli.command {
        Commands::Init { path } => commands::init_project(path.as_deref()),
        Commands::Build => commands::build_site(&cli.config),
        Commands::Dev { port } => commands::dev_server(&cli.config, port).await,
        Commands::Search {
            query,
            limit,
            json,
            types,
            tags,
            with_links,
        } => {
            let opts = commands::SearchOptions {
                limit,
                json,
                types,
                tags,
                with_links,
            };
            commands::search_site(&cli.config, &query, opts)
        }
        Commands::Note {
            slug,
            format,
            with_links,
        } => commands::show_note(&cli.config, &slug, format, with_links),
        Commands::Graph { command } => match command {
            GraphCommands::Neighbors {
                slug,
                depth,
                direction,
                json,
            } => commands::graph_neighbors(&cli.config, &slug, depth, direction, json),
            GraphCommands::Path {
                from,
                to,
                max_depth,
                json,
            } => commands::graph_path(&cli.config, &from, &to, max_depth, json),
        },
        Commands::Export { command } => match command {
            ExportCommands::Sections {
                format,
                output,
                with_links,
                pretty,
            } => commands::export_sections(
                &cli.config,
                format,
                output.as_deref(),
                with_links,
                pretty,
            ),
        },
        Commands::Changes {
            since,
            json,
            with_sections,
        } => commands::changes(&cli.config, &since, json, with_sections),
        Commands::Verify { json } => commands::verify_site(&cli.config, json),
        Commands::Comment { command } => match command {
            CommentCommands::List { slug, status, json } => {
                commands::list_comments(&cli.config, slug.as_deref(), status.as_deref(), json)
            }
            CommentCommands::Add {
                slug,
                anchor,
                quote,
                author,
                tags,
                status,
                body,
            } => commands::add_comment(
                &cli.config,
                &slug,
                anchor.as_deref(),
                quote.as_deref(),
                author.as_deref(),
                tags,
                status.as_deref(),
                &body,
            ),
        },
        Commands::Status {
            since,
            comment_status,
            with_sections,
            json,
        } => commands::status(&cli.config, &since, comment_status, with_sections, json),
        Commands::Watch => commands::watch_changes(&cli.config).await,
        Commands::GithubPages { repo, force } => {
            commands::setup_github_pages(repo.as_deref(), force)
        }
    }
}

#[derive(Copy, Clone, ValueEnum)]
pub enum NoteFormat {
    Json,
    Markdown,
    Html,
    Frontmatter,
    Raw,
}

#[derive(Copy, Clone, ValueEnum)]
pub enum ExportFormat {
    Json,
    Jsonl,
}

#[derive(Copy, Clone, ValueEnum)]
pub enum GraphDirection {
    Outgoing,
    Incoming,
    Both,
}

#[derive(Subcommand)]
pub enum GraphCommands {
    /// Neighbor fan-out for a slug
    Neighbors {
        /// Source slug
        slug: String,

        /// Depth to traverse
        #[arg(long, default_value_t = 1)]
        depth: u8,

        /// Direction to traverse
        #[arg(long, value_enum, default_value_t = GraphDirection::Both)]
        direction: GraphDirection,

        /// Emit JSON instead of text
        #[arg(long)]
        json: bool,
    },

    /// Shortest path between two slugs (breadth-first)
    Path {
        /// Source slug
        from: String,
        /// Target slug
        to: String,

        /// Maximum depth to explore
        #[arg(long, default_value_t = 5)]
        max_depth: u8,

        /// Emit JSON instead of text
        #[arg(long)]
        json: bool,
    },
}

#[derive(Subcommand)]
pub enum CommentCommands {
    /// List comments/annotations
    List {
        /// Target slug filter
        #[arg(long)]
        slug: Option<String>,

        /// Status filter (open/resolved)
        #[arg(long)]
        status: Option<String>,

        /// Emit JSON instead of text
        #[arg(long)]
        json: bool,
    },
    /// Add a new comment file in the vault
    Add {
        /// Target note slug
        #[arg(long)]
        slug: String,

        /// Target anchor (section id or heading id)
        #[arg(long)]
        anchor: Option<String>,

        /// Optional quote to help re-anchor
        #[arg(long)]
        quote: Option<String>,

        /// Author name
        #[arg(long)]
        author: Option<String>,

        /// Tags (comma-separated)
        #[arg(long, value_delimiter = ',')]
        tags: Vec<String>,

        /// Status (open/resolved)
        #[arg(long)]
        status: Option<String>,

        /// Comment body text
        body: String,
    },
}

#[derive(Subcommand)]
pub enum ExportCommands {
    /// Export section-level chunks for embeddings/agents
    Sections {
        /// Output format
        #[arg(long, value_enum, default_value_t = ExportFormat::Jsonl)]
        format: ExportFormat,

        /// Optional output file (defaults to stdout)
        #[arg(long)]
        output: Option<PathBuf>,

        /// Include outgoing/backlinks per chunk
        #[arg(long)]
        with_links: bool,

        /// Pretty-print JSON (only for --format json)
        #[arg(long)]
        pretty: bool,
    },
}
