# Fluent Tools

A CLI tool and Ruby gem for converting between Mozilla's Fluent localization format and other formats, currently including Android XML string resources and GNU gettext PO files.

## Overview

This project provides a CLI for:
- **Android XML conversion**: Convert between Fluent (.ftl) files and Android XML string resources
- **PO conversion**: Convert between Fluent (.ftl) files and GNU gettext PO format

## Installation

### Rust CLI Tool

```bash
cargo build --release
./target/release/fluent-tools --help
```

### Ruby Gem

```bash
cd ruby
gem build fluent-tools.gemspec
gem install fluent-tools-*.gem
```

## Usage

### CLI Commands

#### Android XML Conversion

```bash
# Convert Fluent to Android XML
fluent-tools android to-xml -i input.ftl -o output.xml

# Convert Android XML to Fluent
fluent-tools android to-fluent -i input.xml -o output.ftl

# Convert Android XML to Fluent using original file for variable mapping
fluent-tools android to-fluent -i input.xml -o output.ftl --original-fluent original.ftl
```

#### PO Conversion

```bash
# Convert Fluent to PO format
fluent-tools po to-po -i input.ftl -o output.po -l en-US

# Convert PO to Fluent format
fluent-tools po to-fluent -i input.po -o output.ftl
```

### Ruby API

```ruby
require 'fluent_tools'

# Android XML conversion
FluentTools.fluent_to_android('input.ftl', 'output.xml')
FluentTools.android_to_fluent('input.xml', 'output.ftl')
FluentTools.android_to_fluent('input.xml', 'output.ftl', original_fluent: 'original.ftl')

# PO conversion
FluentTools.fluent_to_po('input.ftl', 'output.po', locale: 'en-US')
FluentTools.po_to_fluent('input.po', 'output.ftl')
```

## Project Structure

```
├── src/
│   ├── main.rs              # Main CLI entry point
│   ├── android/             # Android XML conversion modules
│   │   ├── mod.rs
│   │   ├── android_format.rs
│   │   └── converter.rs
│   ├── po/                  # PO conversion modules
│   │   ├── mod.rs
│   │   ├── po_format.rs
│   │   └── converter.rs
│   └── shared/              # Shared modules
│       ├── mod.rs
│       ├── fluent_parser.rs
│       └── error.rs
├── ruby/                    # Ruby gem wrapper
│   ├── lib/
│   │   ├── fluent_tools.rb
│   │   └── fluent_tools/
│   ├── exe/
│   ├── ext/
│   ├── spec/                # Test specs
│   ├── fluent-tools.gemspec
│   └── installer.rb
├── tests/
│   └── data/                # Test data files
├── Cargo.toml              # Rust project configuration
├── Cargo.lock              # Rust dependency lock file
├── LICENSE                 # License file
└── .gitignore              # Git ignore rules
```

## Features

### Android XML Support
- Convert Fluent messages to Android string resources
- Handle plurals, variables
- Preserve comments and variable mappings
- Support for bidirectional conversion

### PO Format Support
- Convert Fluent to GNU gettext PO format
- Support for plural forms
- Preserve message context and comments
- Support for bidirectional conversion

## Development

### Building
```bash
cargo build
```

### Testing
```bash
cargo test
```

### Ruby Development
```bash
cd ruby
bundle install
bundle exec rspec
```

## License

[MPL-2.0](LICENSE)
