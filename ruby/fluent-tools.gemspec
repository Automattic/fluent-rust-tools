# frozen_string_literal: true

require_relative 'lib/fluent_tools/version'
require_relative 'lib/fluent_tools/utils'

Gem::Specification.new do |spec|
  spec.name = FluentTools::Utils::BINARY_NAME
  spec.version = FluentTools::VERSION
  spec.authors = ['Automattic']
  spec.email = ['mobile@automattic.com']

  spec.summary = 'Convert between Fluent and other formats (Android XML, GNU gettext PO)'
  spec.description = "A Ruby gem that wraps a Rust CLI tool for converting between Mozilla's Fluent localization format and other formats like Android XML string resources and GNU gettext PO files"
  spec.homepage = 'https://github.com/Automattic/fluent-rust-tools'
  spec.license = 'MPL-2.0'
  spec.required_ruby_version = '>= 3.2.2'

  spec.metadata['allowed_push_host'] = 'https://rubygems.org'
  spec.metadata['homepage_uri'] = spec.homepage
  spec.metadata['source_code_uri'] = spec.homepage
  spec.metadata['changelog_uri'] = "#{spec.homepage}/blob/main/CHANGELOG.md"

  # Explicitly list the files that should be included in the gem
  spec.files = Dir[
    'lib/**/*',
    'ext/**/*',
    'exe/**/*',
    'bin/**/*',
    'installer.rb',
    '*.md',
    '*.txt',
    'LICENSE*',
    'CHANGELOG*'
  ].reject { |f| File.directory?(f) }

  spec.bindir = 'exe'
  spec.executables = ['fluent-tools']
  spec.require_paths = ['lib']

  # Runtime dependencies
  spec.add_dependency 'thor', '~> 1.0'

  # Post-install message
  spec.post_install_message = <<~MESSAGE
    This fluent-tools gem will try to use a pre-built binary for your platform.
    If unavailable, it will compile the Rust binary during installation.

    If compilation is needed, make sure you have Rust installed: https://rustup.rs/
  MESSAGE

  # Extensions for building the Rust binary
  spec.extensions = ['ext/fluent_tools/extconf.rb']
  spec.metadata['rubygems_mfa_required'] = 'true'
end
