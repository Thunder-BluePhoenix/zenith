# Plugin Registry

The Zenith plugin registry is a hosted index of community plugins. It lets you discover and install plugins by name without knowing where they are hosted.

---

## Searching the registry

```bash
zenith plugin search <query>

zenith plugin search kubernetes
# NAME            VERSION   DESCRIPTION
# k8s-deploy      1.2.0     Deploy to Kubernetes clusters
# k8s-operator    0.8.1     Kubernetes Operator backend

zenith plugin search terraform
# NAME              VERSION   DESCRIPTION
# terraform-runner  2.1.0     Run Terraform plans and applies
```

The search queries the hosted registry index. If the registry is unreachable (offline), Zenith falls back to showing locally installed plugins that match the query.

---

## Installing from the registry

```bash
zenith plugin install k8s-deploy
```

Zenith:
1. Looks up `k8s-deploy` in the registry index
2. Downloads the plugin archive
3. Validates `plugin.toml` and the `requires_zenith` constraint
4. Runs the smoke test
5. Installs into `~/.zenith/plugins/k8s-deploy/`

---

## Registry index format

The hosted registry is a `registry.toml` file at a well-known URL. Community members can submit plugins by opening a pull request to the registry repository.

```toml
[[plugins]]
name        = "k8s-deploy"
version     = "1.2.0"
description = "Deploy to Kubernetes clusters"
author      = "community"
url         = "https://github.com/example/zenith-k8s-deploy/releases/download/v1.2.0/plugin.tar.gz"
requires_zenith = ">=0.1.0"
```

---

## Publishing a plugin

To publish your plugin to the registry:

1. Create a GitHub release with a `plugin.tar.gz` archive containing `plugin.toml` and your entrypoint binary
2. Open a pull request to the Zenith registry repository adding an entry to `registry.toml`
3. Ensure your `plugin.toml` includes a valid `requires_zenith` constraint
4. After the PR is merged, `zenith plugin search <your-plugin-name>` will find it

---

## Offline mode

When the registry is unreachable:

```bash
zenith plugin search deploy
# Registry unreachable — showing locally installed plugins only:
# k8s-deploy   1.2.0   Deploy to Kubernetes clusters
```

All other plugin commands (`list`, `info`, `install ./local`, `remove`) work fully offline since they only read from `~/.zenith/plugins/`.
