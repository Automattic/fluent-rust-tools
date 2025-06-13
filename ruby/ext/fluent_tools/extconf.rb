# frozen_string_literal: true

require_relative '../../installer'
require_relative '../../lib/fluent_tools/version'
require 'fileutils'

puts "Installing fluent-tools #{FluentTools::VERSION}..."

# Create installer instance for gem context with version
installer = FluentToolsInstaller.new(version: FluentTools::VERSION)

# Run installation
success = installer.install!

# Ensure the bin directory exists and is included in the gem
bin_dir = File.expand_path('../../bin', __dir__)
FileUtils.mkdir_p(bin_dir)

# Create dummy Makefile for RubyGems compatibility
makefile_content = <<~MAKEFILE
  all:
  \t@echo 'Binary already installed'

  install:
  \t@echo 'Binary already installed'

  clean:
  \t@echo 'Nothing to clean'
MAKEFILE

File.write('Makefile', makefile_content)

unless success
  puts '❌ Installation failed - binary could not be installed'
  puts 'This may still work if the binary becomes available at runtime'
  puts 'or if you build it manually using: rake build_rust'
  exit 1
end

puts '✅ Extension configuration complete'
