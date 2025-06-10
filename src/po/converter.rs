use anyhow::Result;
use std::path::Path;
use std::fs;

use crate::shared::fluent_parser;
use crate::po::po_format::{write_po_file, fluent_to_po_catalog, parse_po_file, po_catalog_to_fluent};

pub fn fluent_to_po(input_path: &Path, output_path: &Path, locale: &str) -> Result<()> {
    // Read Fluent file
    let fluent_content = fs::read_to_string(input_path)?;
    
    // Parse Fluent
    let fluent_resource = fluent_parser::parse_fluent(&fluent_content)?;
    
    // Convert to PO
    let po_catalog = fluent_to_po_catalog(fluent_resource, locale)?;
    
    // Write PO file
    write_po_file(&po_catalog, output_path)?;
    
    Ok(())
}

pub fn po_to_fluent(input_path: &Path, output_path: &Path) -> Result<()> {
    // Parse PO file
    let po_catalog = parse_po_file(input_path)?;
    
    // Convert to Fluent
    let fluent_content = po_catalog_to_fluent(po_catalog)?;
    
    // Write Fluent file
    fs::write(output_path, fluent_content)?;
    
    Ok(())
}
