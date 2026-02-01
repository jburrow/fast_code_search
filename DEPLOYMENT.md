# Deployment Guide

This guide covers deploying fast_code_search in various environments.

## Table of Contents

- [Quick Start](#quick-start)
- [Production Deployment](#production-deployment)
- [Docker Deployment](#docker-deployment)
- [Kubernetes Deployment](#kubernetes-deployment)
- [Configuration](#configuration)
- [Monitoring](#monitoring)
- [Troubleshooting](#troubleshooting)

## Quick Start

### Local Deployment

1. Build the release binary:
```bash
cargo build --release
```

2. Start the server:
```bash
./target/release/fast_code_search_server
```

3. The server will listen on `0.0.0.0:50051`

### Testing the Deployment

Use the example client:
```bash
cargo run --example client
```

Or use `grpcurl` to test manually:
```bash
# Install grpcurl
go install github.com/fullstorydev/grpcurl/cmd/grpcurl@latest

# List services
grpcurl -plaintext localhost:50051 list

# Index a directory
grpcurl -plaintext -d '{"paths": ["."]}' \
    localhost:50051 search.CodeSearch/Index

# Search
grpcurl -plaintext -d '{"query": "fn main", "max_results": 10}' \
    localhost:50051 search.CodeSearch/Search
```

## Production Deployment

### System Requirements

**Minimum**:
- 2 CPU cores
- 4GB RAM
- 10GB disk space

**Recommended** (for 10GB+ codebases):
- 8+ CPU cores
- 16GB+ RAM
- SSD storage
- 50GB+ disk space

### Building for Production

```bash
# Build optimized release binary
cargo build --release

# Strip debug symbols to reduce binary size
strip target/release/fast_code_search_server

# Verify binary
./target/release/fast_code_search_server --version
```

### Running as a System Service

#### systemd (Linux)

Create `/etc/systemd/system/fast-code-search.service`:

```ini
[Unit]
Description=Fast Code Search gRPC Service
After=network.target

[Service]
Type=simple
User=codeuser
Group=codeuser
WorkingDirectory=/opt/fast_code_search
ExecStart=/opt/fast_code_search/fast_code_search_server
Restart=on-failure
RestartSec=10
StandardOutput=journal
StandardError=journal

# Security hardening
NoNewPrivileges=true
PrivateTmp=true
ProtectSystem=strict
ProtectHome=true
ReadWritePaths=/var/lib/fast_code_search

[Install]
WantedBy=multi-user.target
```

Enable and start:
```bash
sudo systemctl daemon-reload
sudo systemctl enable fast-code-search
sudo systemctl start fast-code-search
sudo systemctl status fast-code-search
```

View logs:
```bash
sudo journalctl -u fast-code-search -f
```

### Reverse Proxy with nginx

For HTTPS and load balancing:

```nginx
upstream grpc_backend {
    server 127.0.0.1:50051;
}

server {
    listen 443 ssl http2;
    server_name code-search.example.com;

    ssl_certificate /etc/ssl/certs/code-search.crt;
    ssl_certificate_key /etc/ssl/private/code-search.key;

    location / {
        grpc_pass grpc://grpc_backend;
        grpc_set_header Host $host;
        grpc_set_header X-Real-IP $remote_addr;
    }
}
```

## Docker Deployment

### Dockerfile

Create `Dockerfile` in project root:

```dockerfile
# Build stage
FROM rust:1.75 as builder

# Install protobuf compiler
RUN apt-get update && \
    apt-get install -y protobuf-compiler && \
    rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY . .

# Build release binary
RUN cargo build --release

# Runtime stage
FROM debian:bookworm-slim

# Install runtime dependencies
RUN apt-get update && \
    apt-get install -y ca-certificates && \
    rm -rf /var/lib/apt/lists/*

# Create non-root user
RUN useradd -m -u 1000 codeuser

WORKDIR /app

# Copy binary from builder
COPY --from=builder /app/target/release/fast_code_search_server /app/

# Set ownership
RUN chown -R codeuser:codeuser /app

USER codeuser

EXPOSE 50051

CMD ["/app/fast_code_search_server"]
```

### Build and Run

```bash
# Build image
docker build -t fast_code_search:latest .

# Run container
docker run -d \
    --name fast-code-search \
    -p 50051:50051 \
    -v /path/to/code:/data:ro \
    fast_code_search:latest

# View logs
docker logs -f fast-code-search

# Stop container
docker stop fast-code-search
```

### Docker Compose

Create `docker-compose.yml`:

```yaml
version: '3.8'

services:
  code-search:
    build: .
    container_name: fast-code-search
    ports:
      - "50051:50051"
    volumes:
      - /path/to/code:/data:ro
      - search-data:/var/lib/fast_code_search
    restart: unless-stopped
    healthcheck:
      test: ["CMD", "grpcurl", "-plaintext", "localhost:50051", "list"]
      interval: 30s
      timeout: 10s
      retries: 3
    deploy:
      resources:
        limits:
          cpus: '4'
          memory: 8G
        reservations:
          cpus: '2'
          memory: 4G

volumes:
  search-data:
```

Run with:
```bash
docker-compose up -d
```

## Kubernetes Deployment

### Deployment Manifest

Create `k8s/deployment.yaml`:

```yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: fast-code-search
  labels:
    app: fast-code-search
spec:
  replicas: 3
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
        image: fast_code_search:latest
        ports:
        - containerPort: 50051
          protocol: TCP
          name: grpc
        resources:
          requests:
            memory: "4Gi"
            cpu: "2"
          limits:
            memory: "8Gi"
            cpu: "4"
        livenessProbe:
          exec:
            command:
            - /bin/sh
            - -c
            - "grpcurl -plaintext localhost:50051 list"
          initialDelaySeconds: 30
          periodSeconds: 10
        volumeMounts:
        - name: code-volume
          mountPath: /data
          readOnly: true
      volumes:
      - name: code-volume
        persistentVolumeClaim:
          claimName: code-pvc
```

### Service Manifest

Create `k8s/service.yaml`:

```yaml
apiVersion: v1
kind: Service
metadata:
  name: fast-code-search-service
spec:
  selector:
    app: fast-code-search
  ports:
  - protocol: TCP
    port: 50051
    targetPort: 50051
  type: LoadBalancer
```

### Deploy to Kubernetes

```bash
# Create namespace
kubectl create namespace code-search

# Apply configurations
kubectl apply -f k8s/deployment.yaml -n code-search
kubectl apply -f k8s/service.yaml -n code-search

# Check status
kubectl get pods -n code-search
kubectl get svc -n code-search

# View logs
kubectl logs -f deployment/fast-code-search -n code-search
```

## Configuration

### Environment Variables

The server can be configured via environment variables:

```bash
# Server address (default: 0.0.0.0:50051)
export BIND_ADDRESS="0.0.0.0:50051"

# Log level (default: info)
export RUST_LOG="debug"

# Maximum concurrent searches
export MAX_CONCURRENT_SEARCHES="100"
```

### Performance Tuning

#### For Large Codebases (10GB+)

- Increase system file descriptor limit:
```bash
ulimit -n 65536
```

- Adjust memory limits in systemd service:
```ini
[Service]
MemoryMax=16G
```

- Use faster storage (SSD/NVMe)

#### CPU Optimization

The server automatically uses all available CPU cores via rayon. To limit:

```bash
# Use only 4 cores
export RAYON_NUM_THREADS=4
```

## Monitoring

### Metrics to Monitor

- **CPU Usage**: Should be proportional to search load
- **Memory Usage**: Should be < total codebase size (due to memory mapping)
- **Request Latency**: Sub-millisecond for most queries
- **Active Connections**: gRPC connections to clients

### Health Checks

```bash
# Check if server is responding
grpcurl -plaintext localhost:50051 list

# Expected output:
# search.CodeSearch
```

### Log Monitoring

Enable structured logging for better monitoring:

```bash
RUST_LOG=info cargo run --release --bin fast_code_search_server 2>&1 | \
    tee -a /var/log/fast_code_search.log
```

## Troubleshooting

### Server Won't Start

**Issue**: Port already in use
```bash
# Check what's using port 50051
lsof -i :50051

# Kill the process or use a different port
```

**Issue**: Permission denied
```bash
# Run with appropriate permissions or use port > 1024
```

### High Memory Usage

**Cause**: Large codebase indexed

**Solution**: Memory mapping keeps RAM usage low, but ensure sufficient system memory exists

### Slow Search Performance

**Possible causes**:
- Disk I/O bottleneck (use SSD)
- CPU bottleneck (increase cores)
- Large result sets (limit max_results)

**Debug**:
```bash
# Profile the server
perf record -g ./target/release/fast_code_search_server
perf report
```

### Connection Issues

**Test connectivity**:
```bash
# Test from client
telnet server-host 50051

# Use grpcurl for debugging
grpcurl -plaintext -v server-host:50051 list
```

## Security Considerations

### Network Security

- Use TLS for production deployments
- Restrict access via firewall rules
- Use VPN or private networks for internal access

### File System Security

- Run as non-root user
- Use read-only mounts for code directories
- Restrict file permissions

### Resource Limits

- Set memory limits to prevent OOM
- Set CPU limits to prevent resource exhaustion
- Limit concurrent connections

## Backup and Recovery

### Data to Backup

The server is stateless - it rebuilds indexes on startup. Backup:
- Server configuration files
- Deployment scripts
- The codebase being indexed

### Recovery Procedure

1. Restore server binary
2. Restore configuration
3. Start server
4. Re-index codebase via gRPC Index call

## Scaling

### Horizontal Scaling

Deploy multiple instances behind a load balancer:
- Each instance indexes the same codebase
- Load balancer distributes search requests
- Stateless design allows easy scaling

### Vertical Scaling

- Add more CPU cores for faster parallel search
- Add more RAM for larger codebases
- Use faster storage for reduced I/O latency

## Support

For deployment issues:
- Check logs first
- Review this guide
- Open an issue on GitHub with deployment details
