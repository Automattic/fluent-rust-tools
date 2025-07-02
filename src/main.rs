use clap::{Parser, Subcommand};
use std::path::PathBuf;
use anyhow::Result;

mod android;
mod po;
mod shared;

use android::{fluent_to_android, android_to_fluent, android_to_fluent_with_original};
use po::{fluent_to_po, po_to_fluent};

#[derive(Parser)]
#[command(name = "fluent-tools")]
#[command(about = "A CLI tool to convert between Fluent and various formats (Android XML, GNU gettext PO)")]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Android XML conversion commands
    Android {
        #[command(subcommand)]
        android_command: FormatCommands,
    },
    /// PO (GNU gettext) conversion commands
    Po {
        #[command(subcommand)]
        po_command: FormatCommands,
    },
}

#[derive(Subcommand)]
enum FormatCommands {
    /// Convert from Fluent to the target format
    FromFluent {
        /// Input Fluent file path
        #[arg(short, long)]
        input: PathBuf,
        /// Output file path
        #[arg(short, long)]
        output: PathBuf,
        /// Source locale (e.g., en-US) - only used for PO format
        #[arg(short, long, default_value = "en-US")]
        locale: Option<String>,
        /// Source language Fluent file (for translations, this provides the original strings for msgid) - only used for PO format
        #[arg(long)]
        original_language_input: Option<PathBuf>,
    },
    /// Convert from the target format to Fluent
    ToFluent {
        /// Input file path
        #[arg(short, long)]
        input: PathBuf,
        /// Output Fluent file path
        #[arg(short, long)]
        output: PathBuf,
        /// Original Fluent file path (used to recover variable mappings when XML comments are stripped) - only used for Android format
        #[arg(long)]
        original_fluent: Option<PathBuf>,
    },
}

fn main() {
    let cli = Cli::parse();

    if let Err(e) = run(cli) {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}

fn run(cli: Cli) -> Result<()> {
    match cli.command {
        Commands::Android { android_command } => {
            match android_command {
                FormatCommands::FromFluent { input, output, .. } => {
                    fluent_to_android(&input, &output)?;
                    println!("Successfully converted {} to {}", input.display(), output.display());
                }
                FormatCommands::ToFluent { input, output, original_fluent } => {
                    match original_fluent {
                        Some(original_fluent_path) => {
                            android_to_fluent_with_original(&input, &output, &original_fluent_path)?;
                            println!("Successfully converted {} to {} using original Fluent file {}", 
                                     input.display(), output.display(), original_fluent_path.display());
                        }
                        None => {
                            android_to_fluent(&input, &output)?;
                            println!("Successfully converted {} to {}", input.display(), output.display());
                        }
                    }
                }
            }
        }
        Commands::Po { po_command } => {
            match po_command {
                FormatCommands::FromFluent { input, output, locale, original_language_input } => {
                    fluent_to_po(&input, &output, &locale.unwrap_or_else(|| "en-US".to_string()), original_language_input.as_deref())?;
                    println!("Successfully converted {} to {}", input.display(), output.display());
                }
                FormatCommands::ToFluent { input, output, .. } => {
                    po_to_fluent(&input, &output)?;
                    println!("Successfully converted {} to {}", input.display(), output.display());
                }
            }
        }
    }

    Ok(())
}
