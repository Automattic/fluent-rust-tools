#!/usr/bin/env ruby
# frozen_string_literal: true

# Installer for fluent-rust-tools gem
# Used by extconf.rb during gem installation

require 'net/http'
require 'uri'
require 'fileutils'
require 'rbconfig'
require 'json'
require 'pathname'
require_relative 'lib/fluent_tools/utils'

# Installer for fluent-rust-tools gem
# Downloads pre-built binaries
class FluentToolsInstaller
  include FluentTools::Utils::Logger

  def initialize(version: nil)
    @version = version
  end

  # Main installation method
  # rubocop:disable Naming/PredicateMethod
  def install!
    log_info "🚀 Installing #{FluentTools::Utils::BINARY_NAME}#{" `#{@version}`" if @version}..."

    platform = FluentTools::Utils.detect_platform
    if platform
      log_info "🔍 Detected platform: #{platform}"

      if download_prebuilt_binary(platform)
        log_success 'Installation complete using pre-built binary!'
        true
      else
        log_error 'Pre-built binary not available for this platform/version'
        log_error "Please check available releases at: https://github.com/#{FluentTools::Utils::REPO_OWNER}/#{FluentTools::Utils::REPO_NAME}/releases"
        false
      end
    else
      log_error "Platform #{RUBY_PLATFORM} not supported"
      log_error "Please check available releases at: https://github.com/#{FluentTools::Utils::REPO_OWNER}/#{FluentTools::Utils::REPO_NAME}/releases"
      false
    end
  end
  # rubocop:enable Naming/PredicateMethod

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

    Net::HTTP.start(uri.host, uri.port, use_ssl: uri.scheme == 'https') do |http|
      request = Net::HTTP::Get.new(uri)
      response = http.request(request)

      # Follow redirects (GitHub releases use them)
      case response
      when Net::HTTPRedirection
        download_file(response['location'])
      when Net::HTTPSuccess
        response.body
      else
        raise "Download failed: HTTP #{response.code} - #{response.message}"
      end
    end
  end

  def install_binary(binary_data, platform)
    install_dir = FluentTools::Utils.determine_install_dir
    FileUtils.mkdir_p(install_dir)

    binary_extension = FluentTools::Utils.binary_extension(platform)
    binary_path = File.join(install_dir, "#{FluentTools::Utils::BINARY_NAME}#{binary_extension}")

    log_info "📁 Installing binary to: #{binary_path}"
    log_info "📏 Binary data size: #{binary_data.length} bytes"

    raise 'Downloaded binary data is empty' if binary_data.empty?

    File.binwrite(binary_path, binary_data)

    # Make executable on Unix-like systems
    File.chmod(0o755, binary_path) unless FluentTools::Utils.windows_platform?(platform)

    log_info "✅ Binary installed successfully (#{File.size(binary_path)} bytes)"
    @binary_path = binary_path
  end
end
