# frozen_string_literal: true

require_relative 'fluent_tools/version'
require_relative 'fluent_tools/utils'
require_relative 'fluent_tools/command_executor'
require_relative 'fluent_tools/cli'

# Ruby bindings and CLI for fluent-rust-tools
# Provides conversion between Fluent, Android XML, and PO formats
module FluentTools
  # Custom error class for fluent-tools operations
  class Error < StandardError; end

  # Convenience method for converting Fluent to Android XML
  def self.fluent_to_android(input_path, output_path)
    CommandExecutor.new.fluent_to_android(input_path, output_path)
  end

  # Convenience method for converting Android XML to Fluent
  def self.android_to_fluent(input_path, output_path, original_fluent: nil)
    CommandExecutor.new.android_to_fluent(input_path, output_path, original_fluent: original_fluent)
  end

  # Convenience method for converting Fluent to PO
  def self.fluent_to_po(input_path, output_path, locale: 'en-US')
    CommandExecutor.new.fluent_to_po(input_path, output_path, locale: locale)
  end

  # Convenience method for converting PO to Fluent
  def self.po_to_fluent(input_path, output_path)
    CommandExecutor.new.po_to_fluent(input_path, output_path)
  end
end
