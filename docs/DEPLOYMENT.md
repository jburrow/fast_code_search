# Deployment Guide

This guide covers running fast_code_search as a shared service: a
system-wide unit, a reverse proxy, a container and Kubernetes. For a
single developer's machine (a per-user service started at login on Linux,
macOS or Windows) see [RUN-AT-STARTUP.md](RUN-AT-STARTUP.md).

## Table of Contents

- [What runs](#what-runs)
- [Quick Start](#quick-start)
- [Production Deployment](#production-deployment)
- [Container](#container)
- [Kubernetes Deployment](#kubernetes-deployment)
- [Configuration](#configuration)
- [Monitoring](#monitoring)
- [Troubleshooting](#troubleshooting)

## What runs

One process, `fast_code_search_server`, holds the index in memory, watches
the indexed trees, and serves two listeners:

| Listener | Config key | Default | Serves |
|----------|------------|---------|--------|
| Web | `server.web_address` | `127.0.0.1:8080` | The web UI, the REST API (`/api/…`), `/metrics`, and what `fcs` talks to |
| gRPC | `server.address` | `127.0.0.1:50051` | `search.CodeSearch` (see `proto/search.proto`) and `grpc.health.v1` |

Both bind to loopback by default because the server has no authentication
and returns file contents. Turn gRPC off with `enable_grpc = false` (or
`--no-grpc`) if nothing uses it. If one port cannot be bound the server
logs which one and keeps running on the other; it exits only when neither
could start.

The index is saved to `indexer.index_path` and reloaded on the next start,
so a restart takes seconds rather than a full build. Only one server writes
a given index: a second one pointed at the same `index_path` loads it
read-only and never saves.

## Quick Start

### Local Deployment

1. Build the release binaries (or download a release archive):
```bash
cargo build --release
```

2. Write a configuration, add your paths to `[indexer] paths`, and start
   the server:
```bash
./target/release/fast_code_search_server --init config.toml
./target/release/fast_code_search_server --config config.toml
```

3. Open <http://127.0.0.1:8080>, or search from a terminal with
   `./target/release/fcs 'fn main'`.

### Testing the Deployment

```bash
# Liveness, readiness and the index size
curl http://127.0.0.1:8080/api/health
curl http://127.0.0.1:8080/api/ready
fcs status

# A search over REST
curl "http://127.0.0.1:8080/api/search?q=fn%20main&max=5"
```

Over gRPC, use the example client or `grpcurl` with the schema (the server
does not offer gRPC reflection, so `grpcurl` needs the `.proto`):

```bash
cargo run --example client

grpcurl -plaintext -import-path proto -proto search.proto \
    -d '{"query": "fn main", "max_results": 10}' \
    localhost:50051 search.CodeSearch/Search

grpcurl -plaintext -import-path proto -proto search.proto \
    -d '{"paths": ["/srv/repos/api/new-module"]}' \
    localhost:50051 search.CodeSearch/Index    # a path under a configured root
```

## Production Deployment

### System Requirements

Memory is the constraint. The index keeps trigram posting lists and
symbols in memory and maps file contents; as a rough guide, the
[benchmark](https://jburrow.github.io/fast_code_search/docs/benchmarks/latest.html)
tree of 6,200 files and 39 MB of text uses about 120 MB resident. See
[Performance and memory](https://jburrow.github.io/fast_code_search/docs/guides/performance.html)
for sizing. Indexing uses every core; searching is fast on any of them.
Keep the index on local storage with free space and free inodes (the
server warns at startup and before each save when either runs low).

### Running as a System Service

#### systemd (Linux)

The repository's `deploy/systemd/fast-code-search.service` is the per-user
variant. A system-wide one, `/etc/systemd/system/fast-code-search.service`:

```ini
[Unit]
Description=fast_code_search server
After=network.target

[Service]
Type=simple
User=codeuser
Group=codeuser
ExecStart=/usr/local/bin/fast_code_search_server --config /etc/fast_code_search/config.toml
# SIGINT lets the server save the index before exiting.
KillSignal=SIGINT
TimeoutStopSec=120
Restart=on-failure
RestartSec=10
Environment=RUST_LOG=info

# Security hardening
NoNewPrivileges=true
PrivateTmp=true
ProtectSystem=strict
ProtectHome=true
ReadWritePaths=/var/lib/fast_code_search

[Install]
WantedBy=multi-user.target
```

With `index_path = "/var/lib/fast_code_search/index.fcsidx"` in the config.
`ProtectHome=true` hides `/home`; drop it if the indexed trees live there.

```bash
sudo systemctl daemon-reload
sudo systemctl enable --now fast-code-search
sudo journalctl -u fast-code-search -f
```

### Reverse Proxy with nginx

The server has no authentication: put it behind a proxy that has. The web
listener needs WebSocket upgrades for `/ws/progress`; gRPC needs HTTP/2.

```nginx
server {
    listen 443 ssl http2;
    server_name code-search.example.com;

    ssl_certificate /etc/ssl/certs/code-search.crt;
    ssl_certificate_key /etc/ssl/private/code-search.key;

    # auth_request / auth_basic / your SSO here

    location / {
        proxy_pass http://127.0.0.1:8080;
        proxy_http_version 1.1;
        proxy_set_header Upgrade $http_upgrade;
        proxy_set_header Connection "upgrade";
        proxy_set_header Host $host;
    }

    location /search.CodeSearch/ {
        grpc_pass grpc://127.0.0.1:50051;
    }
}
```

## Container

Every release publishes an image built from the repository's `Dockerfile`.
It indexes `/src`, serves the web UI and REST API on 8080 and gRPC on
50051, and saves the index under `/var/lib/fast_code_search`:

```bash
docker run -d --name fast-code-search \
    -p 127.0.0.1:8080:8080 \
    -v /path/to/code:/src:ro \
    -v fcs-index:/var/lib/fast_code_search \
    ghcr.io/jburrow/fast_code_search
```

The baked-in configuration is `deploy/docker/config.toml`. To use your own,
mount it over it: `-v ./config.toml:/etc/fast_code_search/config.toml:ro`.
The image's health check runs `fcs status`. To build the image yourself:
`docker build -t fast_code_search .`.

### Docker Compose

```yaml
services:
  code-search:
    image: ghcr.io/jburrow/fast_code_search
    ports:
      - "127.0.0.1:8080:8080"
    volumes:
      - /path/to/code:/src:ro
      - search-index:/var/lib/fast_code_search
    restart: unless-stopped

volumes:
  search-index:
```

## Kubernetes Deployment

Each replica builds and holds its own index, so give each its own storage
for `index_path` (an `emptyDir` rebuilds on every pod start; a
`StatefulSet` volume keeps it). Never point two replicas at one index file
on a shared volume: the second loads it read-only and never saves.

```yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: fast-code-search
spec:
  replicas: 1
  selector:
    matchLabels:
      app: fast-code-search
  template:
    metadata:
      labels:
        app: fast-code-search
    spec:
      containers:
      - name: fast-code-search
        image: ghcr.io/jburrow/fast_code_search
        ports:
        - containerPort: 8080
          name: http
        - containerPort: 50051
          name: grpc
        resources:
          requests:
            memory: "2Gi"
            cpu: "2"
          limits:
            memory: "8Gi"
        livenessProbe:
          httpGet:
            path: /api/health
            port: http
          periodSeconds: 10
        readinessProbe:
          httpGet:
            path: /api/ready
            port: http
          periodSeconds: 5
        volumeMounts:
        - name: code
          mountPath: /src
          readOnly: true
        - name: index
          mountPath: /var/lib/fast_code_search
      volumes:
      - name: code
        persistentVolumeClaim:
          claimName: code-pvc
      - name: index
        emptyDir: {}
---
apiVersion: v1
kind: Service
metadata:
  name: fast-code-search
spec:
  selector:
    app: fast-code-search
  ports:
  - name: http
    port: 8080
    targetPort: http
  - name: grpc
    port: 50051
    targetPort: grpc
```

`/api/ready` answers 503 until there is an index to search, so a pod
receives traffic only once it can answer. A gRPC-native probe
(`grpc: {port: 50051}`) works too: the standard health service is served.

## Configuration

### Environment Variables

Most settings live in the TOML config (`--init` writes every key with its
default and a comment). The environment variables the server reads are:

```bash
# Config file to load (checked before ./fast_code_search.toml and the user config dir)
export FCS_CONFIG="/etc/fast_code_search/config.toml"

# Log filter (takes precedence over --verbose)
export RUST_LOG="debug"

# OpenTelemetry (standard names) and the project switch
export OTEL_EXPORTER_OTLP_ENDPOINT="http://collector:4317"
export OTEL_SERVICE_NAME="fast_code_search"
export OTEL_SDK_DISABLED="true"        # final: disables export regardless of config
export FCS_TRACING_ENABLED="true"      # overrides telemetry.enabled in the TOML

# Indexing threads (default: every core)
export RAYON_NUM_THREADS=4
```

Listen addresses are `server.address` / `server.web_address` in the config or
`--address` / `--web-address` on the command line; `server.enable_grpc` (or
`--no-grpc`) turns gRPC off; the search concurrency cap is
`server.max_concurrent_searches` and the request timeout
`server.request_timeout_secs`. The configuration is validated at startup:
unknown keys, unparseable addresses and zero limits are errors; a missing
index directory is a warning (the first save creates it). The first log
lines say which config file was used.

## Monitoring

- `GET /api/health`: liveness, always 200 while the process runs, with a
  `problems` count of startup and storage problems.
- `GET /api/ready`: 200 once the index can serve results, 503 before.
- `GET /api/diagnostics` (the **Index** page in the UI): which APIs
  started, where the index is saved, the problems themselves, and
  self-tests that search for sampled files.
- `GET /metrics`: Prometheus text format: request and error counters, a
  search latency histogram, and index gauges (files, trigrams, dependency
  edges, content bytes, indexing, ready).
- Logs: `RUST_LOG=info` (the default) logs startup, each build, save and
  watcher batch; `debug` logs every path and ignore file.

## Troubleshooting

The [troubleshooting guide](https://jburrow.github.io/fast_code_search/docs/guides/troubleshooting.html)
covers the common symptoms. Deployment-specific ones:

### A port is already in use

The log says `gRPC API NOT started` or `Web UI ... NOT started` with the
port, and the server keeps running on the other API. Find the holder with
`ss -ltnp | grep :8080` (or `lsof -i :8080`), then stop it, change the
address, or set `enable_grpc = false`.

### "another fast_code_search server is already using this index"

Two servers share an `index_path`. The log names the other process's PID
and addresses (also in `<index_path>.lock.owner`). Stop it or give each
server its own `index_path`; until then this one runs read-only.

### "No space left on device" with free space showing

The filesystem holding the index has run out of inodes; the log says
whether bytes or inodes ran out, and `df -i` confirms.

### Memory Allocation Errors on RHEL7/CentOS7

**Symptoms**:
- "cannot allocate memory" during indexing
- "Reached mmap limit" error message
- Works fine on other systems

**Root Cause**: Low `vm.max_map_count` limit (often 65530 on RHEL7, need 262144+)

**Automatic Detection**: The server reads the limit at startup (Linux only)
and stops mapping new files at 85% of it, with an error that names the
remedy. Files already indexed remain searchable.

**Solution 1: with sudo access**:
```bash
sysctl vm.max_map_count                                   # current limit
sudo sysctl -w vm.max_map_count=524288                    # until reboot
echo "vm.max_map_count=524288" | sudo tee -a /etc/sysctl.conf
sudo sysctl -p
```

**Solution 2: without sudo access**, index less:
```toml
[indexer]
max_file_size = 2097152  # 2MB instead of 10MB
exclude_patterns = [
    "**/node_modules/**", "**/target/**", "**/.git/**", "**/build/**",
    "**/dist/**", "**/vendor/**", "**/*.min.js", "**/*.min.css", "**/*.lock", "**/*.svg"
]
include_extensions = ["rs", "py", "js", "ts", "java", "go", "c", "cpp", "h"]
```

## Security Considerations

- Keep both listeners on loopback or a private network, and put an
  authenticating proxy in front of anything shared; see [SECURITY.md](../SECURITY.md).
- Run as a non-root user and mount the indexed code read-only.
- `cors_origins` stays empty unless a page on another origin calls the API.

## Backup and Recovery

The index file is a cache of the indexed trees: losing it costs one full
build, nothing else. Back up the configuration file; to recover, restore
it and start the server.

## Scaling

- **Vertically**: more cores make builds faster; more memory holds larger
  trees.
- **Horizontally**: run several instances behind a load balancer, each with
  its own index of the same trees. They are independent; nothing is shared
  between them.
