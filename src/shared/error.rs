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
    
    // Android XML errors
    #[allow(dead_code)]
    #[error("Failed to parse XML file: {0}")]
    XmlParseError(#[from] quick_xml::Error),
    
    #[allow(dead_code)]
    #[error("Invalid Android XML format: {0}")]
    InvalidAndroidXml(String),
    
    #[allow(dead_code)]
    #[error("Invalid variable mapping in comment: {0}")]
    InvalidVariableMapping(String),
    
    // PO format errors
    #[allow(dead_code)]
    #[error("Failed to parse PO file: {0}")]
    PoParseError(String),
    
    #[allow(dead_code)]
    #[error("Failed to write PO file: {0}")]
    PoWriteError(String),
    
    #[allow(dead_code)]
    #[error("Invalid PO format: {0}")]
    InvalidPoFormat(String),
    
    // General errors
    #[allow(dead_code)]
    #[error("Conversion error: {0}")]
    ConversionError(String),
    
    #[allow(dead_code)]
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
}
