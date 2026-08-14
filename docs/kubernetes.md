# Kubernetes Deployment Guide (Helm 3)

VuIO provides an official Helm 3 chart to deploy the media server directly onto Kubernetes clusters.

---

## Quick Start

### Remote Installation (OCI Registry)

Install the chart directly from GitHub Container Registry without cloning the repository:

```bash
helm install vuio oci://ghcr.io/vuiodev/charts/vuio --version 0.0.44
```

### Local Installation

If you have cloned the repository, deploy from the local helm directory:

```bash
helm install vuio ./helm/vuio
```

---

## Networking & SSDP Discovery

For SSDP/UPnP multicast auto-discovery to work natively on your local area network (LAN), the container needs direct host network binding. The Helm chart is configured to use host networking by default:

```yaml
hostNetwork: true
```

> [!NOTE]
> On platforms like macOS Kubernetes (e.g. Minikube, Docker Desktop Kube, Kind), multicast routing restrictions in the hypervisor layer prevent SSDP packets from escaping to the physical LAN. If LAN auto-discovery is not required or you run behind an Ingress, host networking can be disabled:
>
> ```bash
> helm install vuio ./helm/vuio --set hostNetwork=false
> ```

---

## Configuration & Persistence

The Helm chart supports configuring a Persistent Volume Claim (PVC) to retain the SQLite database index and generated configuration across pod restarts. Additionally, your media volumes can be mounted directly:

```yaml
# custom-values.yaml
persistence:
  enabled: true
  storageClass: "local-path"
  size: 5Gi

media:
  volumeMounts:
    - name: media-storage
      mountPath: /media
      readOnly: true
  volumes:
    - name: media-storage
      hostPath:
        path: /mnt/storage/media  # Path to media on your Kubernetes worker node
        type: Directory
```

Apply your configuration:

```bash
helm install vuio ./helm/vuio -f custom-values.yaml
```

Refer to the default [`values.yaml`](../helm/vuio/values.yaml) file for a complete list of all parameters, including resource constraints, service configurations, and Ingress routing rules.

---

## Health Probes & Monitoring

The Helm chart configures automated probes and metrics scraping:
- **Liveness Probe**: `GET /healthz`
- **Readiness Probe**: `GET /readyz`
- **Prometheus Metrics**: `GET /metrics`

For full details on Prometheus metrics and telemetry endpoints, see the [Monitoring & Observability Guide](monitoring.md).
