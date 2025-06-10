# frozen_string_literal: true

require 'rbconfig'

module FluentTools
  # Shared utilities for fluent-tools
  module Utils
    # Project constants
    REPO_OWNER = 'Automattic'
    BINARY_NAME = 'fluent-tools'
    REPO_NAME = 'fluent-rust-tools'

    # Platform detection using RbConfig to identify the current system
    # Returns a string in the format "{architecture}-{os}" (e.g., "arm64-darwin")
    # Returns nil if the platform is not supported
    def self.detect_platform
      host_os = RbConfig::CONFIG['host_os']
      host_cpu = RbConfig::CONFIG['host_cpu']

      platform = case host_os
                 when /linux/
                   'linux'
                 when /darwin/
                   'darwin'
                 when /mingw|mswin/
                   'windows'
                 else
                   return nil
                 end

      architecture = case host_cpu
                     when /x86_64|amd64/
                       'x86_64'
                     when /arm64|aarch64/
                       'arm64'
                     else
                       return nil
                     end

      "#{architecture}-#{platform}"
    end

    # Generate the expected binary name for a given platform
    # Adds .exe extension for Windows platforms
    def self.binary_name_for_platform(platform)
      if platform.include?('windows')
        "#{BINARY_NAME}-#{platform}.exe"
      else
        "#{BINARY_NAME}-#{platform}"
      end
    end

    # Check if the current platform is Windows
    def self.windows_platform?(platform = nil)
      platform ||= detect_platform
      platform&.include?('windows') || false
    end

    # Get binary extension for current platform
    def self.binary_extension(platform = nil)
      windows_platform?(platform) ? '.exe' : ''
    end

    # Logger mixin to provide consistent logging across tools
    module Logger
      def log_info(message, verbose: true)
        puts message if verbose
      end

      def log_success(message, verbose: true)
        puts "✅ #{message}" if verbose
      end

      def log_warning(message, verbose: true)
        puts "⚠️ #{message}" if verbose
      end

      def log_error(message, verbose: true)
        warn "❌ #{message}" if verbose
      end
    end
  end
end
