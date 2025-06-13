# frozen_string_literal: true

require 'open3'
require 'pathname'
require 'fileutils'

module FluentTools
  # Core converter class that interfaces with the fluent-tools Rust binary
  # Handles file validation, output directory creation, and command execution
  class Converter
    def initialize
      @binary_path = find_binary_path
    end

    # Convert Fluent file to Android XML
    def fluent_to_android(input_path, output_path)
      validate_input_file(input_path)
      ensure_output_directory(output_path)

      cmd = [@binary_path, 'android', 'to-xml', '-i', input_path, '-o', output_path]
      execute_command(cmd)
    end

    # Convert Android XML to Fluent file
    def android_to_fluent(input_path, output_path, original_fluent: nil)
      validate_input_file(input_path)
      ensure_output_directory(output_path)

      cmd = [@binary_path, 'android', 'to-fluent', '-i', input_path, '-o', output_path]
      cmd += ['--original-fluent', original_fluent] if original_fluent

      execute_command(cmd)
    end

    # Convert Fluent file to PO
    def fluent_to_po(input_path, output_path, locale: 'en-US')
      validate_input_file(input_path)
      ensure_output_directory(output_path)

      cmd = [@binary_path, 'po', 'to-po', '-i', input_path, '-o', output_path, '-l', locale]
      execute_command(cmd)
    end

    # Convert PO file to Fluent
    def po_to_fluent(input_path, output_path)
      validate_input_file(input_path)
      ensure_output_directory(output_path)

      cmd = [@binary_path, 'po', 'to-fluent', '-i', input_path, '-o', output_path]
      execute_command(cmd)
    end

    private

    def find_binary_path
      binary_name = FluentTools::Utils::BINARY_NAME

      # 1. Standard gem installation (works for all gem contexts)
      gem_binary = File.join(__dir__, '..', '..', 'bin', binary_name)
      return gem_binary if File.executable?(gem_binary)

      # 2. Development context
      if development_context?
        dev_binary = File.join(project_root, 'target', 'release', binary_name)
        return dev_binary if File.executable?(dev_binary)

        ruby_binary = File.join(project_root, 'ruby', 'bin', binary_name)
        return ruby_binary if File.executable?(ruby_binary)
      end

      # 3. System PATH
      system_binary = `which #{binary_name} 2>/dev/null`.strip
      return system_binary unless system_binary.empty?

      # If nothing found, return the expected gem path for better error messages
      gem_binary
    end

    def development_context?
      @project_root ||= find_project_root
      !@project_root.nil?
    end

    def project_root
      @project_root ||= find_project_root
    end

    def find_project_root
      Pathname.new(__dir__).ascend do |dir|
        return dir.to_s if File.exist?(File.join(dir, 'Cargo.toml'))
      end
      nil
    end

    def validate_input_file(path)
      return if File.exist?(path)

      raise Error, "Input file does not exist: #{path}"
    end

    def ensure_output_directory(path)
      output_dir = File.dirname(path)
      return if Dir.exist?(output_dir)

      begin
        FileUtils.mkdir_p(output_dir)
      rescue StandardError => e
        raise Error, "Failed to create output directory #{output_dir}: #{e.message}"
      end
    end

    def execute_command(cmd)
      unless File.executable?(@binary_path)
        raise Error, "Binary not found or not executable: #{@binary_path}. " \
                     'Make sure the gem was installed correctly and Rust is available.'
      end

      stdout, stderr, status = Open3.capture3(*cmd)

      unless status.success?
        error_message = stderr.empty? ? stdout : stderr
        raise Error, "Command failed: #{error_message}"
      end

      stdout
    end
  end
end
