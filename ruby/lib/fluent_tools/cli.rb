# frozen_string_literal: true

require 'thor'

module FluentTools
  # CLI commands for Android XML conversion
  class AndroidCLI < Thor
    desc 'to_xml INPUT OUTPUT', 'Convert Fluent file to Android XML strings'
    long_desc <<~DESC
      Convert a Fluent localization file to Android XML string resources.

      INPUT: Path to the input Fluent (.ftl) file
      OUTPUT: Path to the output Android XML file
    DESC
    def to_xml(input, output)
      converter = Converter.new
      converter.fluent_to_android(input, output)
      puts "Successfully converted #{input} to #{output}"
    rescue Error => e
      puts "Error: #{e.message}"
      exit 1
    end

    desc 'to_fluent INPUT OUTPUT', 'Convert Android XML strings to Fluent file'
    long_desc <<~DESC
      Convert Android XML string resources to a Fluent localization file.

      INPUT: Path to the input Android XML file
      OUTPUT: Path to the output Fluent (.ftl) file
    DESC
    option :original_fluent, aliases: '-o', desc: 'Original Fluent file for variable mapping recovery'
    def to_fluent(input, output)
      converter = Converter.new
      converter.android_to_fluent(input, output, original_fluent: options[:original_fluent])

      message = "Successfully converted #{input} to #{output}"
      message += " using original Fluent file #{options[:original_fluent]}" if options[:original_fluent]
      puts message
    rescue Error => e
      puts "Error: #{e.message}"
      exit 1
    end
  end

  # CLI commands for PO (GNU gettext) conversion
  class PoCLI < Thor
    desc 'to_po INPUT OUTPUT', 'Convert Fluent file to PO format'
    long_desc <<~DESC
      Convert a Fluent localization file to GNU gettext PO format.

      INPUT: Path to the input Fluent (.ftl) file
      OUTPUT: Path to the output PO file
    DESC
    option :locale, aliases: '-l', default: 'en-US', desc: 'Source locale (e.g., en-US)'
    def to_po(input, output)
      converter = Converter.new
      converter.fluent_to_po(input, output, locale: options[:locale])
      puts "Successfully converted #{input} to #{output}"
    rescue Error => e
      puts "Error: #{e.message}"
      exit 1
    end

    desc 'to_fluent INPUT OUTPUT', 'Convert PO file to Fluent format'
    long_desc <<~DESC
      Convert a GNU gettext PO file to Fluent localization format.

      INPUT: Path to the input PO file
      OUTPUT: Path to the output Fluent (.ftl) file
    DESC
    def to_fluent(input, output)
      converter = Converter.new
      converter.po_to_fluent(input, output)
      puts "Successfully converted #{input} to #{output}"
    rescue Error => e
      puts "Error: #{e.message}"
      exit 1
    end
  end

  # Main CLI interface with subcommands for different formats
  class CLI < Thor
    # Android conversion commands
    desc 'android SUBCOMMAND', 'Android XML conversion commands'
    subcommand 'android', AndroidCLI

    # PO conversion commands
    desc 'po SUBCOMMAND', 'PO (GNU gettext) conversion commands'
    subcommand 'po', PoCLI

    desc 'version', 'Show version'
    def version
      puts "fluent-tools #{VERSION}"
    end
  end
end
