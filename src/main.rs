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
        android_command: AndroidCommands,
    },
    /// PO (GNU gettext) conversion commands
    Po {
        #[command(subcommand)]
        po_command: PoCommands,
    },
}

#[derive(Subcommand)]
enum AndroidCommands {
    /// Convert Fluent files to Android XML strings
    ToXml {
        /// Input Fluent file path
        #[arg(short, long)]
        input: PathBuf,
        /// Output Android XML file path
        #[arg(short, long)]
        output: PathBuf,
    },
    /// Convert Android XML strings to Fluent files
    ToFluent {
        /// Input Android XML file path
        #[arg(short, long)]
        input: PathBuf,
        /// Output Fluent file path
        #[arg(short, long)]
        output: PathBuf,
        /// Original Fluent file path (used to recover variable mappings when XML comments are stripped)
        #[arg(long)]
        original_fluent: Option<PathBuf>,
    },
}

#[derive(Subcommand)]
enum PoCommands {
    /// Convert Fluent files to PO format
    ToPo {
        /// Input Fluent file
        #[arg(short, long)]
        input: PathBuf,
        /// Output PO file
        #[arg(short, long)]
        output: PathBuf,
        /// Source locale (e.g., en-US)
        #[arg(short, long, default_value = "en-US")]
        locale: String,
    },
    /// Convert PO files to Fluent format
    ToFluent {
        /// Input PO file
        #[arg(short, long)]
        input: PathBuf,
        /// Output Fluent file
        #[arg(short, long)]
        output: PathBuf,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Android { android_command } => {
            match android_command {
                AndroidCommands::ToXml { input, output } => {
                    fluent_to_android(&input, &output)?;
                    println!("Successfully converted {} to {}", input.display(), output.display());
                }
                AndroidCommands::ToFluent { input, output, original_fluent } => {
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
                PoCommands::ToPo { input, output, locale } => {
                    fluent_to_po(&input, &output, &locale)?;
                    println!("Successfully converted {} to {}", input.display(), output.display());
                }
                PoCommands::ToFluent { input, output } => {
                    po_to_fluent(&input, &output)?;
                    println!("Successfully converted {} to {}", input.display(), output.display());
                }
            }
        }
    }

    Ok(())
}
