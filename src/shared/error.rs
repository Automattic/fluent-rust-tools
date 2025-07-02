use thiserror::Error;

#[derive(Error, Debug)]
pub enum ConversionError {
    // Fluent parsing errors
    #[allow(dead_code)]
    #[error("Failed to parse Fluent file: {0}")]
    FluentParseError(#[from] fluent_syntax::parser::ParserError),

    #[allow(dead_code)]
    #[error("Invalid Fluent syntax: {0}")]
    InvalidFluentSyntax(String),

    #[allow(dead_code)]
    #[error("Unsupported Fluent construct: {0}")]
    UnsupportedFluentConstruct(String),

    // Generic input file parsing errors
    #[allow(dead_code)]
    #[error("Failed to parse input file: {0}")]
    InputFileParseError(String),

    #[allow(dead_code)]
    #[error("Invalid input file format: {0}")]
    InputFileInvalidFormat(String),

    #[allow(dead_code)]
    #[error("Failed to write output file: {0}")]
    OutputFileWriteError(String),

    // General errors
    #[allow(dead_code)]
    #[error("Other conversion error: {0}")]
    Other(String),

    #[allow(dead_code)]
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
}
