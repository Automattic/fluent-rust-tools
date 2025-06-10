#!/usr/bin/env ruby
# frozen_string_literal: true

# Installer for fluent-rust-tools gem
# Used by extconf.rb during gem installation

require 'net/http'
require 'uri'
require 'fileutils'
require 'rbconfig'
require 'json'
require_relative 'lib/fluent_tools/utils'

# Installer for fluent-rust-tools gem
# Downloads pre-built binaries or builds from source
class FluentToolsInstaller
  include FluentTools::Utils::Logger

  def initialize(version: nil)
    @version = version
  end

  # Main installation method
  def install!
    log_info "🚀 Installing #{FluentTools::Utils::BINARY_NAME}#{" `#{@version}`" if @version}..."

    platform = FluentTools::Utils.detect_platform
    if platform
      log_info "🔍 Detected platform: #{platform}"

      if download_prebuilt_binary(platform)
        log_success 'Installation complete using pre-built binary!'
        true
      else
        log_warning 'Pre-built binary not available, falling back to compilation...'
        build_from_source
      end
    else
      log_warning "Platform #{RUBY_PLATFORM} not supported for pre-built binaries"
      build_from_source
    end
  end

  private

  def download_prebuilt_binary(platform)
    log_info "🔍 Attempting to download pre-built binary for #{platform}..."

    begin
      release_data = @version ? fetch_release_by_version(@version) : fetch_latest_release
      download_url = find_download_url(release_data, platform)

      return false unless download_url

      log_info "📦 Downloading from #{download_url}"
      binary_data = download_file(download_url)

      install_binary(binary_data, platform)
      log_success 'Pre-built binary downloaded successfully!'
      true
    rescue StandardError => e
      if @version
        log_error "Failed to download pre-built binary for version #{@version}: #{e.message}"
      else
        log_error "Failed to download pre-built binary: #{e.message}"
      end
      false
    end
  end

  def fetch_latest_release
    api_url = "https://api.github.com/repos/#{FluentTools::Utils::REPO_OWNER}/#{FluentTools::Utils::REPO_NAME}/releases/latest"
    uri = URI(api_url)
    response = Net::HTTP.get_response(uri)

    raise "Could not fetch release information (HTTP #{response.code})" unless response.code == '200'

    JSON.parse(response.body)
  end

  def fetch_release_by_version(version)
    api_url = "https://api.github.com/repos/#{FluentTools::Utils::REPO_OWNER}/#{FluentTools::Utils::REPO_NAME}/releases/tags/#{version}"
    uri = URI(api_url)
    response = Net::HTTP.get_response(uri)

    case response.code
    when '200'
      JSON.parse(response.body)
    when '404'
      raise "Release #{version} not found. Available releases can be viewed at: https://github.com/#{FluentTools::Utils::REPO_OWNER}/#{FluentTools::Utils::REPO_NAME}/releases"
    else
      raise "Could not fetch release #{version} (HTTP #{response.code})"
    end
  end

  def find_download_url(release_data, platform)
    binary_name = FluentTools::Utils.binary_name_for_platform(platform)

    asset = release_data['assets'].find { |a| a['name'] == binary_name }

    unless asset
      log_error "No pre-built binary found for #{platform}"
      return nil
    end

    asset['browser_download_url']
  end

  def download_file(url)
    uri = URI(url)
    Net::HTTP.get(uri)
  end

  def install_binary(binary_data, platform)
    install_dir = determine_install_dir
    FileUtils.mkdir_p(install_dir)

    binary_extension = FluentTools::Utils.binary_extension(platform)
    binary_path = File.join(install_dir, "#{FluentTools::Utils::BINARY_NAME}#{binary_extension}")

    File.binwrite(binary_path, binary_data)

    # Make executable on Unix-like systems
    File.chmod(0o755, binary_path) unless FluentTools::Utils.windows_platform?(platform)

    @binary_path = binary_path
  end

  def build_from_source
    log_info '🔨 Building from source...'

    # Find project root (where Cargo.toml and Makefile are)
    project_root = Dir.pwd.ascend

    # Use Makefile to build the binary
    Dir.chdir(project_root) do
      unless system('make build_native')
        log_error 'Failed to build Rust binary using Makefile'
        return false
      end
    end

    # Copy binary to install location
    copy_built_binary(project_root)
    log_success 'Binary built and installed successfully!'

    true
  end

  def copy_built_binary(project_root)
    install_dir = determine_install_dir
    FileUtils.mkdir_p(install_dir)

    binary_extension = FluentTools::Utils.binary_extension
    source_binary = File.join(project_root, 'target', 'release', "#{FluentTools::Utils::BINARY_NAME}#{binary_extension}")
    dest_binary = File.join(install_dir, "#{FluentTools::Utils::BINARY_NAME}#{binary_extension}")

    raise "Built binary not found at #{source_binary}" unless File.exist?(source_binary)

    FileUtils.cp(source_binary, dest_binary)
    @binary_path = dest_binary
  end

  def determine_install_dir
    # For gem installation, put in ruby/bin relative to project root
    project_root = Dir.pwd.ascend
    if project_root
      File.join(project_root, 'ruby', 'bin')
    else
      File.join(Dir.pwd, '..', '..', 'bin') # Fallback for extconf.rb context
    end
  end
end
