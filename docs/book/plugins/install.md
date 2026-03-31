# Installing Plugins

Zenith's plugin system lets you extend the runtime with custom backends and integrations. Plugins are external processes that communicate over JSON-RPC on stdio — they can be written in any language.

---

## Installing from a local directory

```bash
zenith plugin install ./path/to/my-plugin
```

The directory must contain a `plugin.toml` manifest and the entrypoint binary.

Zenith will:
1. Read and validate `plugin.toml`
2. Check the `requires_zenith` version constraint (if set)
3. Copy the plugin directory into `~/.zenith/plugins/<name>/`
4. Run a smoke test (sends a `name` RPC — must return the plugin name)

If any step fails, installation is aborted and no files are written.

---

## Installing from the hosted registry

```bash
# Search for a plugin
zenith plugin search kubernetes
zenith plugin search terraform
zenith plugin search deploy

# Install by registry name
zenith plugin install k8s-deploy
zenith plugin install terraform-runner
```

The registry is fetched from the Zenith hosted index. Falls back to showing only locally installed plugins if the registry is unreachable (offline mode).

---

## Listing installed plugins

```bash
zenith plugin list
# NAME           VERSION   TYPE      DESCRIPTION
# k8s-deploy     1.2.0     backend   Kubernetes deployment backend
# notify-slack   0.5.1     hook      Post run results to Slack
```

---

## Viewing plugin details

```bash
zenith plugin info k8s-deploy
# name:            k8s-deploy
# version:         1.2.0
# type:            backend
# entrypoint:      k8s-deploy-bin
# description:     Kubernetes deployment backend
# requires_zenith: >=0.1.0
```

---

## Removing a plugin

```bash
zenith plugin remove k8s-deploy
```

This deletes `~/.zenith/plugins/k8s-deploy/` completely.

---

## Version constraints

The `requires_zenith` field in `plugin.toml` specifies the minimum Zenith version the plugin is compatible with:

```toml
requires_zenith = ">=0.1.0"
```

If your installed Zenith version does not satisfy this constraint, `zenith plugin install` refuses the installation and prints the versions involved.

---

## Plugin storage location

All installed plugins live in `~/.zenith/plugins/`. Each plugin occupies its own subdirectory named after the plugin:

```
~/.zenith/plugins/
  k8s-deploy/
    plugin.toml
    k8s-deploy-bin
  terraform-runner/
    plugin.toml
    terraform-runner-bin
```
