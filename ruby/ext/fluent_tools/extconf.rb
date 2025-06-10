# frozen_string_literal: true

require_relative '../../installer'
require_relative '../../lib/fluent_tools/version'

# Create installer instance for gem context with version
installer = FluentToolsInstaller.new(version: FluentTools::VERSION)

# Run installation
success = installer.install!

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
  puts '❌ Installation failed'
  exit 1
end
