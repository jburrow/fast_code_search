/**
 * ServerManager – manages the lifecycle of the fast_code_search_server binary.
 *
 * Responsibilities:
 *  - Detect the current platform/architecture and map to a Rust target triple.
 *  - Locate an existing binary (custom path or global-storage cache).
 *  - Download the correct binary from GitHub Releases when it is not present.
 *  - Spawn and supervise the server process.
 *  - Expose start / stop / isRunning helpers consumed by extension.ts.
 */

import * as vscode from "vscode";
import * as path from "path";
import * as fs from "fs";
import * as https from "https";
import * as child_process from "child_process";
import type { IncomingMessage } from "http";

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const GITHUB_OWNER = "jburrow";
const GITHUB_REPO = "fast_code_search";
const BINARY_NAME = "fast_code_search_server";

/**
 * Map of Node.js (platform, arch) pairs to Rust target triples that are
 * published to GitHub Releases.
 */
const TARGET_MAP: Record<string, Record<string, string>> = {
  linux: {
    x64: "x86_64-unknown-linux-gnu",
    arm64: "aarch64-unknown-linux-gnu",
  },
  darwin: {
    x64: "x86_64-apple-darwin",
    arm64: "aarch64-apple-darwin",
  },
  win32: {
    x64: "x86_64-pc-windows-msvc",
  },
};

// ---------------------------------------------------------------------------
// ServerManager
// ---------------------------------------------------------------------------

export class ServerManager implements vscode.Disposable {
  private serverProcess: child_process.ChildProcess | null = null;
  private readonly storageDir: string;

  constructor(
    private readonly context: vscode.ExtensionContext,
    private readonly outputChannel: vscode.OutputChannel
  ) {
    this.storageDir = context.globalStorageUri.fsPath;
  }

  // --------------------------------------------------------------------------
  // Platform helpers
  // --------------------------------------------------------------------------

  /** Returns the Rust target triple for the running platform, or null if unsupported. */
  getTargetTriple(): string | null {
    return TARGET_MAP[process.platform]?.[process.arch] ?? null;
  }

  /** Returns true when the current platform is supported for auto-download. */
  isPlatformSupported(): boolean {
    return this.getTargetTriple() !== null;
  }

  // --------------------------------------------------------------------------
  // Binary location
  // --------------------------------------------------------------------------

  /**
   * Returns the path to the server binary to use, in priority order:
   *   1. User-configured `fastCodeSearch.serverBinaryPath` (absolute path).
   *   2. Cached binary in the extension's global storage directory.
   */
  getBinaryPath(): string {
    const custom = vscode.workspace
      .getConfiguration("fastCodeSearch")
      .get<string>("serverBinaryPath", "")
      .trim();
    if (custom) {
      return custom;
    }
    const ext = process.platform === "win32" ? ".exe" : "";
    return path.join(this.storageDir, `${BINARY_NAME}${ext}`);
  }

  /** Returns true when the binary file exists on disk. */
  isBinaryInstalled(): boolean {
    return fs.existsSync(this.getBinaryPath());
  }

  // --------------------------------------------------------------------------
  // Download
  // --------------------------------------------------------------------------

  /**
   * Download the platform-appropriate server binary from GitHub Releases.
   *
   * @param version  Tag name to download, e.g. `"v0.1.0"`.
   * @param progress Optional VSCode progress reporter.
   */
  async downloadBinary(
    version: string,
    progress?: vscode.Progress<{ message?: string; increment?: number }>
  ): Promise<void> {
    const target = this.getTargetTriple();
    if (!target) {
      throw new Error(
        `Unsupported platform: ${process.platform} ${process.arch}. ` +
          `Please download the server binary manually and set ` +
          `fastCodeSearch.serverBinaryPath in your settings.`
      );
    }

    const archiveExt = process.platform === "win32" ? ".zip" : ".tar.gz";
    const archiveName = `fast_code_search-${version}-${target}${archiveExt}`;
    const downloadUrl = `https://github.com/${GITHUB_OWNER}/${GITHUB_REPO}/releases/download/${version}/${archiveName}`;

    this.outputChannel.appendLine(
      `Downloading server binary from:\n  ${downloadUrl}`
    );
    progress?.report({ message: `Downloading ${archiveName}…` });

    fs.mkdirSync(this.storageDir, { recursive: true });

    const archivePath = path.join(this.storageDir, archiveName);
    await downloadFile(downloadUrl, archivePath, (pct) => {
      progress?.report({ message: `Downloading… ${pct}%` });
    });

    progress?.report({ message: "Extracting binary…" });
    await this.extractBinary(archivePath);

    // Clean up the archive
    try {
      fs.unlinkSync(archivePath);
    } catch {
      // Non-fatal
    }

    const binaryPath = this.getBinaryPath();
    this.outputChannel.appendLine(
      `Server binary installed at:\n  ${binaryPath}`
    );
    progress?.report({ message: "Binary ready." });
  }

  // --------------------------------------------------------------------------
  // Process management
  // --------------------------------------------------------------------------

  /** Returns true when the managed server process is alive. */
  isRunning(): boolean {
    return this.serverProcess !== null && this.serverProcess.exitCode === null;
  }

  /**
   * Start the server process.
   *
   * @param workspacePaths  Directories to index (passed as `--index` flags).
   */
  async startServer(workspacePaths: string[]): Promise<void> {
    if (this.isRunning()) {
      this.outputChannel.appendLine("Server is already running.");
      return;
    }

    const binaryPath = this.getBinaryPath();
    if (!fs.existsSync(binaryPath)) {
      throw new Error(
        "Server binary not found. " +
          'Run "Fast Code Search: Download Server" to install it, ' +
          "or set fastCodeSearch.serverBinaryPath to an existing binary."
      );
    }

    const cfg = vscode.workspace.getConfiguration("fastCodeSearch");
    const port = cfg.get<number>("keywordServer.port", 8080);

    // Build argument list: --index <path> for each workspace folder
    const args: string[] = [];
    for (const p of workspacePaths) {
      args.push("--index", p);
    }

    this.outputChannel.appendLine(
      `Starting server: ${binaryPath} ${args.join(" ")}`
    );

    this.serverProcess = child_process.spawn(binaryPath, args, {
      stdio: ["ignore", "pipe", "pipe"],
      env: { ...process.env, FAST_CODE_SEARCH_WEB_PORT: String(port) },
    });

    this.serverProcess.stdout?.on("data", (data: Buffer) => {
      this.outputChannel.append(`[server] ${data.toString()}`);
    });
    this.serverProcess.stderr?.on("data", (data: Buffer) => {
      this.outputChannel.append(`[server] ${data.toString()}`);
    });
    this.serverProcess.on("exit", (code) => {
      this.outputChannel.appendLine(
        `Server process exited (code ${code ?? "?"}).`
      );
      this.serverProcess = null;
    });

    // Wait for the server to become reachable
    await waitForServer(port, 15_000, this.outputChannel);
  }

  /** Stop the managed server process if it is running. */
  stopServer(): void {
    if (this.serverProcess) {
      this.outputChannel.appendLine("Stopping server…");
      if (process.platform !== "win32") {
        // Ask the server to shut down gracefully; fall back to SIGKILL after 5 s
        this.serverProcess.kill("SIGTERM");
        const proc = this.serverProcess;
        setTimeout(() => {
          if (proc.exitCode === null) {
            proc.kill("SIGKILL");
          }
        }, 5000);
      } else {
        this.serverProcess.kill();
      }
      this.serverProcess = null;
    }
  }

  dispose(): void {
    this.stopServer();
  }

  // --------------------------------------------------------------------------
  // Private – extraction
  // --------------------------------------------------------------------------

  private async extractBinary(archivePath: string): Promise<void> {
    const binaryPath = this.getBinaryPath();
    const isWindows = process.platform === "win32";

    if (isWindows) {
      // PowerShell: extract zip then move the exe into place
      await runCommand("powershell", [
        "-NoProfile",
        "-Command",
        [
          `$dest = '${this.storageDir}'`,
          `Expand-Archive -Force -Path '${archivePath}' -DestinationPath $dest`,
          `$exe = Get-ChildItem -Recurse -Filter '${BINARY_NAME}.exe' $dest | Select-Object -First 1`,
          `if ($exe -and $exe.FullName -ne '${binaryPath}') { Move-Item -Force $exe.FullName '${binaryPath}' }`,
        ].join("; "),
      ]);
    } else {
      // Attempt to extract just the binary (handles archives with a sub-dir)
      const extractOk = await runCommand("tar", [
        "-xzf",
        archivePath,
        "-C",
        this.storageDir,
        "--strip-components=1",
        "--wildcards",
        `*/${BINARY_NAME}`,
      ]).then(
        () => true,
        () => false
      );

      if (!extractOk) {
        // Fallback: extract everything
        await runCommand("tar", ["-xzf", archivePath, "-C", this.storageDir]);
      }

      // The binary might be inside a sub-directory after a full extraction
      if (!fs.existsSync(binaryPath)) {
        const entries = fs.readdirSync(this.storageDir, {
          withFileTypes: true,
        });
        for (const entry of entries) {
          if (entry.isDirectory()) {
            const candidate = path.join(
              this.storageDir,
              entry.name,
              BINARY_NAME
            );
            if (fs.existsSync(candidate)) {
              fs.renameSync(candidate, binaryPath);
              break;
            }
          }
        }
      }

      if (!fs.existsSync(binaryPath)) {
        throw new Error(
          `Extraction succeeded but binary not found at:\n  ${binaryPath}`
        );
      }

      fs.chmodSync(binaryPath, 0o755);
    }
  }
}

// ---------------------------------------------------------------------------
// Module-level helpers (internal use only)
// ---------------------------------------------------------------------------

/**
 * Download a URL to a local file, following HTTP redirects.
 * Calls `onProgress` with approximate completion percentage (0–100).
 */
function downloadFile(
  url: string,
  dest: string,
  onProgress: (pct: number) => void
): Promise<void> {
  return new Promise((resolve, reject) => {
    const attempt = (currentUrl: string, redirectsLeft: number): void => {
      https
        .get(currentUrl, (res: IncomingMessage) => {
          if (
            (res.statusCode === 301 ||
              res.statusCode === 302 ||
              res.statusCode === 307 ||
              res.statusCode === 308) &&
            res.headers.location
          ) {
            if (redirectsLeft <= 0) {
              reject(new Error("Too many redirects while downloading binary."));
              return;
            }
            attempt(res.headers.location, redirectsLeft - 1);
            return;
          }

          if (res.statusCode !== 200) {
            reject(
              new Error(
                `Download failed – HTTP ${res.statusCode ?? "unknown"}: ${currentUrl}`
              )
            );
            return;
          }

          const total = parseInt(
            res.headers["content-length"] ?? "0",
            10
          );
          let downloaded = 0;

          const file = fs.createWriteStream(dest);
          res.on("data", (chunk: Buffer) => {
            downloaded += chunk.length;
            if (total > 0) {
              onProgress(Math.floor((downloaded / total) * 100));
            }
          });
          res.pipe(file);
          file.on("finish", () => {
            file.close();
            resolve();
          });
          file.on("error", (err) => {
            file.close();
            fs.unlink(dest, () => undefined);
            reject(err);
          });
        })
        .on("error", reject);
    };

    attempt(url, 10);
  });
}

/** Spawn a command and return a promise that resolves when exit code is 0. */
function runCommand(cmd: string, args: string[]): Promise<void> {
  return new Promise((resolve, reject) => {
    const proc = child_process.spawn(cmd, args, { stdio: "pipe" });
    let stderr = "";
    proc.stderr?.on("data", (d: Buffer) => (stderr += d.toString()));
    proc.on("close", (code) => {
      if (code === 0) {
        resolve();
      } else {
        reject(new Error(`${cmd} exited with code ${code ?? "?"}:\n${stderr}`));
      }
    });
    proc.on("error", reject);
  });
}

/**
 * Poll the server's health endpoint until it responds or the timeout expires.
 */
async function waitForServer(
  port: number,
  maxWaitMs: number,
  out: vscode.OutputChannel
): Promise<void> {
  const url = `http://localhost:${port}/api/health`;
  const deadline = Date.now() + maxWaitMs;
  let announced = false;

  while (Date.now() < deadline) {
    try {
      const res = await fetch(url);
      if (res.ok) {
        out.appendLine("Server is ready.");
        return;
      }
    } catch {
      // Server not yet accepting connections
    }

    if (!announced) {
      out.appendLine("Waiting for server to become ready…");
      announced = true;
    }

    await new Promise<void>((resolve) => setTimeout(resolve, 500));
  }

  out.appendLine(
    `Warning: server did not respond on port ${port} within ` +
      `${maxWaitMs / 1000}s. It may still be starting up.`
  );
}
