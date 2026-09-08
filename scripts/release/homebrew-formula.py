#!/usr/bin/env python3
"""Write a Homebrew formula for a release from the archives' .sha256 files.

    scripts/release/homebrew-formula.py v0.13.0 artifacts/ > Formula/fast_code_search.rb

The formula installs both binaries from the release archive for the host
(macOS arm64/x86_64, Linux x86_64 static) and ships the startup unit files
under share/. The release workflow pushes the result to the tap repository
when a HOMEBREW_TAP_TOKEN secret is configured.
"""
import pathlib
import sys

tag, artifacts = sys.argv[1], pathlib.Path(sys.argv[2])
version = tag.lstrip("v")
base = f"https://github.com/jburrow/fast_code_search/releases/download/{tag}"


def sha(target: str) -> str:
    name = f"fast_code_search-{tag}-{target}.tar.gz.sha256"
    return (artifacts / name).read_text().split()[0]


targets = {
    "arm_mac": "aarch64-apple-darwin",
    "intel_mac": "x86_64-apple-darwin",
    "linux": "x86_64-unknown-linux-musl",
}
print(f'''class FastCodeSearch < Formula
  desc "Code search that ranks like code: definitions above usages, from an in-memory trigram index"
  homepage "https://jburrow.github.io/fast_code_search/docs/"
  version "{version}"
  license "MIT"

  on_macos do
    on_arm do
      url "{base}/fast_code_search-{tag}-{targets['arm_mac']}.tar.gz"
      sha256 "{sha(targets['arm_mac'])}"
    end
    on_intel do
      url "{base}/fast_code_search-{tag}-{targets['intel_mac']}.tar.gz"
      sha256 "{sha(targets['intel_mac'])}"
    end
  end

  on_linux do
    url "{base}/fast_code_search-{tag}-{targets['linux']}.tar.gz"
    sha256 "{sha(targets['linux'])}"
  end

  def install
    bin.install "fast_code_search_server", "fcs"
    pkgshare.install "config.toml.example", "deploy", "CLI.md", "RUN-AT-STARTUP.md"
  end

  def caveats
    <<~EOS
      First run: fast_code_search_server --init config.toml, add your paths,
      start the server, then: fcs 'fn main'
      Startup unit files: #{{pkgshare}}/deploy
    EOS
  end

  test do
    assert_match version.to_s, shell_output("#{{bin}}/fcs --version")
    assert_match version.to_s, shell_output("#{{bin}}/fast_code_search_server --version")
  end
end''')
